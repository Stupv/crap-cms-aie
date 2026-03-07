//! Lua CRUD function registration and type conversion helpers.

use anyhow::Result;
use mlua::{Lua, Value};
use std::collections::HashMap;

use crate::config::LocaleConfig;
use crate::core::SharedRegistry;
use crate::core::upload;
use crate::db::query::{self, AccessResult, FindQuery, Filter, FilterOp, FilterClause, LocaleContext};
use crate::db::query::filter::normalize_filter_fields;

use super::{TxContext, UserContext, HookDepth, MaxHookDepth};
use super::{HookContext, HookEvent, FieldHookEvent, hook_ctx_to_string_map};
use super::{run_hooks_inner, run_field_hooks_inner, validate_fields_inner, apply_after_read_inner};
use super::access::{check_access_with_lua, check_field_read_access_with_lua, check_field_write_access_with_lua};
use super::converters::*;

/// Get the active transaction connection from Lua app_data.
/// Returns an error if called outside of `run_hooks_with_conn`.
pub(crate) fn get_tx_conn(lua: &Lua) -> mlua::Result<*const rusqlite::Connection> {
    let ctx = lua.app_data_ref::<TxContext>()
        .ok_or_else(|| mlua::Error::RuntimeError(
            "crap.collections CRUD functions are only available inside hooks \
             with transaction context (before_change, before_delete, etc.)"
                .into()
        ))?;
    Ok(ctx.0)
}

/// Register the CRUD functions on `crap.collections` and `crap.globals`.
/// They read the active connection from Lua app_data (set by `run_hooks_with_conn`).
/// Untestable as unit: registers Lua closures that require TxContext + full DB.
/// Covered by integration tests (hook CRUD operations in tests/).
#[cfg(not(tarpaulin_include))]
pub(crate) fn register_crud_functions(lua: &Lua, registry: SharedRegistry, locale_config: &LocaleConfig, pagination_config: &crate::config::PaginationConfig) -> Result<()> {
    let crap: mlua::Table = lua.globals().get("crap")?;
    let collections: mlua::Table = crap.get("collections")?;

    // crap.collections.find(collection, query?)
    // query.depth (optional, default 0): populate relationship fields to this depth
    // query.locale (optional): locale code or "all"
    // query.overrideAccess (optional, default true): bypass access control
    {
        let reg = registry.clone();
        let lc = locale_config.clone();
        let pg_default = pagination_config.default_limit;
        let pg_max = pagination_config.max_limit;
        let pg_cursor = pagination_config.is_cursor();
        let find_fn = lua.create_function(move |lua, (collection, query_table): (String, Option<mlua::Table>)| {
            let conn_ptr = get_tx_conn(lua)?;
            // Safety: pointer is valid while TxContext is in app_data
            let conn = unsafe { &*conn_ptr };

            let depth: i32 = query_table.as_ref()
                .and_then(|qt| qt.get::<i32>("depth").ok())
                .unwrap_or(0)
                .clamp(0, 10);

            let locale_str: Option<String> = query_table.as_ref()
                .and_then(|qt| qt.get::<Option<String>>("locale").ok().flatten());
            let locale_ctx = LocaleContext::from_locale_string(locale_str.as_deref(), &lc);

            let override_access: bool = query_table.as_ref()
                .and_then(|qt| qt.get::<Option<bool>>("overrideAccess").ok().flatten())
                .unwrap_or(true);

            let def = {
                let r = reg.read().map_err(|e| mlua::Error::RuntimeError(
                    format!("Registry lock: {}", e)
                ))?;
                r.get_collection(&collection)
                    .cloned()
                    .ok_or_else(|| mlua::Error::RuntimeError(
                        format!("Collection '{}' not found", collection)
                    ))?
            };

            let draft: bool = query_table.as_ref()
                .and_then(|qt| qt.get::<Option<bool>>("draft").ok().flatten())
                .unwrap_or(false);

            let (mut find_query, lua_page) = match query_table {
                Some(qt) => lua_table_to_find_query(&qt)?,
                None => (FindQuery::default(), None),
            };

            // Clamp limit to configured bounds
            find_query.limit = Some(query::apply_pagination_limits(
                find_query.limit, pg_default, pg_max,
            ));

            // Convert page → offset if page was provided
            if let Some(p) = lua_page {
                let clamped = find_query.limit.unwrap_or(pg_default);
                find_query.offset = Some((p.max(1) - 1) * clamped);
            }

            // Ignore cursors if cursor pagination is disabled
            if !pg_cursor {
                find_query.after_cursor = None;
                find_query.before_cursor = None;
            }

            // Normalize dot notation: group dots → __, array/block/rel dots preserved
            normalize_filter_fields(&mut find_query.filters, &def.fields);

            // Draft-aware filtering: if collection has drafts and draft=false (default),
            // only return published documents (matches gRPC behavior)
            if def.has_drafts() && !draft {
                find_query.filters.push(FilterClause::Single(Filter {
                    field: "_status".to_string(),
                    op: FilterOp::Equals("published".to_string()),
                }));
            }

            // Enforce access control when overrideAccess = false
            if !override_access {
                let user_doc = lua.app_data_ref::<UserContext>()
                    .and_then(|uc| uc.0.clone());
                let result = check_access_with_lua(lua, def.access.read.as_deref(), user_doc.as_ref(), None, None)
                    .map_err(|e| mlua::Error::RuntimeError(format!("access check error: {}", e)))?;
                match result {
                    AccessResult::Denied => return Err(mlua::Error::RuntimeError("Read access denied".into())),
                    AccessResult::Constrained(extra) => find_query.filters.extend(extra),
                    AccessResult::Allowed => {}
                }
            }

            // Fire before_read hooks (informational, no CRUD access needed)
            let before_ctx = HookContext {
                collection: collection.clone(),
                operation: "find".to_string(),
                data: HashMap::new(),
                locale: None,
                draft: None,
                context: HashMap::new(),
            };
            run_hooks_inner(lua, &def.hooks, HookEvent::BeforeRead, before_ctx)
                .map_err(|e| mlua::Error::RuntimeError(format!("before_read hook error: {}", e)))?;

            query::validate_query_fields(&def, &find_query, locale_ctx.as_ref())
                .map_err(|e| mlua::Error::RuntimeError(format!("find error: {}", e)))?;

            let mut docs = query::find(conn, &collection, &def, &find_query, locale_ctx.as_ref())
                .map_err(|e| mlua::Error::RuntimeError(format!("find error: {}", e)))?;
            let total = query::count_with_search(conn, &collection, &def, &find_query.filters, locale_ctx.as_ref(), find_query.search.as_deref())
                .map_err(|e| mlua::Error::RuntimeError(format!("count error: {}", e)))?;

            // Hydrate join table data + populate relationships
            let select_slice = find_query.select.as_deref();
            for doc in &mut docs {
                query::hydrate_document(conn, &collection, &def.fields, doc, select_slice, locale_ctx.as_ref())
                    .map_err(|e| mlua::Error::RuntimeError(format!("hydrate error: {}", e)))?;
            }
            if depth > 0 {
                let r = reg.read().map_err(|e| mlua::Error::RuntimeError(
                    format!("Registry lock: {}", e)
                ))?;
                let pop_ctx = query::PopulateContext {
                    conn, registry: &r, collection_slug: &collection, def: &def,
                };
                let pop_opts = query::PopulateOpts {
                    depth, select: select_slice, locale_ctx: locale_ctx.as_ref(),
                };
                query::populate_relationships_batch(
                    &pop_ctx, &mut docs, &pop_opts,
                ).map_err(|e| mlua::Error::RuntimeError(format!("populate error: {}", e)))?;
            }
            // Assemble sizes for upload collections
            if let Some(ref upload_config) = def.upload {
                if upload_config.enabled {
                    for doc in &mut docs {
                        upload::assemble_sizes_object(doc, upload_config);
                    }
                }
            }

            // Apply select field stripping for find results
            if let Some(ref sel) = find_query.select {
                for doc in &mut docs {
                    query::apply_select_to_document(doc, sel);
                }
            }

            // Field-level read stripping when overrideAccess = false
            if !override_access {
                let user_doc = lua.app_data_ref::<UserContext>()
                    .and_then(|uc| uc.0.clone());
                let denied = check_field_read_access_with_lua(lua, &def.fields, user_doc.as_ref());
                if !denied.is_empty() {
                    for doc in &mut docs {
                        for name in &denied {
                            doc.fields.remove(name);
                        }
                    }
                }
            }

            // Run after_read hooks (field-level, collection-level, registered)
            let docs: Vec<_> = docs.into_iter()
                .map(|doc| apply_after_read_inner(lua, &def.hooks, &def.fields, &collection, "find", doc))
                .collect();

            let limit = find_query.limit.unwrap_or(pg_default);
            let offset: i64 = find_query.offset.unwrap_or(0);

            // Convert offset back to page for response (page from request, or computed from offset)
            let page: i64 = lua_page.unwrap_or_else(|| if limit > 0 { offset / limit + 1 } else { 1 }).max(1);

            // Build pagination table (camelCase, PayloadCMS-style)
            let pagination = lua.create_table()?;
            pagination.set("totalDocs", total)?;
            pagination.set("limit", limit)?;

            if pg_cursor {
                let (sort_col, sort_dir) = if let Some(ref order) = find_query.order_by {
                    if let Some(stripped) = order.strip_prefix('-') {
                        (stripped.to_string(), "DESC")
                    } else {
                        (order.clone(), "ASC")
                    }
                } else if def.timestamps {
                    ("created_at".to_string(), "DESC")
                } else {
                    ("id".to_string(), "ASC")
                };
                let (start_cursor, end_cursor) = query::cursor::build_cursors(
                    &docs, &sort_col, sort_dir,
                );
                let using_before = find_query.before_cursor.is_some();
                let has_cursor = find_query.after_cursor.is_some() || using_before;
                let at_limit = docs.len() as i64 >= limit && !docs.is_empty();
                let (has_next, has_prev) = if using_before {
                    (true, at_limit)
                } else {
                    (at_limit, has_cursor)
                };
                pagination.set("hasNextPage", has_next)?;
                pagination.set("hasPrevPage", has_prev)?;
                if let Some(sc) = start_cursor {
                    pagination.set("startCursor", sc)?;
                }
                if let Some(ec) = end_cursor {
                    pagination.set("endCursor", ec)?;
                }
            } else {
                let total_pages = if limit > 0 { (total + limit - 1) / limit } else { 0 };
                pagination.set("totalPages", total_pages)?;
                pagination.set("page", page)?;
                pagination.set("pageStart", offset + 1)?;
                pagination.set("hasNextPage", page < total_pages)?;
                pagination.set("hasPrevPage", page > 1)?;
                if page > 1 {
                    pagination.set("prevPage", page - 1)?;
                }
                if page < total_pages {
                    pagination.set("nextPage", page + 1)?;
                }
            }

            let result = find_result_to_lua(lua, &docs, pagination)?;

            Ok(result)
        })?;
        collections.set("find", find_fn)?;
    }

    // crap.collections.find_by_id(collection, id, opts?)
    // opts.depth (optional, default 0): populate relationship fields to this depth
    // opts.locale (optional): locale code or "all"
    // opts.draft (optional, default false): load draft version snapshot if available
    // opts.overrideAccess (optional, default true): bypass access control
    {
        let reg = registry.clone();
        let lc = locale_config.clone();
        let find_by_id_fn = lua.create_function(move |lua, (collection, id, opts): (String, String, Option<mlua::Table>)| {
            let conn_ptr = get_tx_conn(lua)?;
            let conn = unsafe { &*conn_ptr };

            let depth: i32 = opts.as_ref()
                .and_then(|o| o.get::<i32>("depth").ok())
                .unwrap_or(0)
                .clamp(0, 10);

            let locale_str: Option<String> = opts.as_ref()
                .and_then(|o| o.get::<Option<String>>("locale").ok().flatten());
            let locale_ctx = LocaleContext::from_locale_string(locale_str.as_deref(), &lc);

            let use_draft: bool = opts.as_ref()
                .and_then(|o| o.get::<Option<bool>>("draft").ok().flatten())
                .unwrap_or(false);

            let override_access: bool = opts.as_ref()
                .and_then(|o| o.get::<Option<bool>>("overrideAccess").ok().flatten())
                .unwrap_or(true);

            let def = {
                let r = reg.read().map_err(|e| mlua::Error::RuntimeError(
                    format!("Registry lock: {}", e)
                ))?;
                r.get_collection(&collection)
                    .cloned()
                    .ok_or_else(|| mlua::Error::RuntimeError(
                        format!("Collection '{}' not found", collection)
                    ))?
            };

            let select: Option<Vec<String>> = opts.as_ref()
                .and_then(|o| o.get::<mlua::Table>("select").ok())
                .map(|t| t.sequence_values::<String>().filter_map(|r| r.ok()).collect());

            // Fire before_read hooks (informational, no CRUD access needed)
            let before_ctx = HookContext {
                collection: collection.clone(),
                operation: "find_by_id".to_string(),
                data: HashMap::new(),
                locale: None,
                draft: None,
                context: HashMap::new(),
            };
            run_hooks_inner(lua, &def.hooks, HookEvent::BeforeRead, before_ctx)
                .map_err(|e| mlua::Error::RuntimeError(format!("before_read hook error: {}", e)))?;

            // Check access and determine constraints
            let access_constraints = if !override_access {
                let user_doc = lua.app_data_ref::<UserContext>()
                    .and_then(|uc| uc.0.clone());
                let result = check_access_with_lua(lua, def.access.read.as_deref(), user_doc.as_ref(), Some(&id), None)
                    .map_err(|e| mlua::Error::RuntimeError(format!("access check error: {}", e)))?;
                match result {
                    AccessResult::Denied => return Err(mlua::Error::RuntimeError("Read access denied".into())),
                    AccessResult::Constrained(extra) => Some(extra),
                    AccessResult::Allowed => None,
                }
            } else {
                None
            };

            // Unified find: draft overlay + constraints + hydration
            let mut doc = crate::db::ops::find_by_id_full(
                conn, &collection, &def, &id,
                locale_ctx.as_ref(), access_constraints, use_draft,
            ).map_err(|e| mlua::Error::RuntimeError(format!("find_by_id error: {}", e)))?;

            // Depth population, upload sizes, and select stripping (caller-specific)
            if let Some(ref mut d) = doc {
                let select_slice = select.as_deref();
                if depth > 0 {
                    let r = reg.read().map_err(|e| mlua::Error::RuntimeError(
                        format!("Registry lock: {}", e)
                    ))?;
                    let mut visited = std::collections::HashSet::new();
                    let pop_ctx = query::PopulateContext {
                        conn, registry: &r, collection_slug: &collection, def: &def,
                    };
                    let pop_opts = query::PopulateOpts {
                        depth, select: select_slice, locale_ctx: locale_ctx.as_ref(),
                    };
                    query::populate_relationships(
                        &pop_ctx, d, &mut visited, &pop_opts,
                    ).map_err(|e| mlua::Error::RuntimeError(format!("populate error: {}", e)))?;
                }
                // Assemble sizes for upload collections
                if let Some(ref upload_config) = def.upload {
                    if upload_config.enabled {
                        upload::assemble_sizes_object(d, upload_config);
                    }
                }
                if let Some(ref sel) = select {
                    query::apply_select_to_document(d, sel);
                }
            }

            // Field-level read stripping when overrideAccess = false
            if !override_access {
                if let Some(ref mut d) = doc {
                    let user_doc = lua.app_data_ref::<UserContext>()
                        .and_then(|uc| uc.0.clone());
                    let denied = check_field_read_access_with_lua(lua, &def.fields, user_doc.as_ref());
                    for name in &denied {
                        d.fields.remove(name);
                    }
                }
            }

            // Run after_read hooks
            let doc = doc.map(|d| apply_after_read_inner(lua, &def.hooks, &def.fields, &collection, "find_by_id", d));

            match doc {
                Some(d) => Ok(Value::Table(document_to_lua_table(lua, &d)?)),
                None => Ok(Value::Nil),
            }
        })?;
        collections.set("find_by_id", find_by_id_fn)?;
    }

    // crap.collections.create(collection, data, opts?)
    // opts.locale (optional): locale code to write to
    // opts.overrideAccess (optional, default true): bypass access control
    // opts.hooks (optional, default true): run lifecycle hooks
    // opts.draft (optional, default false): create as draft
    {
        let reg = registry.clone();
        let lc = locale_config.clone();
        let create_fn = lua.create_function(move |lua, (collection, data_table, opts): (String, mlua::Table, Option<mlua::Table>)| {
            let conn_ptr = get_tx_conn(lua)?;
            let conn = unsafe { &*conn_ptr };

            let locale_str: Option<String> = opts.as_ref()
                .and_then(|o| o.get::<Option<String>>("locale").ok().flatten());
            let locale_ctx = LocaleContext::from_locale_string(locale_str.as_deref(), &lc);

            let override_access: bool = opts.as_ref()
                .and_then(|o| o.get::<Option<bool>>("overrideAccess").ok().flatten())
                .unwrap_or(true);

            let run_hooks: bool = opts.as_ref()
                .and_then(|o| o.get::<Option<bool>>("hooks").ok().flatten())
                .unwrap_or(true);

            let def = {
                let r = reg.read().map_err(|e| mlua::Error::RuntimeError(
                    format!("Registry lock: {}", e)
                ))?;
                r.get_collection(&collection)
                    .cloned()
                    .ok_or_else(|| mlua::Error::RuntimeError(
                        format!("Collection '{}' not found", collection)
                    ))?
            };

            let mut data = lua_table_to_hashmap(&data_table)?;
            flatten_lua_groups(&data_table, &def.fields, &mut data)?;

            // Extract password for auth collections (before hooks/data flow)
            let password = if def.is_auth_collection() {
                data.remove("password")
            } else {
                None
            };

            // Enforce collection-level access control when overrideAccess = false
            if !override_access {
                let user_doc = lua.app_data_ref::<UserContext>()
                    .and_then(|uc| uc.0.clone());
                let result = check_access_with_lua(lua, def.access.create.as_deref(), user_doc.as_ref(), None, None)
                    .map_err(|e| mlua::Error::RuntimeError(format!("access check error: {}", e)))?;
                if matches!(result, AccessResult::Denied) {
                    return Err(mlua::Error::RuntimeError("Create access denied".into()));
                }
            }

            let draft: bool = opts.as_ref()
                .and_then(|o| o.get::<Option<bool>>("draft").ok().flatten())
                .unwrap_or(false);
            let is_draft = draft && def.has_drafts();

            // Check hook depth for recursion protection
            let current_depth = lua.app_data_ref::<HookDepth>().map(|d| d.0).unwrap_or(0);
            let max_depth = lua.app_data_ref::<MaxHookDepth>().map(|d| d.0).unwrap_or(3);
            let hooks_enabled = run_hooks && current_depth < max_depth;

            if run_hooks && current_depth >= max_depth {
                tracing::warn!(
                    "Hook depth {} reached max {}, skipping hooks for create on {}",
                    current_depth, max_depth, collection
                );
            }

            // Build hook data (JSON values for hooks to see)
            let mut hook_data: HashMap<String, serde_json::Value> = data.iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            let join_data = lua_table_to_json_map(lua, &data_table)?;
            for (k, v) in &join_data {
                hook_data.insert(k.clone(), v.clone());
            }
            // Ensure password doesn't leak into hooks via join_data
            if def.is_auth_collection() {
                hook_data.remove("password");
            }

            // Strip field-level write-denied fields AFTER hook_data is built
            // (must come after join_data merge to avoid re-adding stripped fields)
            if !override_access {
                let user_doc = lua.app_data_ref::<UserContext>()
                    .and_then(|uc| uc.0.clone());
                let denied = check_field_write_access_with_lua(lua, &def.fields, user_doc.as_ref(), "create");
                for name in &denied {
                    data.remove(name);
                    hook_data.remove(name);
                }
            }

            if hooks_enabled {
                // Increment depth
                lua.set_app_data(HookDepth(current_depth + 1));

                // Field-level before_validate
                run_field_hooks_inner(
                    lua, &def.fields, &FieldHookEvent::BeforeValidate,
                    &mut hook_data, &collection, "create",
                ).map_err(|e| mlua::Error::RuntimeError(format!("before_validate field hook error: {}", e)))?;

                // Collection-level before_validate
                let hook_ctx = HookContext {
                    collection: collection.clone(),
                    operation: "create".to_string(),
                    data: hook_data.clone(),
                    locale: locale_str.clone(),
                    draft: Some(is_draft),
                    context: HashMap::new(),
                };
                let ctx = run_hooks_inner(lua, &def.hooks, HookEvent::BeforeValidate, hook_ctx)
                    .map_err(|e| mlua::Error::RuntimeError(format!("before_validate hook error: {}", e)))?;
                hook_data = ctx.data;
            }

            // Validation (always runs unless hooks=false)
            if run_hooks {
                validate_fields_inner(lua, &def.fields, &hook_data, conn, &collection, None, is_draft)
                    .map_err(|e| mlua::Error::RuntimeError(format!("validation error: {}", e)))?;
            }

            if hooks_enabled {
                // Field-level before_change
                run_field_hooks_inner(
                    lua, &def.fields, &FieldHookEvent::BeforeChange,
                    &mut hook_data, &collection, "create",
                ).map_err(|e| mlua::Error::RuntimeError(format!("before_change field hook error: {}", e)))?;

                // Collection-level before_change
                let hook_ctx = HookContext {
                    collection: collection.clone(),
                    operation: "create".to_string(),
                    data: hook_data.clone(),
                    locale: locale_str.clone(),
                    draft: Some(is_draft),
                    context: HashMap::new(),
                };
                let ctx = run_hooks_inner(lua, &def.hooks, HookEvent::BeforeChange, hook_ctx)
                    .map_err(|e| mlua::Error::RuntimeError(format!("before_change hook error: {}", e)))?;
                hook_data = ctx.data;
            }

            // Convert hook-processed data back to string map for query
            let final_data = hook_ctx_to_string_map(
                &HookContext {
                    collection: collection.clone(),
                    operation: "create".to_string(),
                    data: hook_data.clone(),
                    locale: None,
                    draft: None,
                    context: HashMap::new(),
                },
                &def.fields,
            );

            let doc = crate::service::persist_create(
                conn, &collection, &def, &final_data, &hook_data,
                password.as_deref(), locale_ctx.as_ref(), is_draft,
            ).map_err(|e| mlua::Error::RuntimeError(format!("create error: {}", e)))?;

            // After-change hooks
            if hooks_enabled {
                // Field-level after_change
                let mut after_data = doc.fields.clone();
                run_field_hooks_inner(
                    lua, &def.fields, &FieldHookEvent::AfterChange,
                    &mut after_data, &collection, "create",
                ).map_err(|e| mlua::Error::RuntimeError(format!("after_change field hook error: {}", e)))?;

                // Collection-level after_change
                let after_ctx = HookContext {
                    collection: collection.clone(),
                    operation: "create".to_string(),
                    data: doc.fields.clone(),
                    locale: locale_str.clone(),
                    draft: Some(is_draft),
                    context: HashMap::new(),
                };
                run_hooks_inner(lua, &def.hooks, HookEvent::AfterChange, after_ctx)
                    .map_err(|e| mlua::Error::RuntimeError(format!("after_change hook error: {}", e)))?;

                // Restore depth
                lua.set_app_data(HookDepth(current_depth));
            }

            // Hydrate join-table fields before returning
            let mut doc = doc;
            query::hydrate_document(conn, &collection, &def.fields, &mut doc, None, locale_ctx.as_ref())
                .map_err(|e| mlua::Error::RuntimeError(format!("hydrate error: {}", e)))?;

            document_to_lua_table(lua, &doc)
        })?;
        collections.set("create", create_fn)?;
    }

    // crap.collections.update(collection, id, data, opts?)
    // opts.locale (optional): locale code to write to
    // opts.overrideAccess (optional, default true): bypass access control
    // opts.hooks (optional, default true): run lifecycle hooks
    // opts.draft (optional, default false): draft-only version save
    {
        let reg = registry.clone();
        let lc = locale_config.clone();
        let update_fn = lua.create_function(move |lua, (collection, id, data_table, opts): (String, String, mlua::Table, Option<mlua::Table>)| {
            let conn_ptr = get_tx_conn(lua)?;
            let conn = unsafe { &*conn_ptr };

            let locale_str: Option<String> = opts.as_ref()
                .and_then(|o| o.get::<Option<String>>("locale").ok().flatten());
            let locale_ctx = LocaleContext::from_locale_string(locale_str.as_deref(), &lc);

            let override_access: bool = opts.as_ref()
                .and_then(|o| o.get::<Option<bool>>("overrideAccess").ok().flatten())
                .unwrap_or(true);

            let run_hooks: bool = opts.as_ref()
                .and_then(|o| o.get::<Option<bool>>("hooks").ok().flatten())
                .unwrap_or(true);

            let def = {
                let r = reg.read().map_err(|e| mlua::Error::RuntimeError(
                    format!("Registry lock: {}", e)
                ))?;
                r.get_collection(&collection)
                    .cloned()
                    .ok_or_else(|| mlua::Error::RuntimeError(
                        format!("Collection '{}' not found", collection)
                    ))?
            };

            let mut data = lua_table_to_hashmap(&data_table)?;
            flatten_lua_groups(&data_table, &def.fields, &mut data)?;

            // Extract password for auth collections (before hooks/data flow)
            let password = if def.is_auth_collection() {
                data.remove("password")
            } else {
                None
            };

            // Read unpublish option
            let unpublish: bool = opts.as_ref()
                .and_then(|o| o.get::<Option<bool>>("unpublish").ok().flatten())
                .unwrap_or(false);

            // Enforce collection-level access control when overrideAccess = false
            if !override_access {
                let user_doc = lua.app_data_ref::<UserContext>()
                    .and_then(|uc| uc.0.clone());
                let result = check_access_with_lua(lua, def.access.update.as_deref(), user_doc.as_ref(), Some(&id), None)
                    .map_err(|e| mlua::Error::RuntimeError(format!("access check error: {}", e)))?;
                if matches!(result, AccessResult::Denied) {
                    return Err(mlua::Error::RuntimeError("Update access denied".into()));
                }
            }

            let draft: bool = opts.as_ref()
                .and_then(|o| o.get::<Option<bool>>("draft").ok().flatten())
                .unwrap_or(false);
            let is_draft = draft && def.has_drafts();

            // Check hook depth for recursion protection
            let current_depth = lua.app_data_ref::<HookDepth>().map(|d| d.0).unwrap_or(0);
            let max_depth = lua.app_data_ref::<MaxHookDepth>().map(|d| d.0).unwrap_or(3);
            let hooks_enabled = run_hooks && current_depth < max_depth;

            if run_hooks && current_depth >= max_depth {
                tracing::warn!(
                    "Hook depth {} reached max {}, skipping hooks for update on {}",
                    current_depth, max_depth, collection
                );
            }

            // Handle unpublish: set status to draft, create version, return
            if unpublish && def.has_versions() {
                let existing_doc = query::find_by_id_raw(conn, &collection, &def, &id, None)
                    .map_err(|e| mlua::Error::RuntimeError(format!("find error: {}", e)))?
                    .ok_or_else(|| mlua::Error::RuntimeError(
                        format!("Document {} not found in {}", id, collection)
                    ))?;

                // Check hook depth for recursion protection
                let current_depth = lua.app_data_ref::<HookDepth>().map(|d| d.0).unwrap_or(0);
                let max_depth = lua.app_data_ref::<MaxHookDepth>().map(|d| d.0).unwrap_or(3);
                let hooks_enabled = run_hooks && current_depth < max_depth;

                if hooks_enabled {
                    lua.set_app_data(HookDepth(current_depth + 1));
                    let before_ctx = HookContext {
                        collection: collection.clone(),
                        operation: "update".to_string(),
                        data: existing_doc.fields.clone(),
                        locale: locale_str.clone(),
                        draft: Some(false),
                        context: HashMap::new(),
                    };
                    run_hooks_inner(lua, &def.hooks, HookEvent::BeforeChange, before_ctx)
                        .map_err(|e| mlua::Error::RuntimeError(format!("before_change hook error: {}", e)))?;
                }

                crate::service::persist_unpublish(conn, &collection, &id, &def)
                    .map_err(|e| mlua::Error::RuntimeError(format!("unpublish error: {}", e)))?;

                if hooks_enabled {
                    let after_ctx = HookContext {
                        collection: collection.clone(),
                        operation: "update".to_string(),
                        data: existing_doc.fields.clone(),
                        locale: locale_str.clone(),
                        draft: Some(false),
                        context: HashMap::new(),
                    };
                    run_hooks_inner(lua, &def.hooks, HookEvent::AfterChange, after_ctx)
                        .map_err(|e| mlua::Error::RuntimeError(format!("after_change hook error: {}", e)))?;
                    lua.set_app_data(HookDepth(current_depth));
                }

                return document_to_lua_table(lua, &existing_doc);
            }

            // Build hook data (JSON values for hooks to see)
            let mut hook_data: HashMap<String, serde_json::Value> = data.iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            let join_data = lua_table_to_json_map(lua, &data_table)?;
            for (k, v) in &join_data {
                hook_data.insert(k.clone(), v.clone());
            }
            // Ensure password doesn't leak into hooks via join_data
            if def.is_auth_collection() {
                hook_data.remove("password");
            }

            // Strip field-level write-denied fields AFTER hook_data is built
            // (must come after join_data merge to avoid re-adding stripped fields)
            if !override_access {
                let user_doc = lua.app_data_ref::<UserContext>()
                    .and_then(|uc| uc.0.clone());
                let denied = check_field_write_access_with_lua(lua, &def.fields, user_doc.as_ref(), "update");
                for name in &denied {
                    data.remove(name);
                    hook_data.remove(name);
                }
            }

            if hooks_enabled {
                // Increment depth
                lua.set_app_data(HookDepth(current_depth + 1));

                // Field-level before_validate
                run_field_hooks_inner(
                    lua, &def.fields, &FieldHookEvent::BeforeValidate,
                    &mut hook_data, &collection, "update",
                ).map_err(|e| mlua::Error::RuntimeError(format!("before_validate field hook error: {}", e)))?;

                // Collection-level before_validate
                let hook_ctx = HookContext {
                    collection: collection.clone(),
                    operation: "update".to_string(),
                    data: hook_data.clone(),
                    locale: locale_str.clone(),
                    draft: Some(is_draft),
                    context: HashMap::new(),
                };
                let ctx = run_hooks_inner(lua, &def.hooks, HookEvent::BeforeValidate, hook_ctx)
                    .map_err(|e| mlua::Error::RuntimeError(format!("before_validate hook error: {}", e)))?;
                hook_data = ctx.data;
            }

            // Validation (always runs unless hooks=false)
            if run_hooks {
                validate_fields_inner(lua, &def.fields, &hook_data, conn, &collection, Some(&id), is_draft)
                    .map_err(|e| mlua::Error::RuntimeError(format!("validation error: {}", e)))?;
            }

            if hooks_enabled {
                // Field-level before_change
                run_field_hooks_inner(
                    lua, &def.fields, &FieldHookEvent::BeforeChange,
                    &mut hook_data, &collection, "update",
                ).map_err(|e| mlua::Error::RuntimeError(format!("before_change field hook error: {}", e)))?;

                // Collection-level before_change
                let hook_ctx = HookContext {
                    collection: collection.clone(),
                    operation: "update".to_string(),
                    data: hook_data.clone(),
                    locale: locale_str.clone(),
                    draft: Some(is_draft),
                    context: HashMap::new(),
                };
                let ctx = run_hooks_inner(lua, &def.hooks, HookEvent::BeforeChange, hook_ctx)
                    .map_err(|e| mlua::Error::RuntimeError(format!("before_change hook error: {}", e)))?;
                hook_data = ctx.data;
            }

            // Convert hook-processed data back to string map for query
            let final_data = hook_ctx_to_string_map(
                &HookContext {
                    collection: collection.clone(),
                    operation: "update".to_string(),
                    data: hook_data.clone(),
                    locale: None,
                    draft: None,
                    context: HashMap::new(),
                },
                &def.fields,
            );

            if is_draft && def.has_versions() {
                // Version-only save: do NOT update the main table.
                let existing_doc = crate::service::persist_draft_version(
                    conn, &collection, &id, &def, &hook_data, locale_ctx.as_ref(),
                ).map_err(|e| mlua::Error::RuntimeError(format!("draft version error: {}", e)))?;

                // After-change hooks (draft path)
                if hooks_enabled {
                    let after_ctx = HookContext {
                        collection: collection.clone(),
                        operation: "update".to_string(),
                        data: existing_doc.fields.clone(),
                        locale: locale_str.clone(),
                        draft: Some(is_draft),
                        context: HashMap::new(),
                    };
                    run_hooks_inner(lua, &def.hooks, HookEvent::AfterChange, after_ctx)
                        .map_err(|e| mlua::Error::RuntimeError(format!("after_change hook error: {}", e)))?;
                    lua.set_app_data(HookDepth(current_depth));
                }

                // Hydrate join-table fields before returning
                let mut existing_doc = existing_doc;
                query::hydrate_document(conn, &collection, &def.fields, &mut existing_doc, None, locale_ctx.as_ref())
                    .map_err(|e| mlua::Error::RuntimeError(format!("hydrate error: {}", e)))?;

                document_to_lua_table(lua, &existing_doc)
            } else {
                // Normal update: write to main table
                let doc = crate::service::persist_update(
                    conn, &collection, &id, &def, &final_data, &hook_data,
                    password.as_deref(), locale_ctx.as_ref(),
                ).map_err(|e| mlua::Error::RuntimeError(format!("update error: {}", e)))?;

                // After-change hooks
                if hooks_enabled {
                    let mut after_data = doc.fields.clone();
                    run_field_hooks_inner(
                        lua, &def.fields, &FieldHookEvent::AfterChange,
                        &mut after_data, &collection, "update",
                    ).map_err(|e| mlua::Error::RuntimeError(format!("after_change field hook error: {}", e)))?;

                    let after_ctx = HookContext {
                        collection: collection.clone(),
                        operation: "update".to_string(),
                        data: doc.fields.clone(),
                        locale: locale_str.clone(),
                        draft: Some(is_draft),
                        context: HashMap::new(),
                    };
                    run_hooks_inner(lua, &def.hooks, HookEvent::AfterChange, after_ctx)
                        .map_err(|e| mlua::Error::RuntimeError(format!("after_change hook error: {}", e)))?;
                    lua.set_app_data(HookDepth(current_depth));
                }

                // Hydrate join-table fields before returning
                let mut doc = doc;
                query::hydrate_document(conn, &collection, &def.fields, &mut doc, None, locale_ctx.as_ref())
                    .map_err(|e| mlua::Error::RuntimeError(format!("hydrate error: {}", e)))?;

                document_to_lua_table(lua, &doc)
            }
        })?;
        collections.set("update", update_fn)?;
    }

    // crap.collections.delete(collection, id, opts?)
    // opts.overrideAccess (optional, default true): bypass access control
    // opts.hooks (optional, default true): run lifecycle hooks
    {
        let reg = registry.clone();
        let delete_fn = lua.create_function(move |lua, (collection, id, opts): (String, String, Option<mlua::Table>)| {
            let conn_ptr = get_tx_conn(lua)?;
            let conn = unsafe { &*conn_ptr };

            let override_access: bool = opts.as_ref()
                .and_then(|o| o.get::<Option<bool>>("overrideAccess").ok().flatten())
                .unwrap_or(true);

            let run_hooks: bool = opts.as_ref()
                .and_then(|o| o.get::<Option<bool>>("hooks").ok().flatten())
                .unwrap_or(true);

            let def = {
                let r = reg.read().map_err(|e| mlua::Error::RuntimeError(
                    format!("Registry lock: {}", e)
                ))?;
                r.get_collection(&collection)
                    .cloned()
                    .ok_or_else(|| mlua::Error::RuntimeError(
                        format!("Collection '{}' not found", collection)
                    ))?
            };

            // Enforce access control when overrideAccess = false
            if !override_access {
                let user_doc = lua.app_data_ref::<UserContext>()
                    .and_then(|uc| uc.0.clone());
                let result = check_access_with_lua(lua, def.access.delete.as_deref(), user_doc.as_ref(), Some(&id), None)
                    .map_err(|e| mlua::Error::RuntimeError(format!("access check error: {}", e)))?;
                if matches!(result, AccessResult::Denied) {
                    return Err(mlua::Error::RuntimeError("Delete access denied".into()));
                }
            }

            // Check hook depth for recursion protection
            let current_depth = lua.app_data_ref::<HookDepth>().map(|d| d.0).unwrap_or(0);
            let max_depth = lua.app_data_ref::<MaxHookDepth>().map(|d| d.0).unwrap_or(3);
            let hooks_enabled = run_hooks && current_depth < max_depth;

            if run_hooks && current_depth >= max_depth {
                tracing::warn!(
                    "Hook depth {} reached max {}, skipping hooks for delete on {}",
                    current_depth, max_depth, collection
                );
            }

            if hooks_enabled {
                lua.set_app_data(HookDepth(current_depth + 1));

                let hook_ctx = HookContext {
                    collection: collection.clone(),
                    operation: "delete".to_string(),
                    data: [("id".to_string(), serde_json::Value::String(id.clone()))].into(),
                    locale: None,
                    draft: None,
                    context: HashMap::new(),
                };
                run_hooks_inner(lua, &def.hooks, HookEvent::BeforeDelete, hook_ctx)
                    .map_err(|e| mlua::Error::RuntimeError(format!("before_delete hook error: {}", e)))?;
            }

            query::delete(conn, &collection, &id)
                .map_err(|e| mlua::Error::RuntimeError(format!("delete error: {}", e)))?;

            // Sync FTS index
            query::fts::fts_delete(conn, &collection, &id)
                .map_err(|e| mlua::Error::RuntimeError(format!("FTS delete error: {}", e)))?;

            if hooks_enabled {
                let after_ctx = HookContext {
                    collection: collection.clone(),
                    operation: "delete".to_string(),
                    data: [("id".to_string(), serde_json::Value::String(id.clone()))].into(),
                    locale: None,
                    draft: None,
                    context: HashMap::new(),
                };
                run_hooks_inner(lua, &def.hooks, HookEvent::AfterDelete, after_ctx)
                    .map_err(|e| mlua::Error::RuntimeError(format!("after_delete hook error: {}", e)))?;

                lua.set_app_data(HookDepth(current_depth));
            }

            Ok(true)
        })?;
        collections.set("delete", delete_fn)?;
    }

    // crap.collections.count(collection, query?)
    // query.locale (optional): locale code or "all"
    // query.overrideAccess (optional, default true): bypass access control
    {
        let reg = registry.clone();
        let lc = locale_config.clone();
        let count_fn = lua.create_function(move |lua, (collection, query_table): (String, Option<mlua::Table>)| {
            let conn_ptr = get_tx_conn(lua)?;
            let conn = unsafe { &*conn_ptr };

            let locale_str: Option<String> = query_table.as_ref()
                .and_then(|qt| qt.get::<Option<String>>("locale").ok().flatten());
            let locale_ctx = LocaleContext::from_locale_string(locale_str.as_deref(), &lc);

            let override_access: bool = query_table.as_ref()
                .and_then(|qt| qt.get::<Option<bool>>("overrideAccess").ok().flatten())
                .unwrap_or(true);

            let def = {
                let r = reg.read().map_err(|e| mlua::Error::RuntimeError(
                    format!("Registry lock: {}", e)
                ))?;
                r.get_collection(&collection)
                    .cloned()
                    .ok_or_else(|| mlua::Error::RuntimeError(
                        format!("Collection '{}' not found", collection)
                    ))?
            };

            let draft: bool = query_table.as_ref()
                .and_then(|qt| qt.get::<Option<bool>>("draft").ok().flatten())
                .unwrap_or(false);

            let (find_query, _) = match query_table {
                Some(ref qt) => (lua_table_to_find_query(qt)?.0, true),
                None => (query::FindQuery::default(), false),
            };
            let mut filters = find_query.filters;
            let search = find_query.search;

            normalize_filter_fields(&mut filters, &def.fields);

            // Draft-aware filtering (matches gRPC behavior)
            if def.has_drafts() && !draft {
                filters.push(FilterClause::Single(Filter {
                    field: "_status".to_string(),
                    op: FilterOp::Equals("published".to_string()),
                }));
            }

            // Enforce access control when overrideAccess = false
            if !override_access {
                let user_doc = lua.app_data_ref::<UserContext>()
                    .and_then(|uc| uc.0.clone());
                let result = check_access_with_lua(lua, def.access.read.as_deref(), user_doc.as_ref(), None, None)
                    .map_err(|e| mlua::Error::RuntimeError(format!("access check error: {}", e)))?;
                match result {
                    AccessResult::Denied => return Err(mlua::Error::RuntimeError("Read access denied".into())),
                    AccessResult::Constrained(extra) => filters.extend(extra),
                    AccessResult::Allowed => {}
                }
            }

            let count = query::count_with_search(conn, &collection, &def, &filters, locale_ctx.as_ref(), search.as_deref())
                .map_err(|e| mlua::Error::RuntimeError(format!("count error: {}", e)))?;

            Ok(count)
        })?;
        collections.set("count", count_fn)?;
    }

    // crap.collections.update_many(collection, query, data, opts?)
    // Raw bulk update: finds matching docs, checks access, updates each. No per-doc hooks.
    // Returns { modified = N }
    {
        let reg = registry.clone();
        let lc = locale_config.clone();
        let update_many_fn = lua.create_function(move |lua, (collection, query_table, data_table, opts): (String, mlua::Table, mlua::Table, Option<mlua::Table>)| {
            let conn_ptr = get_tx_conn(lua)?;
            let conn = unsafe { &*conn_ptr };

            let locale_str: Option<String> = opts.as_ref()
                .and_then(|o| o.get::<Option<String>>("locale").ok().flatten());
            let locale_ctx = LocaleContext::from_locale_string(locale_str.as_deref(), &lc);

            let override_access: bool = opts.as_ref()
                .and_then(|o| o.get::<Option<bool>>("overrideAccess").ok().flatten())
                .unwrap_or(true);

            let def = {
                let r = reg.read().map_err(|e| mlua::Error::RuntimeError(
                    format!("Registry lock: {}", e)
                ))?;
                r.get_collection(&collection)
                    .cloned()
                    .ok_or_else(|| mlua::Error::RuntimeError(
                        format!("Collection '{}' not found", collection)
                    ))?
            };

            let draft: bool = opts.as_ref()
                .and_then(|o| o.get::<Option<bool>>("draft").ok().flatten())
                .unwrap_or(false);

            let (mut find_query, _) = lua_table_to_find_query(&query_table)?;
            normalize_filter_fields(&mut find_query.filters, &def.fields);

            // Draft-aware filtering (matches gRPC behavior)
            if def.has_drafts() && !draft {
                find_query.filters.push(FilterClause::Single(Filter {
                    field: "_status".to_string(),
                    op: FilterOp::Equals("published".to_string()),
                }));
            }

            // Find all matching docs first
            if !override_access {
                let user_doc = lua.app_data_ref::<UserContext>()
                    .and_then(|uc| uc.0.clone());
                let result = check_access_with_lua(lua, def.access.read.as_deref(), user_doc.as_ref(), None, None)
                    .map_err(|e| mlua::Error::RuntimeError(format!("access check error: {}", e)))?;
                match result {
                    AccessResult::Denied => return Err(mlua::Error::RuntimeError("Read access denied".into())),
                    AccessResult::Constrained(extra) => find_query.filters.extend(extra),
                    AccessResult::Allowed => {}
                }
            }

            // Remove limit/offset to get all matching docs
            let find_all = FindQuery { filters: find_query.filters, ..Default::default() };
            let docs = query::find(conn, &collection, &def, &find_all, locale_ctx.as_ref())
                .map_err(|e| mlua::Error::RuntimeError(format!("find error: {}", e)))?;

            // Check per-doc update access (all-or-nothing)
            if !override_access {
                let user_doc = lua.app_data_ref::<UserContext>()
                    .and_then(|uc| uc.0.clone());
                for doc in &docs {
                    let result = check_access_with_lua(lua, def.access.update.as_deref(), user_doc.as_ref(), Some(&doc.id), None)
                        .map_err(|e| mlua::Error::RuntimeError(format!("access check error: {}", e)))?;
                    if matches!(result, AccessResult::Denied) {
                        return Err(mlua::Error::RuntimeError(
                            format!("Update access denied for document {}", doc.id)
                        ));
                    }
                }
            }

            let data = lua_table_to_hashmap(&data_table)?;
            let join_data = lua_table_to_json_map(lua, &data_table)?;
            let mut modified = 0i64;

            for doc in &docs {
                let updated = query::update(conn, &collection, &def, &doc.id, &data, locale_ctx.as_ref())
                    .map_err(|e| mlua::Error::RuntimeError(format!("update error: {}", e)))?;
                query::save_join_table_data(conn, &collection, &def.fields, &doc.id, &join_data, locale_ctx.as_ref())
                    .map_err(|e| mlua::Error::RuntimeError(format!("join data error: {}", e)))?;
                query::fts::fts_upsert(conn, &collection, &updated, Some(&def))
                    .map_err(|e| mlua::Error::RuntimeError(format!("FTS upsert error: {}", e)))?;
                modified += 1;
            }

            let result = lua.create_table()?;
            result.set("modified", modified)?;
            Ok(result)
        })?;
        collections.set("update_many", update_many_fn)?;
    }

    // crap.collections.delete_many(collection, query, opts?)
    // Raw bulk delete: finds matching docs, checks access, deletes each. No per-doc hooks.
    // Returns { deleted = N }
    {
        let reg = registry.clone();
        let lc = locale_config.clone();
        let delete_many_fn = lua.create_function(move |lua, (collection, query_table, opts): (String, mlua::Table, Option<mlua::Table>)| {
            let conn_ptr = get_tx_conn(lua)?;
            let conn = unsafe { &*conn_ptr };

            let override_access: bool = opts.as_ref()
                .and_then(|o| o.get::<Option<bool>>("overrideAccess").ok().flatten())
                .unwrap_or(true);

            let locale_str: Option<String> = opts.as_ref()
                .and_then(|o| o.get::<Option<String>>("locale").ok().flatten());
            let locale_ctx = LocaleContext::from_locale_string(locale_str.as_deref(), &lc);

            let def = {
                let r = reg.read().map_err(|e| mlua::Error::RuntimeError(
                    format!("Registry lock: {}", e)
                ))?;
                r.get_collection(&collection)
                    .cloned()
                    .ok_or_else(|| mlua::Error::RuntimeError(
                        format!("Collection '{}' not found", collection)
                    ))?
            };

            let draft: bool = opts.as_ref()
                .and_then(|o| o.get::<Option<bool>>("draft").ok().flatten())
                .unwrap_or(false);

            let (mut find_query, _) = lua_table_to_find_query(&query_table)?;
            normalize_filter_fields(&mut find_query.filters, &def.fields);

            // Draft-aware filtering (matches gRPC behavior)
            if def.has_drafts() && !draft {
                find_query.filters.push(FilterClause::Single(Filter {
                    field: "_status".to_string(),
                    op: FilterOp::Equals("published".to_string()),
                }));
            }

            if !override_access {
                let user_doc = lua.app_data_ref::<UserContext>()
                    .and_then(|uc| uc.0.clone());
                let result = check_access_with_lua(lua, def.access.read.as_deref(), user_doc.as_ref(), None, None)
                    .map_err(|e| mlua::Error::RuntimeError(format!("access check error: {}", e)))?;
                match result {
                    AccessResult::Denied => return Err(mlua::Error::RuntimeError("Read access denied".into())),
                    AccessResult::Constrained(extra) => find_query.filters.extend(extra),
                    AccessResult::Allowed => {}
                }
            }

            let find_all = FindQuery { filters: find_query.filters, ..Default::default() };
            let docs = query::find(conn, &collection, &def, &find_all, locale_ctx.as_ref())
                .map_err(|e| mlua::Error::RuntimeError(format!("find error: {}", e)))?;

            // Check per-doc delete access (all-or-nothing)
            if !override_access {
                let user_doc = lua.app_data_ref::<UserContext>()
                    .and_then(|uc| uc.0.clone());
                for doc in &docs {
                    let result = check_access_with_lua(lua, def.access.delete.as_deref(), user_doc.as_ref(), Some(&doc.id), None)
                        .map_err(|e| mlua::Error::RuntimeError(format!("access check error: {}", e)))?;
                    if matches!(result, AccessResult::Denied) {
                        return Err(mlua::Error::RuntimeError(
                            format!("Delete access denied for document {}", doc.id)
                        ));
                    }
                }
            }

            let mut deleted = 0i64;
            for doc in &docs {
                query::delete(conn, &collection, &doc.id)
                    .map_err(|e| mlua::Error::RuntimeError(format!("delete error: {}", e)))?;
                query::fts::fts_delete(conn, &collection, &doc.id)
                    .map_err(|e| mlua::Error::RuntimeError(format!("FTS delete error: {}", e)))?;
                deleted += 1;
            }

            let result = lua.create_table()?;
            result.set("deleted", deleted)?;
            Ok(result)
        })?;
        collections.set("delete_many", delete_many_fn)?;
    }

    // ── Globals CRUD ─────────────────────────────────────────────────────────

    let globals: mlua::Table = crap.get("globals")?;

    // crap.globals.get(slug, opts?)
    // opts.locale (optional): locale code or "all"
    {
        let reg = registry.clone();
        let lc = locale_config.clone();
        let get_fn = lua.create_function(move |lua, (slug, opts): (String, Option<mlua::Table>)| {
            let conn_ptr = get_tx_conn(lua)?;
            let conn = unsafe { &*conn_ptr };

            let locale_str: Option<String> = opts.as_ref()
                .and_then(|o| o.get::<Option<String>>("locale").ok().flatten());
            let locale_ctx = LocaleContext::from_locale_string(locale_str.as_deref(), &lc);

            let def = {
                let r = reg.read().map_err(|e| mlua::Error::RuntimeError(
                    format!("Registry lock: {}", e)
                ))?;
                r.get_global(&slug)
                    .cloned()
                    .ok_or_else(|| mlua::Error::RuntimeError(
                        format!("Global '{}' not found", slug)
                    ))?
            };

            let doc = query::get_global(conn, &slug, &def, locale_ctx.as_ref())
                .map_err(|e| mlua::Error::RuntimeError(format!("get_global error: {}", e)))?;

            document_to_lua_table(lua, &doc)
        })?;
        globals.set("get", get_fn)?;
    }

    // crap.globals.update(slug, data, opts?)
    // opts.locale (optional): locale code to write to
    {
        let reg = registry.clone();
        let lc = locale_config.clone();
        let update_fn = lua.create_function(move |lua, (slug, data_table, opts): (String, mlua::Table, Option<mlua::Table>)| {
            let conn_ptr = get_tx_conn(lua)?;
            let conn = unsafe { &*conn_ptr };

            let locale_str: Option<String> = opts.as_ref()
                .and_then(|o| o.get::<Option<String>>("locale").ok().flatten());
            let locale_ctx = LocaleContext::from_locale_string(locale_str.as_deref(), &lc);

            let def = {
                let r = reg.read().map_err(|e| mlua::Error::RuntimeError(
                    format!("Registry lock: {}", e)
                ))?;
                r.get_global(&slug)
                    .cloned()
                    .ok_or_else(|| mlua::Error::RuntimeError(
                        format!("Global '{}' not found", slug)
                    ))?
            };

            let data = lua_table_to_hashmap(&data_table)?;
            let doc = query::update_global(conn, &slug, &def, &data, locale_ctx.as_ref())
                .map_err(|e| mlua::Error::RuntimeError(format!("update_global error: {}", e)))?;

            document_to_lua_table(lua, &doc)
        })?;
        globals.set("update", update_fn)?;
    }

    // ── Jobs queue ──────────────────────────────────────────────────────────

    // crap.jobs.queue(slug, data?) — insert a pending job row
    {
        let reg = registry.clone();
        let queue_fn = lua.create_function(move |lua, (slug, data): (String, Option<mlua::Table>)| {
            let conn_ptr = get_tx_conn(lua)?;
            let conn = unsafe { &*conn_ptr };

            // Verify job exists in registry
            let job_def = {
                let r = reg.read().map_err(|e| mlua::Error::RuntimeError(
                    format!("Registry lock: {}", e)
                ))?;
                r.get_job(&slug)
                    .cloned()
                    .ok_or_else(|| mlua::Error::RuntimeError(
                        format!("Job '{}' not defined", slug)
                    ))?
            };

            let data_json = match data {
                Some(tbl) => {
                    let json_val = crate::hooks::api::lua_to_json(lua, &Value::Table(tbl))?;
                    serde_json::to_string(&json_val)
                        .map_err(|e| mlua::Error::RuntimeError(format!("JSON error: {}", e)))?
                }
                None => "{}".to_string(),
            };

            let job_run = crate::db::query::jobs::insert_job(
                conn,
                &slug,
                &data_json,
                "hook",
                job_def.retries + 1,
                &job_def.queue,
            ).map_err(|e| mlua::Error::RuntimeError(format!("queue error: {}", e)))?;

            Ok(job_run.id)
        })?;

        let jobs: mlua::Table = crap.get("jobs")?;
        jobs.set("queue", queue_fn)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;

    // --- get_tx_conn tests ---

    #[test]
    fn test_get_tx_conn_without_context() {
        let lua = Lua::new();
        let result = get_tx_conn(&lua);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("only available inside hooks"));
    }
}
