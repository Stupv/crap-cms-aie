//! Access control checks executed within the Lua VM.

use anyhow::Result;
use mlua::{Lua, Value};
use std::collections::HashMap;

use crate::core::Document;
use crate::core::field::FieldDefinition;
use crate::db::query::{AccessResult, Filter, FilterClause, FilterOp};

use super::crud::{document_to_lua_table, lua_parse_filter_op};
use super::resolve_hook_function;

/// Check collection-level access using an already-held `&Lua` reference.
/// Does NOT lock the VM or manage TxContext — caller must ensure those are set.
/// Returns Allowed if `access_ref` is None (no restriction configured).
pub(crate) fn check_access_with_lua(
    lua: &Lua,
    access_ref: Option<&str>,
    user: Option<&Document>,
    id: Option<&str>,
    data: Option<&HashMap<String, serde_json::Value>>,
) -> Result<AccessResult> {
    let func_ref = match access_ref {
        Some(r) => r,
        None => return Ok(AccessResult::Allowed),
    };

    let func = resolve_hook_function(lua, func_ref)?;

    // Build context table: { user = ..., id = ..., data = ... }
    let ctx_table = lua.create_table()?;
    if let Some(user_doc) = user {
        let user_table = document_to_lua_table(lua, user_doc)?;
        ctx_table.set("user", user_table)?;
    }
    if let Some(doc_id) = id {
        ctx_table.set("id", doc_id)?;
    }
    if let Some(doc_data) = data {
        let data_table = lua.create_table()?;
        for (k, v) in doc_data {
            data_table.set(k.as_str(), crate::hooks::api::json_to_lua(lua, v)?)?;
        }
        ctx_table.set("data", data_table)?;
    }

    let result: Value = func.call(ctx_table)?;

    match result {
        Value::Boolean(true) => Ok(AccessResult::Allowed),
        Value::Boolean(false) | Value::Nil => Ok(AccessResult::Denied),
        Value::Table(tbl) => {
            let mut clauses = Vec::new();
            for pair in tbl.pairs::<String, Value>() {
                let (field, value) = pair?;
                match value {
                    Value::String(s) => {
                        clauses.push(FilterClause::Single(Filter {
                            field,
                            op: FilterOp::Equals(s.to_str()?.to_string()),
                        }));
                    }
                    Value::Integer(i) => {
                        clauses.push(FilterClause::Single(Filter {
                            field,
                            op: FilterOp::Equals(i.to_string()),
                        }));
                    }
                    Value::Number(n) => {
                        clauses.push(FilterClause::Single(Filter {
                            field,
                            op: FilterOp::Equals(n.to_string()),
                        }));
                    }
                    Value::Table(op_tbl) => {
                        for op_pair in op_tbl.pairs::<String, Value>() {
                            let (op_name, op_val) = op_pair?;
                            let op = lua_parse_filter_op(&op_name, &op_val)?;
                            clauses.push(FilterClause::Single(Filter {
                                field: field.clone(),
                                op,
                            }));
                        }
                    }
                    _ => {}
                }
            }
            Ok(AccessResult::Constrained(clauses))
        }
        _ => Ok(AccessResult::Denied),
    }
}

/// Check field-level read access using an already-held `&Lua` reference.
/// Returns a list of field names that should be stripped (denied fields).
pub(crate) fn check_field_read_access_with_lua(
    lua: &Lua,
    fields: &[FieldDefinition],
    user: Option<&Document>,
) -> Vec<String> {
    let mut denied = Vec::new();
    for field in fields {
        if let Some(ref read_ref) = field.access.read {
            match check_access_with_lua(lua, Some(read_ref), user, None, None) {
                Ok(AccessResult::Allowed) | Ok(AccessResult::Constrained(_)) => {}
                Ok(AccessResult::Denied) => denied.push(field.name.clone()),
                Err(e) => {
                    tracing::warn!("field access check error for {}: {}", field.name, e);
                    denied.push(field.name.clone());
                }
            }
        }
    }
    denied
}

/// Check field-level write access using an already-held `&Lua` reference.
/// Returns a list of field names that should be stripped from the input.
pub(crate) fn check_field_write_access_with_lua(
    lua: &Lua,
    fields: &[FieldDefinition],
    user: Option<&Document>,
    operation: &str,
) -> Vec<String> {
    let mut denied = Vec::new();
    for field in fields {
        let access_ref = match operation {
            "create" => field.access.create.as_deref(),
            "update" => field.access.update.as_deref(),
            _ => None,
        };
        if let Some(ref_str) = access_ref {
            match check_access_with_lua(lua, Some(ref_str), user, None, None) {
                Ok(AccessResult::Allowed) | Ok(AccessResult::Constrained(_)) => {}
                Ok(AccessResult::Denied) => denied.push(field.name.clone()),
                Err(e) => {
                    tracing::warn!("field write access check error for {}: {}", field.name, e);
                    denied.push(field.name.clone());
                }
            }
        }
    }
    denied
}
