//! DB-access enrichment for field contexts (relationship options, array rows, upload thumbnails).

use std::collections::HashMap;
use crate::core::field::FieldType;
use super::{safe_template_id, count_errors_in_fields, MAX_FIELD_DEPTH};
use super::super::shared::auto_label_from_name;
use super::builder::{build_single_field_context, apply_field_type_extras};

/// Build a sub-field context for a single field within an array/blocks row,
/// recursively handling nested composite sub-fields.
///
/// Build enriched child field contexts from structured JSON data.
/// Used by layout wrapper handlers (Tabs/Row/Collapsible) inside Array/Blocks
/// rows to correctly propagate structured data to nested layout wrappers.
///
/// For each child field:
/// - Layout wrappers get transparent names and the whole parent data object
/// - Leaf fields get `parent_name[field_name]` names and their specific value
/// - Recursion handles arbitrary nesting depth (Row inside Tabs inside Array, etc.)
pub fn build_enriched_children_from_data(
    fields: &[crate::core::field::FieldDefinition],
    data: Option<&serde_json::Value>,
    parent_name: &str,
    locale_locked: bool,
    non_default_locale: bool,
    depth: usize,
    errors: &HashMap<String, String>,
) -> Vec<serde_json::Value> {
    if depth >= MAX_FIELD_DEPTH { return Vec::new(); }

    let data_obj = data.and_then(|v| v.as_object());

    fields.iter().map(|child| {
        let is_wrapper = matches!(child.field_type,
            FieldType::Tabs | FieldType::Row | FieldType::Collapsible);

        let child_raw = if is_wrapper {
            data // pass whole object
        } else {
            data_obj.and_then(|m| m.get(&child.name))
        };

        let child_name = if is_wrapper {
            parent_name.to_string() // transparent
        } else {
            format!("{}[{}]", parent_name, child.name)
        };

        let child_val = child_raw
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Null => String::new(),
                other => {
                    if is_wrapper {
                        String::new()
                    } else {
                        other.to_string()
                    }
                }
            })
            .unwrap_or_default();

        let child_label = child.admin.label.as_ref()
            .map(|ls| ls.resolve_default().to_string())
            .unwrap_or_else(|| auto_label_from_name(&child.name));

        let mut child_ctx = serde_json::json!({
            "name": child_name,
            "field_type": child.field_type.as_str(),
            "label": child_label,
            "value": child_val,
            "required": child.required,
            "readonly": child.admin.readonly || locale_locked,
            "locale_locked": locale_locked,
            "placeholder": child.admin.placeholder.as_ref().map(|ls| ls.resolve_default()),
            "description": child.admin.description.as_ref().map(|ls| ls.resolve_default()),
        });

        if let Some(err) = errors.get(&child_name) {
            child_ctx["error"] = serde_json::json!(err);
        }

        match child.field_type {
            FieldType::Row | FieldType::Collapsible => {
                let sub_fields = build_enriched_children_from_data(
                    &child.fields, child_raw, &child_name,
                    locale_locked, non_default_locale, depth + 1, errors,
                );
                child_ctx["sub_fields"] = serde_json::json!(sub_fields);
                if child.field_type == FieldType::Collapsible {
                    child_ctx["collapsed"] = serde_json::json!(child.admin.collapsed);
                }
            }
            FieldType::Tabs => {
                let tabs_ctx: Vec<_> = child.tabs.iter().map(|tab| {
                    let tab_sub_fields = build_enriched_children_from_data(
                        &tab.fields, child_raw, &child_name,
                        locale_locked, non_default_locale, depth + 1, errors,
                    );
                    let error_count = count_errors_in_fields(&tab_sub_fields);
                    let mut tab_ctx = serde_json::json!({
                        "label": &tab.label,
                        "sub_fields": tab_sub_fields,
                    });
                    if error_count > 0 {
                        tab_ctx["error_count"] = serde_json::json!(error_count);
                    }
                    if let Some(ref desc) = tab.description {
                        tab_ctx["description"] = serde_json::json!(desc);
                    }
                    tab_ctx
                }).collect();
                child_ctx["tabs"] = serde_json::json!(tabs_ctx);
            }
            _ => {
                apply_field_type_extras(
                    child, &child_val, &mut child_ctx,
                    &HashMap::new(), errors, &child_name,
                    non_default_locale, depth + 1,
                );
            }
        }

        child_ctx
    }).collect()
}

/// `sf`: the sub-field definition
/// `raw_value`: the raw JSON value for this sub-field from the hydrated document
/// `parent_name`: the parent field's name (e.g. "content")
/// `idx`: the row index within the parent
/// `locale_locked`: whether the parent is locale-locked
/// `non_default_locale`: whether we're on a non-default locale
/// `depth`: nesting depth
pub fn build_enriched_sub_field_context(
    sf: &crate::core::field::FieldDefinition,
    raw_value: Option<&serde_json::Value>,
    parent_name: &str,
    idx: usize,
    locale_locked: bool,
    non_default_locale: bool,
    depth: usize,
    errors: &HashMap<String, String>,
) -> serde_json::Value {
    let indexed_name = if matches!(sf.field_type, FieldType::Tabs | FieldType::Row | FieldType::Collapsible) {
        format!("{}[{}]", parent_name, idx) // transparent — layout wrappers don't add their name
    } else {
        format!("{}[{}][{}]", parent_name, idx, sf.name)
    };

    // For scalar types, stringify the value. For composites, keep structured.
    let val = raw_value
        .map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => String::new(),
            other => {
                match sf.field_type {
                    FieldType::Array | FieldType::Blocks | FieldType::Group | FieldType::Row | FieldType::Collapsible | FieldType::Tabs => String::new(),
                    _ => other.to_string(),
                }
            }
        })
        .unwrap_or_default();

    let sf_label = sf.admin.label.as_ref()
        .map(|ls| ls.resolve_default().to_string())
        .unwrap_or_else(|| auto_label_from_name(&sf.name));

    let mut sub_ctx = serde_json::json!({
        "name": indexed_name,
        "field_type": sf.field_type.as_str(),
        "label": sf_label,
        "value": val,
        "required": sf.required,
        "readonly": sf.admin.readonly || locale_locked,
        "locale_locked": locale_locked,
        "placeholder": sf.admin.placeholder.as_ref().map(|ls| ls.resolve_default()),
        "description": sf.admin.description.as_ref().map(|ls| ls.resolve_default()),
    });

    if let Some(err) = errors.get(&indexed_name) {
        sub_ctx["error"] = serde_json::json!(err);
    }

    if depth >= MAX_FIELD_DEPTH { return sub_ctx; }

    match &sf.field_type {
        FieldType::Checkbox => {
            let checked = matches!(val.as_str(), "1" | "true" | "on" | "yes");
            sub_ctx["checked"] = serde_json::json!(checked);
        }
        FieldType::Select | FieldType::Radio => {
            if sf.has_many {
                let selected_values: std::collections::HashSet<String> =
                    serde_json::from_str(&val)
                        .unwrap_or_default();
                let options: Vec<_> = sf.options.iter().map(|opt| {
                    serde_json::json!({
                        "label": opt.label.resolve_default(),
                        "value": opt.value,
                        "selected": selected_values.contains(&opt.value),
                    })
                }).collect();
                sub_ctx["options"] = serde_json::json!(options);
                sub_ctx["has_many"] = serde_json::json!(true);
            } else {
                let options: Vec<_> = sf.options.iter().map(|opt| {
                    serde_json::json!({
                        "label": opt.label.resolve_default(),
                        "value": opt.value,
                        "selected": opt.value == val,
                    })
                }).collect();
                sub_ctx["options"] = serde_json::json!(options);
            }
        }
        FieldType::Date => {
            let appearance = sf.picker_appearance.as_deref().unwrap_or("dayOnly");
            sub_ctx["picker_appearance"] = serde_json::json!(appearance);
            match appearance {
                "dayOnly" => {
                    let date_val = if val.len() >= 10 { &val[..10] } else { &val };
                    sub_ctx["date_only_value"] = serde_json::json!(date_val);
                }
                "dayAndTime" => {
                    let dt_val = if val.len() >= 16 { &val[..16] } else { &val };
                    sub_ctx["datetime_local_value"] = serde_json::json!(dt_val);
                }
                _ => {}
            }
        }
        FieldType::Relationship => {
            if let Some(ref rc) = sf.relationship {
                sub_ctx["relationship_collection"] = serde_json::json!(rc.collection);
                sub_ctx["has_many"] = serde_json::json!(rc.has_many);
                if rc.is_polymorphic() {
                    sub_ctx["polymorphic"] = serde_json::json!(true);
                    sub_ctx["collections"] = serde_json::json!(rc.polymorphic);
                }
            }
            if let Some(ref p) = sf.admin.picker {
                sub_ctx["picker"] = serde_json::json!(p);
            }
        }
        FieldType::Upload => {
            if let Some(ref rc) = sf.relationship {
                sub_ctx["relationship_collection"] = serde_json::json!(rc.collection);
                if rc.has_many {
                    sub_ctx["has_many"] = serde_json::json!(true);
                }
            }
            let picker = sf.admin.picker.as_deref().unwrap_or("drawer");
            if picker != "none" {
                sub_ctx["picker"] = serde_json::json!(picker);
            }
        }
        FieldType::Array => {
            // Nested array: recurse into sub-rows
            let nested_rows: Vec<serde_json::Value> = match raw_value {
                Some(serde_json::Value::Array(arr)) => {
                    arr.iter().enumerate().map(|(nested_idx, nested_row)| {
                        let nested_row_obj = nested_row.as_object();
                        let nested_sub_values: Vec<_> = sf.fields.iter().map(|nested_sf| {
                            let nested_raw = if matches!(nested_sf.field_type,
                                FieldType::Tabs | FieldType::Row | FieldType::Collapsible)
                            {
                                Some(nested_row) // pass whole row — data is stored flat
                            } else {
                                nested_row_obj.and_then(|m| m.get(&nested_sf.name))
                            };
                            build_enriched_sub_field_context(
                                nested_sf, nested_raw, &indexed_name, nested_idx,
                                locale_locked, non_default_locale, depth + 1, errors,
                            )
                        }).collect();
                        let row_has_errors = nested_sub_values.iter()
                            .any(|sf_ctx| sf_ctx.get("error").is_some());
                        let mut row_json = serde_json::json!({
                            "index": nested_idx,
                            "sub_fields": nested_sub_values,
                        });
                        if row_has_errors {
                            row_json["has_errors"] = serde_json::json!(true);
                        }
                        row_json
                    }).collect()
                }
                _ => Vec::new(),
            };
            // Template sub_fields for the nested <template> section
            let template_prefix = format!("{}[__INDEX__]", indexed_name);
            let template_sub_fields: Vec<_> = sf.fields.iter().map(|nested_sf| {
                build_single_field_context(nested_sf, &HashMap::new(), &HashMap::new(), &template_prefix, non_default_locale, depth + 1)
            }).collect();
            sub_ctx["sub_fields"] = serde_json::json!(template_sub_fields);
            sub_ctx["rows"] = serde_json::json!(nested_rows);
            sub_ctx["row_count"] = serde_json::json!(nested_rows.len());
            sub_ctx["template_id"] = serde_json::json!(safe_template_id(&indexed_name));
            if let Some(max) = sf.max_rows {
                sub_ctx["max_rows"] = serde_json::json!(max);
            }
            if let Some(min) = sf.min_rows {
                sub_ctx["min_rows"] = serde_json::json!(min);
            }
            sub_ctx["init_collapsed"] = serde_json::json!(sf.admin.collapsed);
            if let Some(ref ls) = sf.admin.labels_singular {
                sub_ctx["add_label"] = serde_json::json!(ls.resolve_default());
            }
        }
        FieldType::Blocks => {
            // Nested blocks: recurse into block rows
            let nested_rows: Vec<serde_json::Value> = match raw_value {
                Some(serde_json::Value::Array(arr)) => {
                    arr.iter().enumerate().map(|(nested_idx, nested_row)| {
                        let nested_row_obj = nested_row.as_object();
                        let block_type = nested_row_obj
                            .and_then(|m| m.get("_block_type"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let block_label = sf.blocks.iter()
                            .find(|bd| bd.block_type == block_type)
                            .and_then(|bd| bd.label.as_ref().map(|ls| ls.resolve_default()))
                            .unwrap_or(block_type);
                        let block_def = sf.blocks.iter().find(|bd| bd.block_type == block_type);
                        let nested_sub_values: Vec<_> = block_def
                            .map(|bd| bd.fields.iter().map(|nested_sf| {
                                let nested_raw = if matches!(nested_sf.field_type,
                                    FieldType::Tabs | FieldType::Row | FieldType::Collapsible)
                                {
                                    Some(nested_row) // pass whole block data object
                                } else {
                                    nested_row_obj.and_then(|m| m.get(&nested_sf.name))
                                };
                                build_enriched_sub_field_context(
                                    nested_sf, nested_raw, &indexed_name, nested_idx,
                                    locale_locked, non_default_locale, depth + 1, errors,
                                )
                            }).collect())
                            .unwrap_or_default();
                        let row_has_errors = nested_sub_values.iter()
                            .any(|sf_ctx| sf_ctx.get("error").is_some());
                        let mut row_json = serde_json::json!({
                            "index": nested_idx,
                            "_block_type": block_type,
                            "block_label": block_label,
                            "sub_fields": nested_sub_values,
                        });
                        if row_has_errors {
                            row_json["has_errors"] = serde_json::json!(true);
                        }
                        row_json
                    }).collect()
                }
                _ => Vec::new(),
            };
            // Block definitions for the nested <template> sections
            let block_defs: Vec<_> = sf.blocks.iter().map(|bd| {
                let template_prefix = format!("{}[__INDEX__]", indexed_name);
                let block_fields: Vec<_> = bd.fields.iter().map(|nested_sf| {
                    build_single_field_context(nested_sf, &HashMap::new(), &HashMap::new(), &template_prefix, non_default_locale, depth + 1)
                }).collect();
                let mut def = serde_json::json!({
                    "block_type": bd.block_type,
                    "label": bd.label.as_ref().map(|ls| ls.resolve_default()).unwrap_or(&bd.block_type),
                    "fields": block_fields,
                });
                if let Some(ref lf) = bd.label_field {
                    def["label_field"] = serde_json::json!(lf);
                }
                if let Some(ref g) = bd.group {
                    def["group"] = serde_json::json!(g);
                }
                if let Some(ref url) = bd.image_url {
                    def["image_url"] = serde_json::json!(url);
                }
                def
            }).collect();
            sub_ctx["block_definitions"] = serde_json::json!(block_defs);
            sub_ctx["rows"] = serde_json::json!(nested_rows);
            sub_ctx["row_count"] = serde_json::json!(nested_rows.len());
            sub_ctx["template_id"] = serde_json::json!(safe_template_id(&indexed_name));
            if let Some(ref lf) = sf.admin.label_field {
                sub_ctx["label_field"] = serde_json::json!(lf);
            }
            if let Some(max) = sf.max_rows {
                sub_ctx["max_rows"] = serde_json::json!(max);
            }
            if let Some(min) = sf.min_rows {
                sub_ctx["min_rows"] = serde_json::json!(min);
            }
            sub_ctx["init_collapsed"] = serde_json::json!(sf.admin.collapsed);
            if let Some(ref ls) = sf.admin.labels_singular {
                sub_ctx["add_label"] = serde_json::json!(ls.resolve_default());
            }
        }
        FieldType::Group => {
            // Nested group: sub-fields are stored as keys in the same row object
            let group_obj = match raw_value {
                Some(serde_json::Value::Object(_)) => raw_value,
                _ => None,
            };
            let nested_sub_fields: Vec<_> = sf.fields.iter().map(|nested_sf| {
                let nested_raw = group_obj
                    .and_then(|v| v.as_object())
                    .and_then(|m| m.get(&nested_sf.name));
                let nested_name = format!("{}[{}]", indexed_name, nested_sf.name);
                let nested_val = nested_raw
                    .map(|v| match v {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Null => String::new(),
                        other => other.to_string(),
                    })
                    .unwrap_or_default();
                let nested_label = nested_sf.admin.label.as_ref()
                    .map(|ls| ls.resolve_default().to_string())
                    .unwrap_or_else(|| auto_label_from_name(&nested_sf.name));
                let mut nested_ctx = serde_json::json!({
                    "name": nested_name,
                    "field_type": nested_sf.field_type.as_str(),
                    "label": nested_label,
                    "value": nested_val,
                    "required": nested_sf.required,
                    "readonly": nested_sf.admin.readonly || locale_locked,
                    "locale_locked": locale_locked,
                    "placeholder": nested_sf.admin.placeholder.as_ref().map(|ls| ls.resolve_default()),
                    "description": nested_sf.admin.description.as_ref().map(|ls| ls.resolve_default()),
                });
                apply_field_type_extras(
                    nested_sf, &nested_val, &mut nested_ctx,
                    &HashMap::new(), &HashMap::new(), &nested_name,
                    non_default_locale, depth + 1,
                );
                nested_ctx
            }).collect();
            sub_ctx["sub_fields"] = serde_json::json!(nested_sub_fields);
            sub_ctx["collapsed"] = serde_json::json!(sf.admin.collapsed);
        }
        FieldType::Row | FieldType::Collapsible => {
            // Nested row/collapsible: use recursive helper that handles
            // arbitrary nesting of layout wrappers with structured data
            let nested_sub_fields = build_enriched_children_from_data(
                &sf.fields, raw_value, &indexed_name,
                locale_locked, non_default_locale, depth + 1, errors,
            );
            sub_ctx["sub_fields"] = serde_json::json!(nested_sub_fields);
            if sf.field_type == FieldType::Collapsible {
                sub_ctx["collapsed"] = serde_json::json!(sf.admin.collapsed);
            }
        }
        FieldType::Tabs => {
            // Nested tabs: use recursive helper that handles
            // arbitrary nesting of layout wrappers with structured data
            let tabs_ctx: Vec<_> = sf.tabs.iter().map(|tab| {
                let tab_sub_fields = build_enriched_children_from_data(
                    &tab.fields, raw_value, &indexed_name,
                    locale_locked, non_default_locale, depth + 1, errors,
                );
                let error_count = count_errors_in_fields(&tab_sub_fields);
                let mut tab_ctx = serde_json::json!({
                    "label": &tab.label,
                    "sub_fields": tab_sub_fields,
                });
                if error_count > 0 {
                    tab_ctx["error_count"] = serde_json::json!(error_count);
                }
                if let Some(ref desc) = tab.description {
                    tab_ctx["description"] = serde_json::json!(desc);
                }
                tab_ctx
            }).collect();
            sub_ctx["tabs"] = serde_json::json!(tabs_ctx);
        }
        FieldType::Text | FieldType::Number if sf.has_many => {
            let tags: Vec<String> = serde_json::from_str(&val).unwrap_or_default();
            sub_ctx["has_many"] = serde_json::json!(true);
            sub_ctx["tags"] = serde_json::json!(tags);
            sub_ctx["value"] = serde_json::json!(tags.join(","));
        }
        _ => {}
    }

    sub_ctx
}

/// Build selected_items for a polymorphic relationship field.
///
/// Polymorphic values are stored as "collection/id" composites. Each item is
/// looked up in its respective collection to get its label.
pub fn enrich_polymorphic_selected(
    rc: &crate::core::field::RelationshipConfig,
    field_name: &str,
    doc_fields: &HashMap<String, serde_json::Value>,
    reg: &crate::core::Registry,
    conn: &rusqlite::Connection,
    locale_ctx: Option<&crate::db::query::LocaleContext>,
) -> Vec<serde_json::Value> {
    // Parse "collection/id" refs
    let refs: Vec<(String, String)> = if rc.has_many {
        match doc_fields.get(field_name) {
            Some(serde_json::Value::Array(arr)) => {
                arr.iter().filter_map(|v| {
                    v.as_str().and_then(|s| {
                        let pos = s.find('/')?;
                        let col = &s[..pos];
                        let id = &s[pos + 1..];
                        if col.is_empty() || id.is_empty() { return None; }
                        Some((col.to_string(), id.to_string()))
                    })
                }).collect()
            }
            _ => Vec::new(),
        }
    } else {
        match doc_fields.get(field_name) {
            Some(serde_json::Value::String(s)) if !s.is_empty() => {
                if let Some(pos) = s.find('/') {
                    let col = &s[..pos];
                    let id = &s[pos + 1..];
                    if !col.is_empty() && !id.is_empty() {
                        vec![(col.to_string(), id.to_string())]
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    };

    refs.iter().filter_map(|(col, id)| {
        let related_def = reg.get_collection(col)?;
        let title_field = related_def.title_field().map(|s| s.to_string());
        crate::db::query::find_by_id(conn, col, related_def, id, locale_ctx)
            .ok()
            .flatten()
            .map(|doc| {
                let label = title_field.as_ref()
                    .and_then(|f| doc.get_str(f))
                    .unwrap_or(&doc.id)
                    .to_string();
                // Include collection in the id so JS knows which collection this item belongs to
                serde_json::json!({ "id": format!("{}/{}", col, doc.id), "label": label, "collection": col })
            })
    }).collect()
}

/// Enrich field contexts with data that requires DB access:
/// - Relationship fields: fetch available options from related collection
/// - Array fields: populate existing rows from hydrated document data
/// - Upload fields: fetch upload collection options with thumbnails
/// - Blocks fields: populate block rows from hydrated document data
pub fn enrich_field_contexts(
    fields: &mut [serde_json::Value],
    field_defs: &[crate::core::field::FieldDefinition],
    doc_fields: &HashMap<String, serde_json::Value>,
    state: &crate::admin::AdminState,
    filter_hidden: bool,
    non_default_locale: bool,
    errors: &HashMap<String, String>,
    doc_id: Option<&str>,
) {
    use crate::core::upload;
    use crate::db::query::{self, LocaleContext};

    let reg = &state.registry;
    let conn = match state.pool.get() {
        Ok(c) => c,
        Err(_) => return,
    };

    let rel_locale_ctx = LocaleContext::from_locale_string(None, &state.config.locale);

    let defs_iter: Box<dyn Iterator<Item = &crate::core::field::FieldDefinition>> = if filter_hidden {
        Box::new(field_defs.iter().filter(|f| !f.admin.hidden))
    } else {
        Box::new(field_defs.iter())
    };

    for (ctx, field_def) in fields.iter_mut().zip(defs_iter) {
        match field_def.field_type {
            FieldType::Relationship => {
                if let Some(ref rc) = field_def.relationship {
                    if rc.is_polymorphic() {
                        // Polymorphic: values are "collection/id" composites
                        let selected_items = enrich_polymorphic_selected(
                            rc, &field_def.name, doc_fields, reg, &conn, rel_locale_ctx.as_ref(),
                        );
                        ctx["selected_items"] = serde_json::json!(selected_items);
                    } else if let Some(related_def) = reg.get_collection(&rc.collection) {
                        let title_field = related_def.title_field().map(|s| s.to_string());
                        if rc.has_many {
                            let selected_ids: Vec<String> = match doc_fields.get(&field_def.name) {
                                Some(serde_json::Value::Array(arr)) => {
                                    arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
                                }
                                _ => Vec::new(),
                            };
                            let selected_items: Vec<_> = selected_ids.iter().filter_map(|id| {
                                query::find_by_id(&conn, &rc.collection, related_def, id, rel_locale_ctx.as_ref())
                                    .ok()
                                    .flatten()
                                    .map(|doc| {
                                        let label = title_field.as_ref()
                                            .and_then(|f| doc.get_str(f))
                                            .unwrap_or(&doc.id)
                                            .to_string();
                                        serde_json::json!({ "id": doc.id, "label": label })
                                    })
                            }).collect();
                            ctx["selected_items"] = serde_json::json!(selected_items);
                        } else {
                            let current_value = ctx.get("value")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            if !current_value.is_empty() {
                                if let Ok(Some(doc)) = query::find_by_id(&conn, &rc.collection, related_def, &current_value, rel_locale_ctx.as_ref()) {
                                    let label = title_field.as_ref()
                                        .and_then(|f| doc.get_str(f))
                                        .unwrap_or(&doc.id)
                                        .to_string();
                                    ctx["selected_items"] = serde_json::json!([{ "id": doc.id, "label": label }]);
                                } else {
                                    ctx["selected_items"] = serde_json::json!([]);
                                }
                            } else {
                                ctx["selected_items"] = serde_json::json!([]);
                            }
                        }
                    }
                }
            }
            FieldType::Array => {
                // Populate rows from hydrated document data
                let locale_locked = non_default_locale && !field_def.localized;
                let rows: Vec<serde_json::Value> = match doc_fields.get(&field_def.name) {
                    Some(serde_json::Value::Array(arr)) => {
                        arr.iter().enumerate().map(|(idx, row)| {
                            let row_obj = row.as_object();
                            let sub_values: Vec<_> = field_def.fields.iter().map(|sf| {
                                let raw_value = if matches!(sf.field_type,
                                    FieldType::Tabs | FieldType::Row | FieldType::Collapsible)
                                {
                                    Some(row) // pass whole row — data is stored flat
                                } else {
                                    row_obj.and_then(|m| m.get(&sf.name))
                                };
                                build_enriched_sub_field_context(
                                    sf, raw_value, &field_def.name, idx,
                                    locale_locked, non_default_locale, 1, errors,
                                )
                            }).collect();
                            let row_has_errors = sub_values.iter()
                                .any(|sf_ctx| sf_ctx.get("error").is_some());
                            let mut row_json = serde_json::json!({
                                "index": idx,
                                "sub_fields": sub_values,
                            });
                            if row_has_errors {
                                row_json["has_errors"] = serde_json::json!(true);
                            }
                            // Compute custom row label
                            use super::super::shared::compute_row_label;
                            if let Some(label) = compute_row_label(
                                &field_def.admin, None, row_obj, &state.hook_runner,
                            ) {
                                row_json["custom_label"] = serde_json::json!(label);
                            }
                            row_json
                        }).collect()
                    }
                    _ => Vec::new(),
                };
                ctx["row_count"] = serde_json::json!(rows.len());
                ctx["rows"] = serde_json::json!(rows);
                // Enrich Upload/Relationship sub-fields within each row
                if let Some(rows_arr) = ctx.get_mut("rows").and_then(|v| v.as_array_mut()) {
                    for row_ctx in rows_arr.iter_mut() {
                        if let Some(sub_arr) = row_ctx.get_mut("sub_fields").and_then(|v| v.as_array_mut()) {
                            enrich_nested_fields(sub_arr, &field_def.fields, &conn, reg, rel_locale_ctx.as_ref());
                        }
                    }
                }
                // Enrich the <template> sub-fields so new rows added via JS have upload/relationship options
                if let Some(sub_arr) = ctx.get_mut("sub_fields").and_then(|v| v.as_array_mut()) {
                    enrich_nested_fields(sub_arr, &field_def.fields, &conn, reg, rel_locale_ctx.as_ref());
                }
            }
            FieldType::Upload => {
                if let Some(ref rc) = field_def.relationship {
                    if let Some(related_def) = reg.get_collection(&rc.collection) {
                        let title_field = related_def.title_field().map(|s| s.to_string());
                        let admin_thumbnail = related_def.upload.as_ref()
                            .and_then(|u| u.admin_thumbnail.as_ref().cloned());

                        if rc.has_many {
                            // Has-many upload: build selected_items from hydrated IDs
                            let selected_ids: Vec<String> = match doc_fields.get(&field_def.name) {
                                Some(serde_json::Value::Array(arr)) => {
                                    arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
                                }
                                _ => Vec::new(),
                            };
                            let selected_items: Vec<_> = selected_ids.iter().filter_map(|id| {
                                query::find_by_id(&conn, &rc.collection, related_def, id, rel_locale_ctx.as_ref())
                                    .ok()
                                    .flatten()
                                    .map(|mut doc| {
                                        if let Some(ref uc) = related_def.upload {
                                            if uc.enabled { upload::assemble_sizes_object(&mut doc, uc); }
                                        }
                                        let label = doc.get_str("filename")
                                            .or_else(|| title_field.as_ref().and_then(|f| doc.get_str(f)))
                                            .unwrap_or(&doc.id)
                                            .to_string();
                                        let mime = doc.get_str("mime_type").unwrap_or("").to_string();
                                        let is_image = mime.starts_with("image/");
                                        let thumb_url = if is_image {
                                            admin_thumbnail.as_ref()
                                                .and_then(|thumb_name| {
                                                    doc.fields.get("sizes")
                                                        .and_then(|v| v.get(thumb_name))
                                                        .and_then(|v| v.get("url"))
                                                        .and_then(|v| v.as_str())
                                                        .map(|s| s.to_string())
                                                })
                                                .or_else(|| doc.get_str("url").map(|s| s.to_string()))
                                        } else { None };
                                        let mut item = serde_json::json!({ "id": doc.id, "label": label });
                                        if let Some(url) = thumb_url { item["thumbnail_url"] = serde_json::json!(url); }
                                        if is_image { item["is_image"] = serde_json::json!(true); }
                                        item
                                    })
                            }).collect();
                            ctx["selected_items"] = serde_json::json!(selected_items);
                        } else {
                            // Has-one upload: fetch only the selected doc (not all docs)
                            let current_value = ctx.get("value")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            if !current_value.is_empty() {
                                if let Ok(Some(mut doc)) = query::find_by_id(&conn, &rc.collection, related_def, &current_value, rel_locale_ctx.as_ref()) {
                                    if let Some(ref uc) = related_def.upload {
                                        if uc.enabled { upload::assemble_sizes_object(&mut doc, uc); }
                                    }
                                    let label = doc.get_str("filename")
                                        .or_else(|| title_field.as_ref().and_then(|f| doc.get_str(f)))
                                        .unwrap_or(&doc.id)
                                        .to_string();
                                    let mime = doc.get_str("mime_type").unwrap_or("").to_string();
                                    let is_image = mime.starts_with("image/");
                                    let thumb_url = if is_image {
                                        admin_thumbnail.as_ref()
                                            .and_then(|thumb_name| {
                                                doc.fields.get("sizes")
                                                    .and_then(|v| v.get(thumb_name))
                                                    .and_then(|v| v.get("url"))
                                                    .and_then(|v| v.as_str())
                                                    .map(|s| s.to_string())
                                            })
                                            .or_else(|| doc.get_str("url").map(|s| s.to_string()))
                                    } else { None };
                                    let mut item = serde_json::json!({ "id": doc.id, "label": label });
                                    if let Some(ref url) = thumb_url { item["thumbnail_url"] = serde_json::json!(url); }
                                    if is_image { item["is_image"] = serde_json::json!(true); }
                                    item["filename"] = serde_json::json!(label);
                                    ctx["selected_items"] = serde_json::json!([item]);
                                    if let Some(url) = thumb_url {
                                        ctx["selected_preview_url"] = serde_json::json!(url);
                                    }
                                    ctx["selected_filename"] = serde_json::json!(label);
                                } else {
                                    ctx["selected_items"] = serde_json::json!([]);
                                }
                            } else {
                                ctx["selected_items"] = serde_json::json!([]);
                            }
                        }
                    }
                }
            }
            FieldType::Blocks => {
                // Populate rows from hydrated document data
                let locale_locked = non_default_locale && !field_def.localized;
                let rows: Vec<serde_json::Value> = match doc_fields.get(&field_def.name) {
                    Some(serde_json::Value::Array(arr)) => {
                        arr.iter().enumerate().map(|(idx, row)| {
                            let row_obj = row.as_object();
                            let block_type = row_obj
                                .and_then(|m| m.get("_block_type"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let block_label = field_def.blocks.iter()
                                .find(|bd| bd.block_type == block_type)
                                .and_then(|bd| bd.label.as_ref().map(|ls| ls.resolve_default()))
                                .unwrap_or(block_type);
                            let block_def = field_def.blocks.iter()
                                .find(|bd| bd.block_type == block_type);
                            let block_label_field = block_def.and_then(|bd| bd.label_field.as_deref());
                            let sub_values: Vec<_> = block_def
                                .map(|bd| bd.fields.iter().map(|sf| {
                                    let raw_value = if matches!(sf.field_type,
                                        FieldType::Tabs | FieldType::Row | FieldType::Collapsible)
                                    {
                                        Some(row) // pass whole block data object
                                    } else {
                                        row_obj.and_then(|m| m.get(&sf.name))
                                    };
                                    build_enriched_sub_field_context(
                                        sf, raw_value, &field_def.name, idx,
                                        locale_locked, non_default_locale, 1, errors,
                                    )
                                }).collect())
                                .unwrap_or_default();
                            let row_has_errors = sub_values.iter()
                                .any(|sf_ctx| sf_ctx.get("error").is_some());
                            let mut row_json = serde_json::json!({
                                "index": idx,
                                "_block_type": block_type,
                                "block_label": block_label,
                                "sub_fields": sub_values,
                            });
                            if row_has_errors {
                                row_json["has_errors"] = serde_json::json!(true);
                            }
                            // Compute custom row label
                            use super::super::shared::compute_row_label;
                            if let Some(label) = compute_row_label(
                                &field_def.admin, block_label_field, row_obj, &state.hook_runner,
                            ) {
                                row_json["custom_label"] = serde_json::json!(label);
                            }
                            row_json
                        }).collect()
                    }
                    _ => Vec::new(),
                };
                ctx["row_count"] = serde_json::json!(rows.len());
                ctx["rows"] = serde_json::json!(rows);
                // Enrich Upload/Relationship sub-fields within each block row
                if let Some(rows_arr) = ctx.get_mut("rows").and_then(|v| v.as_array_mut()) {
                    for row_ctx in rows_arr.iter_mut() {
                        let block_type = row_ctx.get("_block_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if let Some(block_def) = field_def.blocks.iter().find(|bd| bd.block_type == block_type) {
                            if let Some(sub_arr) = row_ctx.get_mut("sub_fields").and_then(|v| v.as_array_mut()) {
                                enrich_nested_fields(sub_arr, &block_def.fields, &conn, reg, rel_locale_ctx.as_ref());
                            }
                        }
                    }
                }
                // Enrich Upload/Relationship sub-fields within block definition templates
                // (these are the <template> elements cloned by JS when adding new block rows)
                if let Some(defs_arr) = ctx.get_mut("block_definitions").and_then(|v| v.as_array_mut()) {
                    for (def_ctx, block_def) in defs_arr.iter_mut().zip(field_def.blocks.iter()) {
                        if let Some(sub_arr) = def_ctx.get_mut("fields").and_then(|v| v.as_array_mut()) {
                            enrich_nested_fields(sub_arr, &block_def.fields, &conn, reg, rel_locale_ctx.as_ref());
                        }
                    }
                }
            }
            FieldType::Row | FieldType::Collapsible => {
                // Recurse with full enrichment so Blocks/Arrays inside get rows populated from doc_fields
                if let Some(sub_arr) = ctx.get_mut("sub_fields").and_then(|v| v.as_array_mut()) {
                    enrich_field_contexts(sub_arr, &field_def.fields, doc_fields, state, filter_hidden, non_default_locale, errors, doc_id);
                }
            }
            FieldType::Group => {
                // Groups use prefixed columns — nested enrichment is sufficient
                if let Some(sub_arr) = ctx.get_mut("sub_fields").and_then(|v| v.as_array_mut()) {
                    enrich_nested_fields(sub_arr, &field_def.fields, &conn, reg, rel_locale_ctx.as_ref());
                }
            }
            FieldType::Tabs => {
                // Recurse into each tab's sub-fields with full enrichment (not just nested),
                // so Blocks/Arrays/Relationships inside tabs get their rows populated from doc_fields.
                if let Some(tabs_arr) = ctx.get_mut("tabs").and_then(|v| v.as_array_mut()) {
                    for (tab_ctx, tab_def) in tabs_arr.iter_mut().zip(field_def.tabs.iter()) {
                        if let Some(sub_arr) = tab_ctx.get_mut("sub_fields").and_then(|v| v.as_array_mut()) {
                            enrich_field_contexts(sub_arr, &tab_def.fields, doc_fields, state, filter_hidden, non_default_locale, errors, doc_id);
                        }
                    }
                }
            }
            FieldType::Join => {
                // Virtual reverse-relationship: query target collection for docs that reference this one
                if let Some(ref jc) = field_def.join {
                    if let Some(doc_id_str) = doc_id {
                        if let Some(target_def) = reg.get_collection(&jc.collection) {
                            let title_field = target_def.title_field().map(|s| s.to_string());
                            let fq = query::FindQuery {
                                filters: vec![query::FilterClause::Single(query::Filter {
                                    field: jc.on.clone(),
                                    op: query::FilterOp::Equals(doc_id_str.to_string()),
                                })],
                                ..Default::default()
                            };
                            if let Ok(docs) = query::find(&conn, &jc.collection, target_def, &fq, rel_locale_ctx.as_ref()) {
                                let items: Vec<_> = docs.iter().map(|doc| {
                                    let label = title_field.as_ref()
                                        .and_then(|f| doc.get_str(f))
                                        .unwrap_or(&doc.id)
                                        .to_string();
                                    serde_json::json!({ "id": doc.id, "label": label })
                                }).collect();
                                ctx["join_items"] = serde_json::json!(items);
                                ctx["join_count"] = serde_json::json!(items.len());
                            }
                        }
                    }
                }
            }
            FieldType::Richtext => {
                // Resolve custom node names to full definitions from registry
                if let Some(node_names) = ctx.get("_node_names").cloned() {
                    if let Some(names) = node_names.as_array() {
                        let node_defs: Vec<_> = names.iter()
                            .filter_map(|n| n.as_str())
                            .filter_map(|name| reg.get_richtext_node(name))
                            .map(|def| serde_json::json!({
                                "name": def.name,
                                "label": def.label,
                                "inline": def.inline,
                                "attrs": def.attrs.iter().map(|a| {
                                    let mut attr = serde_json::json!({
                                        "name": a.name,
                                        "type": a.attr_type.as_str(),
                                        "label": a.label,
                                        "required": a.required,
                                    });
                                    if let Some(ref dv) = a.default_value {
                                        attr["default"] = dv.clone();
                                    }
                                    if !a.options.is_empty() {
                                        attr["options"] = serde_json::json!(
                                            a.options.iter().map(|o| serde_json::json!({
                                                "label": o.label.resolve_default(),
                                                "value": o.value,
                                            })).collect::<Vec<_>>()
                                        );
                                    }
                                    attr
                                }).collect::<Vec<_>>(),
                            }))
                            .collect();
                        if !node_defs.is_empty() {
                            ctx["custom_nodes"] = serde_json::json!(node_defs);
                        }
                    }
                    // Clean up internal marker
                    if let Some(obj) = ctx.as_object_mut() {
                        obj.remove("_node_names");
                    }
                }
            }
            _ => {}
        }
    }
}

/// Recursively enrich Upload and Relationship sub-field contexts with options from the database.
/// Called for sub-fields inside layout containers (Row, Collapsible, Tabs, Group) and
/// composite fields (Array, Blocks) that can't be enriched during initial context building.
pub fn enrich_nested_fields(
    sub_fields: &mut [serde_json::Value],
    field_defs: &[crate::core::field::FieldDefinition],
    conn: &rusqlite::Connection,
    reg: &crate::core::Registry,
    rel_locale_ctx: Option<&crate::db::query::LocaleContext>,
) {
    use crate::core::upload;
    use crate::db::query;

    for (ctx, field_def) in sub_fields.iter_mut().zip(field_defs.iter()) {
        match field_def.field_type {
            FieldType::Relationship => {
                if let Some(ref rc) = field_def.relationship {
                    if let Some(related_def) = reg.get_collection(&rc.collection) {
                        let title_field = related_def.title_field().map(|s| s.to_string());
                        if rc.has_many {
                            // Has-many nested relationships use selected_items built by parent
                        } else {
                            let current_value = ctx.get("value")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            if !current_value.is_empty() {
                                if let Ok(Some(doc)) = query::find_by_id(conn, &rc.collection, related_def, &current_value, rel_locale_ctx) {
                                    let label = title_field.as_ref()
                                        .and_then(|f| doc.get_str(f))
                                        .unwrap_or(&doc.id)
                                        .to_string();
                                    ctx["selected_items"] = serde_json::json!([{ "id": doc.id, "label": label }]);
                                } else {
                                    ctx["selected_items"] = serde_json::json!([]);
                                }
                            } else {
                                ctx["selected_items"] = serde_json::json!([]);
                            }
                        }
                    }
                }
            }
            FieldType::Upload => {
                if let Some(ref rc) = field_def.relationship {
                    if let Some(related_def) = reg.get_collection(&rc.collection) {
                        let title_field = related_def.title_field().map(|s| s.to_string());
                        let admin_thumbnail = related_def.upload.as_ref()
                            .and_then(|u| u.admin_thumbnail.as_ref().cloned());

                        if rc.has_many {
                            // Has-many: selected_items already handled by the parent context
                        } else {
                            // Has-one upload: fetch only the selected doc via search widget
                            let current_value = ctx.get("value")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            if !current_value.is_empty() {
                                if let Ok(Some(mut doc)) = query::find_by_id(conn, &rc.collection, related_def, &current_value, rel_locale_ctx) {
                                    if let Some(ref uc) = related_def.upload {
                                        if uc.enabled { upload::assemble_sizes_object(&mut doc, uc); }
                                    }
                                    let label = doc.get_str("filename")
                                        .or_else(|| title_field.as_ref().and_then(|f| doc.get_str(f)))
                                        .unwrap_or(&doc.id)
                                        .to_string();
                                    let mime = doc.get_str("mime_type").unwrap_or("").to_string();
                                    let is_image = mime.starts_with("image/");
                                    let thumb_url = if is_image {
                                        admin_thumbnail.as_ref()
                                            .and_then(|thumb_name| {
                                                doc.fields.get("sizes")
                                                    .and_then(|v| v.get(thumb_name))
                                                    .and_then(|v| v.get("url"))
                                                    .and_then(|v| v.as_str())
                                                    .map(|s| s.to_string())
                                            })
                                            .or_else(|| doc.get_str("url").map(|s| s.to_string()))
                                    } else { None };
                                    let mut item = serde_json::json!({ "id": doc.id, "label": label });
                                    if let Some(ref url) = thumb_url { item["thumbnail_url"] = serde_json::json!(url); }
                                    if is_image { item["is_image"] = serde_json::json!(true); }
                                    item["filename"] = serde_json::json!(label);
                                    ctx["selected_items"] = serde_json::json!([item]);
                                    if let Some(url) = thumb_url {
                                        ctx["selected_preview_url"] = serde_json::json!(url);
                                    }
                                    ctx["selected_filename"] = serde_json::json!(label);
                                } else {
                                    ctx["selected_items"] = serde_json::json!([]);
                                }
                            } else {
                                ctx["selected_items"] = serde_json::json!([]);
                            }
                        }
                    }
                }
            }
            FieldType::Row | FieldType::Collapsible | FieldType::Group => {
                if let Some(sub_arr) = ctx.get_mut("sub_fields").and_then(|v| v.as_array_mut()) {
                    enrich_nested_fields(sub_arr, &field_def.fields, conn, reg, rel_locale_ctx);
                }
            }
            FieldType::Tabs => {
                if let Some(tabs_arr) = ctx.get_mut("tabs").and_then(|v| v.as_array_mut()) {
                    for (tab_ctx, tab_def) in tabs_arr.iter_mut().zip(field_def.tabs.iter()) {
                        if let Some(sub_arr) = tab_ctx.get_mut("sub_fields").and_then(|v| v.as_array_mut()) {
                            enrich_nested_fields(sub_arr, &tab_def.fields, conn, reg, rel_locale_ctx);
                        }
                    }
                }
            }
            FieldType::Array => {
                // Recurse into array rows' sub-fields
                if let Some(rows_arr) = ctx.get_mut("rows").and_then(|v| v.as_array_mut()) {
                    for row_ctx in rows_arr.iter_mut() {
                        if let Some(sub_arr) = row_ctx.get_mut("sub_fields").and_then(|v| v.as_array_mut()) {
                            enrich_nested_fields(sub_arr, &field_def.fields, conn, reg, rel_locale_ctx);
                        }
                    }
                }
                // Enrich the <template> sub-fields so new rows added via JS have upload/relationship options
                if let Some(sub_arr) = ctx.get_mut("sub_fields").and_then(|v| v.as_array_mut()) {
                    enrich_nested_fields(sub_arr, &field_def.fields, conn, reg, rel_locale_ctx);
                }
            }
            FieldType::Blocks => {
                // Recurse into block rows' sub-fields, matching each row's block type
                if let Some(rows_arr) = ctx.get_mut("rows").and_then(|v| v.as_array_mut()) {
                    for row_ctx in rows_arr.iter_mut() {
                        let block_type = row_ctx.get("_block_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if let Some(block_def) = field_def.blocks.iter().find(|bd| bd.block_type == block_type) {
                            if let Some(sub_arr) = row_ctx.get_mut("sub_fields").and_then(|v| v.as_array_mut()) {
                                enrich_nested_fields(sub_arr, &block_def.fields, conn, reg, rel_locale_ctx);
                            }
                        }
                    }
                }
                // Enrich block definition templates so new block rows have upload/relationship options
                if let Some(defs_arr) = ctx.get_mut("block_definitions").and_then(|v| v.as_array_mut()) {
                    for (def_ctx, block_def) in defs_arr.iter_mut().zip(field_def.blocks.iter()) {
                        if let Some(sub_arr) = def_ctx.get_mut("fields").and_then(|v| v.as_array_mut()) {
                            enrich_nested_fields(sub_arr, &block_def.fields, conn, reg, rel_locale_ctx);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::builder::build_field_contexts;
    use crate::core::field::{FieldDefinition, SelectOption, LocalizedString, BlockDefinition};

    fn make_field(name: &str, ft: FieldType) -> FieldDefinition {
        FieldDefinition {
            name: name.to_string(),
            field_type: ft,
            ..Default::default()
        }
    }

    // --- Recursive enrichment tests (build_enriched_sub_field_context) ---

    #[test]
    fn enriched_sub_field_nested_array_populates_rows() {
        let mut inner_array = make_field("images", FieldType::Array);
        inner_array.fields = vec![
            make_field("url", FieldType::Text),
            make_field("alt", FieldType::Text),
        ];

        // Simulate hydrated data: an array with 2 rows
        let raw_value = serde_json::json!([
            {"url": "img1.jpg", "alt": "First"},
            {"url": "img2.jpg", "alt": "Second"},
        ]);

        let ctx = build_enriched_sub_field_context(
            &inner_array, Some(&raw_value), "content", 0,
            false, false, 1, &HashMap::new(),
        );

        assert_eq!(ctx["field_type"], "array");
        assert_eq!(ctx["row_count"], 2);

        let rows = ctx["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2);

        // First row sub_fields
        let row0_fields = rows[0]["sub_fields"].as_array().unwrap();
        assert_eq!(row0_fields[0]["name"], "content[0][images][0][url]");
        assert_eq!(row0_fields[0]["value"], "img1.jpg");
        assert_eq!(row0_fields[1]["name"], "content[0][images][0][alt]");
        assert_eq!(row0_fields[1]["value"], "First");

        // Second row sub_fields
        let row1_fields = rows[1]["sub_fields"].as_array().unwrap();
        assert_eq!(row1_fields[0]["value"], "img2.jpg");
        assert_eq!(row1_fields[1]["value"], "Second");

        // Template sub_fields should use __INDEX__
        let template_sub = ctx["sub_fields"].as_array().unwrap();
        assert_eq!(template_sub[0]["name"], "content[0][images][__INDEX__][url]");
    }

    #[test]
    fn enriched_sub_field_nested_blocks_populates_rows() {
        let mut inner_blocks = make_field("sections", FieldType::Blocks);
        inner_blocks.blocks = vec![BlockDefinition {
            block_type: "text".to_string(),
            label: Some(LocalizedString::Plain("Text".to_string())),
            fields: vec![make_field("body", FieldType::Richtext)],
            ..Default::default()
        }];

        let raw_value = serde_json::json!([
            {"_block_type": "text", "body": "<p>Hello</p>"},
        ]);

        let ctx = build_enriched_sub_field_context(
            &inner_blocks, Some(&raw_value), "page", 2,
            false, false, 1, &HashMap::new(),
        );

        assert_eq!(ctx["field_type"], "blocks");
        assert_eq!(ctx["row_count"], 1);

        let rows = ctx["rows"].as_array().unwrap();
        assert_eq!(rows[0]["_block_type"], "text");
        assert_eq!(rows[0]["block_label"], "Text");

        let sub_fields = rows[0]["sub_fields"].as_array().unwrap();
        assert_eq!(sub_fields[0]["name"], "page[2][sections][0][body]");
        assert_eq!(sub_fields[0]["value"], "<p>Hello</p>");

        // Block definitions for templates
        let block_defs = ctx["block_definitions"].as_array().unwrap();
        assert_eq!(block_defs.len(), 1);
    }

    #[test]
    fn enriched_sub_field_nested_group_populates_values() {
        let mut inner_group = make_field("meta", FieldType::Group);
        inner_group.fields = vec![
            make_field("author", FieldType::Text),
            make_field("published", FieldType::Checkbox),
        ];

        let raw_value = serde_json::json!({
            "author": "Alice",
            "published": "1",
        });

        let ctx = build_enriched_sub_field_context(
            &inner_group, Some(&raw_value), "items", 0,
            false, false, 1, &HashMap::new(),
        );

        assert_eq!(ctx["field_type"], "group");
        let sub_fields = ctx["sub_fields"].as_array().unwrap();
        assert_eq!(sub_fields.len(), 2);
        assert_eq!(sub_fields[0]["name"], "items[0][meta][author]");
        assert_eq!(sub_fields[0]["value"], "Alice");
        assert_eq!(sub_fields[1]["name"], "items[0][meta][published]");
        assert_eq!(sub_fields[1]["checked"], true);
    }

    #[test]
    fn enriched_sub_field_empty_nested_array() {
        let mut inner_array = make_field("tags", FieldType::Array);
        inner_array.fields = vec![make_field("name", FieldType::Text)];

        // No data
        let ctx = build_enriched_sub_field_context(
            &inner_array, None, "items", 0,
            false, false, 1, &HashMap::new(),
        );

        assert_eq!(ctx["field_type"], "array");
        assert_eq!(ctx["row_count"], 0);
        let rows = ctx["rows"].as_array().unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn enriched_sub_field_select_preserves_selected() {
        let mut select_field = make_field("status", FieldType::Select);
        select_field.options = vec![
            SelectOption { label: LocalizedString::Plain("Draft".to_string()), value: "draft".to_string() },
            SelectOption { label: LocalizedString::Plain("Published".to_string()), value: "published".to_string() },
        ];

        let raw_value = serde_json::json!("published");

        let ctx = build_enriched_sub_field_context(
            &select_field, Some(&raw_value), "items", 0,
            false, false, 1, &HashMap::new(),
        );

        let opts = ctx["options"].as_array().unwrap();
        assert_eq!(opts[0]["selected"], false);
        assert_eq!(opts[1]["selected"], true);
    }

    #[test]
    fn max_depth_prevents_infinite_recursion() {
        // Build a deeply nested array structure
        fn make_nested_array(depth: usize) -> FieldDefinition {
            let mut field = FieldDefinition {
                name: format!("level{}", depth),
                field_type: FieldType::Array,
                ..Default::default()
            };
            if depth < 10 {
                field.fields = vec![make_nested_array(depth + 1)];
            } else {
                field.fields = vec![FieldDefinition {
                    name: "leaf".to_string(),
                    field_type: FieldType::Text,
                    ..Default::default()
                }];
            }
            field
        }
        let deep = make_nested_array(0);
        let fields = vec![deep];
        // This should not stack overflow -- MAX_FIELD_DEPTH caps recursion
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["field_type"], "array");
    }

    // --- enriched_sub_field: error propagation ---

    #[test]
    fn enriched_sub_field_with_error() {
        let sf = make_field("title", FieldType::Text);
        let mut errors = HashMap::new();
        errors.insert("content[0][title]".to_string(), "Required".to_string());
        let ctx = build_enriched_sub_field_context(
            &sf, Some(&serde_json::json!("val")), "content", 0,
            false, false, 1, &errors,
        );
        assert_eq!(ctx["error"], "Required");
    }

    // --- enriched_sub_field: max depth ---

    #[test]
    fn enriched_sub_field_max_depth_returns_early() {
        let mut arr = make_field("deep", FieldType::Array);
        arr.fields = vec![make_field("leaf", FieldType::Text)];
        let ctx = build_enriched_sub_field_context(
            &arr, Some(&serde_json::json!([])), "parent", 0,
            false, false, MAX_FIELD_DEPTH, &HashMap::new(),
        );
        // At max depth, array-specific fields should not be added
        assert!(ctx.get("rows").is_none());
        assert!(ctx.get("sub_fields").is_none());
    }

    // --- enriched_sub_field: date field ---

    #[test]
    fn enriched_sub_field_date_day_only() {
        let sf = make_field("d", FieldType::Date);
        let raw = serde_json::json!("2026-03-15T10:00:00Z");
        let ctx = build_enriched_sub_field_context(
            &sf, Some(&raw), "items", 0, false, false, 1, &HashMap::new(),
        );
        assert_eq!(ctx["picker_appearance"], "dayOnly");
        assert_eq!(ctx["date_only_value"], "2026-03-15");
    }

    #[test]
    fn enriched_sub_field_date_day_and_time() {
        let mut sf = make_field("d", FieldType::Date);
        sf.picker_appearance = Some("dayAndTime".to_string());
        let raw = serde_json::json!("2026-03-15T10:30:00Z");
        let ctx = build_enriched_sub_field_context(
            &sf, Some(&raw), "items", 0, false, false, 1, &HashMap::new(),
        );
        assert_eq!(ctx["picker_appearance"], "dayAndTime");
        assert_eq!(ctx["datetime_local_value"], "2026-03-15T10:30");
    }

    #[test]
    fn enriched_sub_field_date_short_value() {
        let sf = make_field("d", FieldType::Date);
        let raw = serde_json::json!("short");
        let ctx = build_enriched_sub_field_context(
            &sf, Some(&raw), "items", 0, false, false, 1, &HashMap::new(),
        );
        assert_eq!(ctx["date_only_value"], "short");
    }

    // --- enriched_sub_field: upload field ---

    #[test]
    fn enriched_sub_field_upload() {
        use crate::core::field::RelationshipConfig;
        let mut sf = make_field("image", FieldType::Upload);
        sf.relationship = Some(RelationshipConfig {
            collection: "media".to_string(),
            has_many: false,
            max_depth: None,
            polymorphic: vec![],
        });
        let ctx = build_enriched_sub_field_context(
            &sf, Some(&serde_json::json!("img123")), "items", 0,
            false, false, 1, &HashMap::new(),
        );
        assert_eq!(ctx["relationship_collection"], "media");
        assert_eq!(ctx["picker"], "drawer");
    }

    // --- enriched_sub_field: relationship field ---

    #[test]
    fn enriched_sub_field_relationship() {
        use crate::core::field::RelationshipConfig;
        let mut sf = make_field("author", FieldType::Relationship);
        sf.relationship = Some(RelationshipConfig {
            collection: "users".to_string(),
            has_many: true,
            max_depth: None,
            polymorphic: vec![],
        });
        let ctx = build_enriched_sub_field_context(
            &sf, Some(&serde_json::json!("user1")), "items", 0,
            false, false, 1, &HashMap::new(),
        );
        assert_eq!(ctx["relationship_collection"], "users");
        assert_eq!(ctx["has_many"], true);
    }

    // --- enriched_sub_field: value stringification ---

    #[test]
    fn enriched_sub_field_null_value_empty_string() {
        let sf = make_field("title", FieldType::Text);
        let ctx = build_enriched_sub_field_context(
            &sf, Some(&serde_json::Value::Null), "items", 0,
            false, false, 1, &HashMap::new(),
        );
        assert_eq!(ctx["value"], "");
    }

    #[test]
    fn enriched_sub_field_number_to_string() {
        let sf = make_field("count", FieldType::Number);
        let ctx = build_enriched_sub_field_context(
            &sf, Some(&serde_json::json!(42)), "items", 0,
            false, false, 1, &HashMap::new(),
        );
        assert_eq!(ctx["value"], "42");
    }

    #[test]
    fn enriched_sub_field_no_value() {
        let sf = make_field("title", FieldType::Text);
        let ctx = build_enriched_sub_field_context(
            &sf, None, "items", 0, false, false, 1, &HashMap::new(),
        );
        assert_eq!(ctx["value"], "");
    }

    // --- enriched_sub_field: array with min/max rows, collapsed, labels ---

    #[test]
    fn enriched_sub_field_array_with_options() {
        let mut arr = make_field("tags", FieldType::Array);
        arr.fields = vec![make_field("name", FieldType::Text)];
        arr.min_rows = Some(1);
        arr.max_rows = Some(5);
        arr.admin.collapsed = true;
        arr.admin.labels_singular = Some(LocalizedString::Plain("Tag".to_string()));
        let ctx = build_enriched_sub_field_context(
            &arr, Some(&serde_json::json!([])), "items", 0,
            false, false, 1, &HashMap::new(),
        );
        assert_eq!(ctx["min_rows"], 1);
        assert_eq!(ctx["max_rows"], 5);
        assert_eq!(ctx["init_collapsed"], true);
        assert_eq!(ctx["add_label"], "Tag");
    }

    // --- enriched_sub_field: blocks with min/max rows, collapsed, labels ---

    #[test]
    fn enriched_sub_field_blocks_with_options() {
        let mut blk = make_field("sections", FieldType::Blocks);
        blk.blocks = vec![BlockDefinition {
            block_type: "text".to_string(),
            label: None,
            fields: vec![make_field("body", FieldType::Text)],
            ..Default::default()
        }];
        blk.min_rows = Some(0);
        blk.max_rows = Some(10);
        blk.admin.collapsed = true;
        blk.admin.labels_singular = Some(LocalizedString::Plain("Section".to_string()));
        blk.admin.label_field = Some("body".to_string());
        let ctx = build_enriched_sub_field_context(
            &blk, Some(&serde_json::json!([])), "items", 0,
            false, false, 1, &HashMap::new(),
        );
        assert_eq!(ctx["min_rows"], 0);
        assert_eq!(ctx["max_rows"], 10);
        assert_eq!(ctx["init_collapsed"], true);
        assert_eq!(ctx["add_label"], "Section");
        assert_eq!(ctx["label_field"], "body");
    }

    // --- enriched_sub_field: nested blocks with row errors ---

    #[test]
    fn enriched_sub_field_nested_array_row_errors() {
        let mut inner_array = make_field("items", FieldType::Array);
        inner_array.fields = vec![make_field("title", FieldType::Text)];

        let raw_value = serde_json::json!([{"title": ""}]);
        let mut errors = HashMap::new();
        errors.insert("parent[0][items][0][title]".to_string(), "Required".to_string());

        let ctx = build_enriched_sub_field_context(
            &inner_array, Some(&raw_value), "parent", 0,
            false, false, 1, &errors,
        );

        let rows = ctx["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        let row_fields = rows[0]["sub_fields"].as_array().unwrap();
        assert_eq!(row_fields[0]["error"], "Required");
        assert_eq!(rows[0]["has_errors"], true);
    }

    #[test]
    fn enriched_sub_field_nested_blocks_row_errors() {
        let mut blk = make_field("sections", FieldType::Blocks);
        blk.blocks = vec![BlockDefinition {
            block_type: "text".to_string(),
            label: Some(LocalizedString::Plain("Text".to_string())),
            fields: vec![make_field("body", FieldType::Richtext)],
            ..Default::default()
        }];

        let raw_value = serde_json::json!([{"_block_type": "text", "body": ""}]);
        let mut errors = HashMap::new();
        errors.insert("parent[0][sections][0][body]".to_string(), "Required".to_string());

        let ctx = build_enriched_sub_field_context(
            &blk, Some(&raw_value), "parent", 0,
            false, false, 1, &errors,
        );

        let rows = ctx["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["has_errors"], true);
    }

    // --- enriched_sub_field: group with collapsed ---

    #[test]
    fn enriched_sub_field_group_collapsed() {
        let mut grp = make_field("meta", FieldType::Group);
        grp.fields = vec![make_field("author", FieldType::Text)];
        grp.admin.collapsed = true;
        let raw = serde_json::json!({"author": "Alice"});
        let ctx = build_enriched_sub_field_context(
            &grp, Some(&raw), "items", 0,
            false, false, 1, &HashMap::new(),
        );
        assert_eq!(ctx["collapsed"], true);
    }

    // --- enriched_sub_field: group with non-object value ---

    #[test]
    fn enriched_sub_field_group_with_null_value() {
        let mut grp = make_field("meta", FieldType::Group);
        grp.fields = vec![make_field("author", FieldType::Text)];
        let ctx = build_enriched_sub_field_context(
            &grp, Some(&serde_json::Value::Null), "items", 0,
            false, false, 1, &HashMap::new(),
        );
        // group_obj should be None so nested values are empty
        let sub_fields = ctx["sub_fields"].as_array().unwrap();
        assert_eq!(sub_fields[0]["value"], "");
    }

    // --- enriched_sub_field: nested blocks with unknown block type ---

    #[test]
    fn enriched_sub_field_nested_blocks_unknown_type() {
        let mut blk = make_field("sections", FieldType::Blocks);
        blk.blocks = vec![BlockDefinition {
            block_type: "text".to_string(),
            label: Some(LocalizedString::Plain("Text".to_string())),
            fields: vec![make_field("body", FieldType::Richtext)],
            ..Default::default()
        }];

        // Row with unknown block type
        let raw_value = serde_json::json!([{"_block_type": "unknown_type", "body": "content"}]);

        let ctx = build_enriched_sub_field_context(
            &blk, Some(&raw_value), "parent", 0,
            false, false, 1, &HashMap::new(),
        );

        let rows = ctx["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["_block_type"], "unknown_type");
        assert_eq!(rows[0]["block_label"], "unknown_type"); // falls back to block_type string
        // sub_fields should be empty since block_def is not found
        let sub_fields = rows[0]["sub_fields"].as_array().unwrap();
        assert!(sub_fields.is_empty());
    }

    // --- enrich_nested_fields tests ---

    #[test]
    fn enrich_nested_fields_upload_gets_options() {
        use crate::core::collection::*;
        use crate::core::field::RelationshipConfig;

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE media (
                id TEXT PRIMARY KEY,
                alt TEXT,
                caption TEXT,
                filename TEXT,
                mime_type TEXT,
                url TEXT,
                created_at TEXT,
                updated_at TEXT
            );
            INSERT INTO media (id, alt, filename, mime_type, url, created_at, updated_at)
            VALUES ('img1', 'Logo', 'logo.png', 'image/png', '/uploads/media/logo.png', '2024-01-01', '2024-01-01');
            INSERT INTO media (id, alt, filename, mime_type, url, created_at, updated_at)
            VALUES ('img2', 'Banner', 'banner.jpg', 'image/jpeg', '/uploads/media/banner.jpg', '2024-01-01', '2024-01-01');"
        ).unwrap();

        let media_def = CollectionDefinition {
            slug: "media".to_string(),
            labels: CollectionLabels::default(),
            timestamps: true,
            fields: vec![
                make_field("alt", FieldType::Text),
                make_field("caption", FieldType::Text),
                make_field("filename", FieldType::Text),
                make_field("mime_type", FieldType::Text),
                make_field("url", FieldType::Text),
            ],
            admin: CollectionAdmin::default(),
            hooks: CollectionHooks::default(),
            auth: None,
            upload: Some(crate::core::upload::CollectionUpload {
                enabled: true,
                mime_types: vec!["image/*".to_string()],
                max_file_size: None,
                image_sizes: vec![],
                admin_thumbnail: None,
                format_options: Default::default(),
            }),
            access: CollectionAccess::default(),
            mcp: Default::default(),
            live: None,
            versions: None,
            indexes: Vec::new(),
        };

        let mut registry = crate::core::Registry::new();
        registry.register_collection(media_def);

        let mut upload_field = make_field("image", FieldType::Upload);
        upload_field.relationship = Some(RelationshipConfig {
            collection: "media".to_string(),
            has_many: false,
            max_depth: None,
            polymorphic: vec![],
        });

        let field_defs = vec![upload_field];
        let mut sub_fields = vec![serde_json::json!({
            "name": "content[0][image]",
            "field_type": "upload",
            "value": "img1",
            "relationship_collection": "media",
        })];

        enrich_nested_fields(&mut sub_fields, &field_defs, &conn, &registry, None);

        let items = sub_fields[0]["selected_items"].as_array()
            .expect("selected_items should be populated");
        assert_eq!(items.len(), 1, "Should have 1 selected item");
        assert_eq!(items[0]["id"], "img1");
        assert_eq!(items[0]["label"], "logo.png");
    }

    #[test]
    fn enrich_nested_fields_relationship_gets_options() {
        use crate::core::collection::*;
        use crate::core::field::RelationshipConfig;

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE users (
                id TEXT PRIMARY KEY,
                name TEXT,
                created_at TEXT,
                updated_at TEXT
            );
            INSERT INTO users (id, name, created_at, updated_at)
            VALUES ('u1', 'Alice', '2024-01-01', '2024-01-01');
            INSERT INTO users (id, name, created_at, updated_at)
            VALUES ('u2', 'Bob', '2024-01-01', '2024-01-01');"
        ).unwrap();

        let users_def = CollectionDefinition {
            slug: "users".to_string(),
            labels: CollectionLabels::default(),
            timestamps: true,
            fields: vec![make_field("name", FieldType::Text)],
            admin: CollectionAdmin {
                use_as_title: Some("name".to_string()),
                ..Default::default()
            },
            hooks: CollectionHooks::default(),
            auth: None,
            upload: None,
            access: CollectionAccess::default(),
            mcp: Default::default(),
            live: None,
            versions: None,
            indexes: Vec::new(),
        };

        let mut registry = crate::core::Registry::new();
        registry.register_collection(users_def);

        let mut rel_field = make_field("author", FieldType::Relationship);
        rel_field.relationship = Some(RelationshipConfig {
            collection: "users".to_string(),
            has_many: false,
            max_depth: None,
            polymorphic: vec![],
        });

        let field_defs = vec![rel_field];
        let mut sub_fields = vec![serde_json::json!({
            "name": "items[0][author]",
            "field_type": "relationship",
            "value": "u1",
            "relationship_collection": "users",
        })];

        enrich_nested_fields(&mut sub_fields, &field_defs, &conn, &registry, None);

        let items = sub_fields[0]["selected_items"].as_array()
            .expect("selected_items should be populated");
        assert_eq!(items.len(), 1, "Should have 1 selected item");
        assert_eq!(items[0]["id"], "u1");
        assert_eq!(items[0]["label"], "Alice");
    }

    #[test]
    fn enrich_nested_fields_recurses_into_layout() {
        use crate::core::collection::*;
        use crate::core::field::RelationshipConfig;

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE tags (
                id TEXT PRIMARY KEY,
                label TEXT,
                created_at TEXT,
                updated_at TEXT
            );
            INSERT INTO tags (id, label, created_at, updated_at)
            VALUES ('t1', 'Rust', '2024-01-01', '2024-01-01');"
        ).unwrap();

        let tags_def = CollectionDefinition {
            slug: "tags".to_string(),
            labels: CollectionLabels::default(),
            timestamps: true,
            fields: vec![make_field("label", FieldType::Text)],
            admin: CollectionAdmin {
                use_as_title: Some("label".to_string()),
                ..Default::default()
            },
            hooks: CollectionHooks::default(),
            auth: None,
            upload: None,
            access: CollectionAccess::default(),
            mcp: Default::default(),
            live: None,
            versions: None,
            indexes: Vec::new(),
        };

        let mut registry = crate::core::Registry::new();
        registry.register_collection(tags_def);

        // A Row containing a Relationship field
        let mut rel_field = make_field("tag", FieldType::Relationship);
        rel_field.relationship = Some(RelationshipConfig {
            collection: "tags".to_string(),
            has_many: false,
            max_depth: None,
            polymorphic: vec![],
        });
        let row_field = FieldDefinition {
            name: "row1".to_string(),
            field_type: FieldType::Row,
            fields: vec![rel_field],
            ..Default::default()
        };

        let field_defs = vec![row_field];
        let mut sub_fields = vec![serde_json::json!({
            "name": "row1",
            "field_type": "row",
            "sub_fields": [{
                "name": "tag",
                "field_type": "relationship",
                "value": "",
                "relationship_collection": "tags",
            }],
        })];

        enrich_nested_fields(&mut sub_fields, &field_defs, &conn, &registry, None);

        let row_subs = sub_fields[0]["sub_fields"].as_array().unwrap();
        // Empty value → selected_items is empty array
        let items = row_subs[0]["selected_items"].as_array()
            .expect("Nested relationship inside Row should be enriched");
        assert_eq!(items.len(), 0, "Empty value should produce empty selected_items");
    }

    #[test]
    fn enrich_nested_fields_blocks_template_gets_upload_options() {
        // Regression: block definition templates (for new rows) must have upload options enriched
        use crate::core::collection::*;
        use crate::core::field::RelationshipConfig;

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE media (
                id TEXT PRIMARY KEY,
                filename TEXT,
                mime_type TEXT,
                url TEXT,
                created_at TEXT,
                updated_at TEXT
            );
            INSERT INTO media (id, filename, mime_type, url, created_at, updated_at)
            VALUES ('m1', 'photo.jpg', 'image/jpeg', '/uploads/photo.jpg', '2024-01-01', '2024-01-01');"
        ).unwrap();

        let media_def = CollectionDefinition {
            slug: "media".to_string(),
            labels: CollectionLabels::default(),
            timestamps: true,
            fields: vec![
                make_field("filename", FieldType::Text),
                make_field("mime_type", FieldType::Text),
                make_field("url", FieldType::Text),
            ],
            admin: CollectionAdmin::default(),
            hooks: CollectionHooks::default(),
            auth: None,
            upload: Some(crate::core::upload::CollectionUpload { enabled: true, ..Default::default() }),
            access: CollectionAccess::default(),
            mcp: Default::default(),
            live: None,
            versions: None,
            indexes: Vec::new(),
        };

        let mut registry = crate::core::Registry::new();
        registry.register_collection(media_def);

        // A Blocks field with an "image" block containing an upload field
        let mut upload_field = make_field("image", FieldType::Upload);
        upload_field.relationship = Some(RelationshipConfig {
            collection: "media".to_string(),
            has_many: false,
            max_depth: None,
            polymorphic: vec![],
        });
        let blocks_field = FieldDefinition {
            name: "content".to_string(),
            field_type: FieldType::Blocks,
            blocks: vec![BlockDefinition {
                block_type: "image".to_string(),
                fields: vec![upload_field],
                ..Default::default()
            }],
            ..Default::default()
        };

        let field_defs = vec![blocks_field];
        // Simulate the block_definitions context (as built by build_single_field_context)
        let mut sub_fields = vec![serde_json::json!({
            "name": "content",
            "field_type": "blocks",
            "block_definitions": [{
                "block_type": "image",
                "label": "Image",
                "fields": [{
                    "name": "content[__INDEX__][image]",
                    "field_type": "upload",
                    "value": "",
                    "relationship_collection": "media",
                }],
            }],
            "rows": [],
        })];

        enrich_nested_fields(&mut sub_fields, &field_defs, &conn, &registry, None);

        let block_defs = sub_fields[0]["block_definitions"].as_array().unwrap();
        let fields = block_defs[0]["fields"].as_array().unwrap();
        // Empty value → selected_items is empty array (no full table scan)
        let items = fields[0]["selected_items"].as_array()
            .expect("Upload inside block template should have selected_items");
        assert_eq!(items.len(), 0, "Empty value should produce empty selected_items");
    }

    #[test]
    fn enrich_nested_fields_array_template_gets_upload_options() {
        // Regression: array sub_fields template (for new rows) must have upload options enriched
        use crate::core::collection::*;
        use crate::core::field::RelationshipConfig;

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE media (
                id TEXT PRIMARY KEY,
                filename TEXT,
                mime_type TEXT,
                url TEXT,
                created_at TEXT,
                updated_at TEXT
            );
            INSERT INTO media (id, filename, mime_type, url, created_at, updated_at)
            VALUES ('m1', 'doc.pdf', 'application/pdf', '/uploads/doc.pdf', '2024-01-01', '2024-01-01');"
        ).unwrap();

        let media_def = CollectionDefinition {
            slug: "media".to_string(),
            labels: CollectionLabels::default(),
            timestamps: true,
            fields: vec![
                make_field("filename", FieldType::Text),
                make_field("mime_type", FieldType::Text),
                make_field("url", FieldType::Text),
            ],
            admin: CollectionAdmin::default(),
            hooks: CollectionHooks::default(),
            auth: None,
            upload: Some(crate::core::upload::CollectionUpload { enabled: true, ..Default::default() }),
            access: CollectionAccess::default(),
            mcp: Default::default(),
            live: None,
            versions: None,
            indexes: Vec::new(),
        };

        let mut registry = crate::core::Registry::new();
        registry.register_collection(media_def);

        let mut upload_field = make_field("file", FieldType::Upload);
        upload_field.relationship = Some(RelationshipConfig {
            collection: "media".to_string(),
            has_many: false,
            max_depth: None,
            polymorphic: vec![],
        });
        let array_field = FieldDefinition {
            name: "attachments".to_string(),
            field_type: FieldType::Array,
            fields: vec![upload_field],
            ..Default::default()
        };

        let field_defs = vec![array_field];
        let mut sub_fields = vec![serde_json::json!({
            "name": "attachments",
            "field_type": "array",
            "sub_fields": [{
                "name": "attachments[__INDEX__][file]",
                "field_type": "upload",
                "value": "",
                "relationship_collection": "media",
            }],
            "rows": [],
        })];

        enrich_nested_fields(&mut sub_fields, &field_defs, &conn, &registry, None);

        let template_fields = sub_fields[0]["sub_fields"].as_array().unwrap();
        // Empty value → selected_items is empty array (no full table scan)
        let items = template_fields[0]["selected_items"].as_array()
            .expect("Upload inside array template should have selected_items");
        assert_eq!(items.len(), 0, "Empty value should produce empty selected_items");
    }

    #[test]
    fn enrich_field_contexts_blocks_inside_tabs_populates_rows() {
        // Regression: blocks inside Tabs were not populated from doc_fields because
        // enrich_field_contexts delegated to enrich_nested_fields instead of recursing.
        use crate::core::field::{FieldTab, BlockDefinition};

        let blocks_field = FieldDefinition {
            name: "content".to_string(),
            field_type: FieldType::Blocks,
            blocks: vec![BlockDefinition {
                block_type: "hero".to_string(),
                label: Some(LocalizedString::Plain("Hero".to_string())),
                fields: vec![make_field("heading", FieldType::Text)],
                ..Default::default()
            }],
            ..Default::default()
        };
        let tabs_field = FieldDefinition {
            name: "page_settings".to_string(),
            field_type: FieldType::Tabs,
            tabs: vec![crate::core::field::FieldTab {
                label: "Content".to_string(),
                description: None,
                fields: vec![blocks_field.clone()],
            }],
            ..Default::default()
        };
        let field_defs = vec![tabs_field];

        // Build initial field contexts (like the template would)
        let values = HashMap::new();
        let errors = HashMap::new();
        let mut contexts = build_field_contexts(&field_defs, &values, &errors, false, false);

        // Simulate doc_fields with blocks data (as hydrate_document would produce)
        let mut doc_fields: HashMap<String, serde_json::Value> = HashMap::new();
        doc_fields.insert("content".to_string(), serde_json::json!([
            {"_block_type": "hero", "heading": "Welcome"},
        ]));

        // Construct a minimal AdminState for the test
        let tmp = tempfile::tempdir().unwrap();
        let manager = r2d2_sqlite::SqliteConnectionManager::memory();
        let pool = r2d2::Pool::builder().max_size(4).build(manager).unwrap();
        let shared_reg = std::sync::Arc::new(
            std::sync::RwLock::new(crate::core::registry::Registry::default())
        );
        let config = crate::config::CrapConfig::default();
        let hook_runner = crate::hooks::lifecycle::HookRunner::new(
            tmp.path(), shared_reg.clone(), &config,
        ).unwrap();
        let registry = std::sync::Arc::new(shared_reg.read().unwrap().clone());
        let hbs = std::sync::Arc::new(handlebars::Handlebars::new());
        let email_renderer = std::sync::Arc::new(
            crate::core::email::EmailRenderer::new(tmp.path()).unwrap()
        );
        let login_limiter = std::sync::Arc::new(
            crate::core::rate_limit::LoginRateLimiter::new(5, 300)
        );
        let translations = std::sync::Arc::new(
            crate::admin::translations::Translations::load(tmp.path())
        );
        let state = crate::admin::AdminState {
            config,
            config_dir: tmp.path().to_path_buf(),
            pool,
            registry,
            handlebars: hbs,
            hook_runner,
            jwt_secret: "test".to_string(),
            email_renderer,
            event_bus: None,
            login_limiter,
            forgot_password_limiter: std::sync::Arc::new(crate::core::rate_limit::LoginRateLimiter::new(3, 900)),
            has_auth: false,
            translations,
            shutdown: tokio_util::sync::CancellationToken::new(),
        };

        // Call enrich_field_contexts — the fix ensures Tabs recurse into Blocks
        enrich_field_contexts(
            &mut contexts, &field_defs, &doc_fields, &state,
            false, false, &errors, None,
        );

        // Verify: the blocks field inside the first tab should have populated rows
        let tabs = contexts[0]["tabs"].as_array().unwrap();
        let tab_sub_fields = tabs[0]["sub_fields"].as_array().unwrap();
        let blocks_ctx = &tab_sub_fields[0];
        assert_eq!(blocks_ctx["field_type"], "blocks");
        let rows = blocks_ctx["rows"].as_array()
            .expect("blocks inside Tabs must have rows populated from doc_fields");
        assert_eq!(rows.len(), 1, "should have 1 block row");
        assert_eq!(rows[0]["_block_type"], "hero");
    }

    #[test]
    fn enrich_field_contexts_array_inside_row_populates_rows() {
        // Regression: arrays inside Row were not populated from doc_fields
        let array_field = FieldDefinition {
            name: "items".to_string(),
            field_type: FieldType::Array,
            fields: vec![make_field("label", FieldType::Text)],
            ..Default::default()
        };
        let row_field = FieldDefinition {
            name: "main_row".to_string(),
            field_type: FieldType::Row,
            fields: vec![array_field.clone()],
            ..Default::default()
        };
        let field_defs = vec![row_field];

        let values = HashMap::new();
        let errors = HashMap::new();
        let mut contexts = build_field_contexts(&field_defs, &values, &errors, false, false);

        let mut doc_fields: HashMap<String, serde_json::Value> = HashMap::new();
        doc_fields.insert("items".to_string(), serde_json::json!([
            {"label": "First"},
            {"label": "Second"},
        ]));

        let tmp = tempfile::tempdir().unwrap();
        let manager = r2d2_sqlite::SqliteConnectionManager::memory();
        let pool = r2d2::Pool::builder().max_size(4).build(manager).unwrap();
        let shared_reg = std::sync::Arc::new(
            std::sync::RwLock::new(crate::core::registry::Registry::default())
        );
        let config = crate::config::CrapConfig::default();
        let hook_runner = crate::hooks::lifecycle::HookRunner::new(
            tmp.path(), shared_reg.clone(), &config,
        ).unwrap();
        let registry = std::sync::Arc::new(shared_reg.read().unwrap().clone());
        let hbs = std::sync::Arc::new(handlebars::Handlebars::new());
        let email_renderer = std::sync::Arc::new(
            crate::core::email::EmailRenderer::new(tmp.path()).unwrap()
        );
        let login_limiter = std::sync::Arc::new(
            crate::core::rate_limit::LoginRateLimiter::new(5, 300)
        );
        let translations = std::sync::Arc::new(
            crate::admin::translations::Translations::load(tmp.path())
        );
        let state = crate::admin::AdminState {
            config,
            config_dir: tmp.path().to_path_buf(),
            pool,
            registry,
            handlebars: hbs,
            hook_runner,
            jwt_secret: "test".to_string(),
            email_renderer,
            event_bus: None,
            login_limiter,
            forgot_password_limiter: std::sync::Arc::new(crate::core::rate_limit::LoginRateLimiter::new(3, 900)),
            has_auth: false,
            translations,
            shutdown: tokio_util::sync::CancellationToken::new(),
        };

        enrich_field_contexts(
            &mut contexts, &field_defs, &doc_fields, &state,
            false, false, &errors, None,
        );

        let row_sub_fields = contexts[0]["sub_fields"].as_array().unwrap();
        let array_ctx = &row_sub_fields[0];
        assert_eq!(array_ctx["field_type"], "array");
        let rows = array_ctx["rows"].as_array()
            .expect("array inside Row must have rows populated from doc_fields");
        assert_eq!(rows.len(), 2, "should have 2 array rows");
    }

    // ── Layout wrappers inside Array: transparent names and data ─────────

    fn make_test_state() -> crate::admin::AdminState {
        let tmp = tempfile::tempdir().unwrap();
        let manager = r2d2_sqlite::SqliteConnectionManager::memory();
        let pool = r2d2::Pool::builder().max_size(4).build(manager).unwrap();
        let shared_reg = std::sync::Arc::new(
            std::sync::RwLock::new(crate::core::registry::Registry::default())
        );
        let config = crate::config::CrapConfig::default();
        let hook_runner = crate::hooks::lifecycle::HookRunner::new(
            tmp.path(), shared_reg.clone(), &config,
        ).unwrap();
        let registry = std::sync::Arc::new(shared_reg.read().unwrap().clone());
        let hbs = std::sync::Arc::new(handlebars::Handlebars::new());
        let email_renderer = std::sync::Arc::new(
            crate::core::email::EmailRenderer::new(tmp.path()).unwrap()
        );
        let login_limiter = std::sync::Arc::new(
            crate::core::rate_limit::LoginRateLimiter::new(5, 300)
        );
        let translations = std::sync::Arc::new(
            crate::admin::translations::Translations::load(tmp.path())
        );
        crate::admin::AdminState {
            config,
            config_dir: tmp.path().to_path_buf(),
            pool,
            registry,
            handlebars: hbs,
            hook_runner,
            jwt_secret: "test".to_string(),
            email_renderer,
            event_bus: None,
            login_limiter,
            forgot_password_limiter: std::sync::Arc::new(crate::core::rate_limit::LoginRateLimiter::new(3, 900)),
            has_auth: false,
            translations,
            shutdown: tokio_util::sync::CancellationToken::new(),
        }
    }

    #[test]
    fn enriched_sub_field_tabs_in_array_transparent_names() {
        use crate::core::field::FieldTab;

        // Array "items" with sub-fields inside a Tabs wrapper
        let mut arr_field = make_field("items", FieldType::Array);
        arr_field.fields = vec![
            FieldDefinition {
                name: "layout".to_string(),
                field_type: FieldType::Tabs,
                tabs: vec![
                    FieldTab {
                        label: "General".to_string(),
                        description: None,
                        fields: vec![make_field("title", FieldType::Text)],
                    },
                    FieldTab {
                        label: "Content".to_string(),
                        description: None,
                        fields: vec![make_field("body", FieldType::Textarea)],
                    },
                ],
                ..Default::default()
            },
        ];

        // Simulate hydrated data: flat JSON (as it comes from the join table)
        let row_data = serde_json::json!([
            {"id": "r1", "title": "Hello", "body": "World"}
        ]);

        let fields = vec![arr_field.clone()];
        let values = HashMap::new();
        let errors = HashMap::new();
        let mut contexts = build_field_contexts(&fields, &values, &errors, false, false);

        let mut doc_fields = HashMap::new();
        doc_fields.insert("items".to_string(), row_data);

        let state = make_test_state();

        enrich_field_contexts(
            &mut contexts, &fields, &doc_fields, &state,
            false, false, &errors, None,
        );

        // The array row should contain a Tabs sub-field whose tabs contain the actual fields
        let rows = contexts[0]["rows"].as_array().expect("should have rows");
        assert_eq!(rows.len(), 1);

        let row_sub_fields = rows[0]["sub_fields"].as_array().unwrap();
        // The sub_fields should contain the Tabs wrapper
        assert_eq!(row_sub_fields.len(), 1);
        assert_eq!(row_sub_fields[0]["field_type"], "tabs");

        // The Tabs wrapper's name should be transparent: items[0] (not items[0][layout])
        assert_eq!(row_sub_fields[0]["name"], "items[0]");

        // Check that tab children have correct transparent names and data
        let tabs = row_sub_fields[0]["tabs"].as_array().unwrap();
        assert_eq!(tabs.len(), 2);

        let tab1_fields = tabs[0]["sub_fields"].as_array().unwrap();
        assert_eq!(tab1_fields[0]["name"], "items[0][title]");
        assert_eq!(tab1_fields[0]["value"], "Hello");

        let tab2_fields = tabs[1]["sub_fields"].as_array().unwrap();
        assert_eq!(tab2_fields[0]["name"], "items[0][body]");
        assert_eq!(tab2_fields[0]["value"], "World");
    }

    #[test]
    fn enriched_sub_field_row_in_array_transparent_names() {
        // Array "items" with sub-fields inside a Row wrapper
        let mut arr_field = make_field("items", FieldType::Array);
        arr_field.fields = vec![
            FieldDefinition {
                name: "row_wrap".to_string(),
                field_type: FieldType::Row,
                fields: vec![
                    make_field("x", FieldType::Text),
                    make_field("y", FieldType::Text),
                ],
                ..Default::default()
            },
        ];

        let row_data = serde_json::json!([
            {"id": "r1", "x": "10", "y": "20"}
        ]);

        let fields = vec![arr_field.clone()];
        let values = HashMap::new();
        let errors = HashMap::new();
        let mut contexts = build_field_contexts(&fields, &values, &errors, false, false);

        let mut doc_fields = HashMap::new();
        doc_fields.insert("items".to_string(), row_data);

        let state = make_test_state();

        enrich_field_contexts(
            &mut contexts, &fields, &doc_fields, &state,
            false, false, &errors, None,
        );

        let rows = contexts[0]["rows"].as_array().expect("should have rows");
        assert_eq!(rows.len(), 1);

        let row_sub_fields = rows[0]["sub_fields"].as_array().unwrap();
        assert_eq!(row_sub_fields.len(), 1);
        assert_eq!(row_sub_fields[0]["field_type"], "row");

        // Transparent name: items[0] (not items[0][row_wrap])
        assert_eq!(row_sub_fields[0]["name"], "items[0]");

        // Children have correct names and data
        let children = row_sub_fields[0]["sub_fields"].as_array().unwrap();
        assert_eq!(children[0]["name"], "items[0][x]");
        assert_eq!(children[0]["value"], "10");
        assert_eq!(children[1]["name"], "items[0][y]");
        assert_eq!(children[1]["value"], "20");
    }

    #[test]
    fn enriched_sub_field_row_inside_tabs_in_array_transparent_names() {
        use crate::core::field::FieldTab;

        // Array "team_members" with Tabs containing Rows (double nesting)
        let mut arr_field = make_field("team_members", FieldType::Array);
        arr_field.fields = vec![
            FieldDefinition {
                name: "member_tabs".to_string(),
                field_type: FieldType::Tabs,
                tabs: vec![
                    FieldTab {
                        label: "Personal".to_string(),
                        description: None,
                        fields: vec![
                            FieldDefinition {
                                name: "name_row".to_string(),
                                field_type: FieldType::Row,
                                fields: vec![
                                    make_field("first_name", FieldType::Text),
                                    make_field("last_name", FieldType::Text),
                                ],
                                ..Default::default()
                            },
                            make_field("email", FieldType::Email),
                        ],
                    },
                    FieldTab {
                        label: "Professional".to_string(),
                        description: None,
                        fields: vec![
                            make_field("job_title", FieldType::Text),
                        ],
                    },
                ],
                ..Default::default()
            },
        ];

        let row_data = serde_json::json!([
            {"id": "r1", "first_name": "John", "last_name": "Doe", "email": "john@example.com", "job_title": "Dev"}
        ]);

        let fields = vec![arr_field.clone()];
        let values = HashMap::new();
        let errors = HashMap::new();
        let mut contexts = build_field_contexts(&fields, &values, &errors, false, false);

        let mut doc_fields = HashMap::new();
        doc_fields.insert("team_members".to_string(), row_data);

        let state = make_test_state();

        enrich_field_contexts(
            &mut contexts, &fields, &doc_fields, &state,
            false, false, &errors, None,
        );

        let rows = contexts[0]["rows"].as_array().expect("should have rows");
        assert_eq!(rows.len(), 1);

        // Top level: Tabs wrapper (transparent name)
        let row_sub_fields = rows[0]["sub_fields"].as_array().unwrap();
        assert_eq!(row_sub_fields.len(), 1);
        assert_eq!(row_sub_fields[0]["field_type"], "tabs");
        assert_eq!(row_sub_fields[0]["name"], "team_members[0]");

        let tabs = row_sub_fields[0]["tabs"].as_array().unwrap();
        assert_eq!(tabs.len(), 2);

        // Personal tab: Row (transparent) + email
        let personal_fields = tabs[0]["sub_fields"].as_array().unwrap();
        assert_eq!(personal_fields.len(), 2);

        // Row wrapper should be transparent: team_members[0] (not team_members[0][name_row])
        assert_eq!(personal_fields[0]["field_type"], "row");
        assert_eq!(personal_fields[0]["name"], "team_members[0]");

        // Row children should be: team_members[0][first_name], team_members[0][last_name]
        let row_children = personal_fields[0]["sub_fields"].as_array().unwrap();
        assert_eq!(row_children[0]["name"], "team_members[0][first_name]");
        assert_eq!(row_children[0]["value"], "John");
        assert_eq!(row_children[1]["name"], "team_members[0][last_name]");
        assert_eq!(row_children[1]["value"], "Doe");

        // email field
        assert_eq!(personal_fields[1]["name"], "team_members[0][email]");
        assert_eq!(personal_fields[1]["value"], "john@example.com");

        // Professional tab: job_title
        let pro_fields = tabs[1]["sub_fields"].as_array().unwrap();
        assert_eq!(pro_fields[0]["name"], "team_members[0][job_title]");
        assert_eq!(pro_fields[0]["value"], "Dev");
    }
}
