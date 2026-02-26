//! Version-specific database operations for the `_versions_{slug}` table.

use anyhow::{Context, Result};
use rusqlite::params_from_iter;
use std::collections::HashMap;

use crate::config::LocaleConfig;
use crate::core::collection::{CollectionDefinition, GlobalDefinition};
use crate::core::document::VersionSnapshot;
use crate::core::field::FieldDefinition;

/// Build a JSON snapshot of a document's current state (fields + join data).
pub fn build_snapshot(
    conn: &rusqlite::Connection,
    slug: &str,
    fields: &[FieldDefinition],
    doc: &crate::core::Document,
) -> Result<serde_json::Value> {
    let mut data = serde_json::Map::new();
    for (k, v) in &doc.fields {
        data.insert(k.clone(), v.clone());
    }
    // Hydrate join table data into the snapshot
    let mut doc_clone = doc.clone();
    super::hydrate_document(conn, slug, fields, &mut doc_clone, None, None)?;
    for (k, v) in &doc_clone.fields {
        data.insert(k.clone(), v.clone());
    }
    if let Some(ref ts) = doc.created_at {
        data.insert("created_at".to_string(), serde_json::Value::String(ts.clone()));
    }
    if let Some(ref ts) = doc.updated_at {
        data.insert("updated_at".to_string(), serde_json::Value::String(ts.clone()));
    }
    Ok(serde_json::Value::Object(data))
}

/// Create a new version entry. Clears previous `_latest` flag, inserts new version.
pub fn create_version(
    conn: &rusqlite::Connection,
    slug: &str,
    parent_id: &str,
    status: &str,
    snapshot: &serde_json::Value,
) -> Result<VersionSnapshot> {
    let table = format!("_versions_{}", slug);
    let id = nanoid::nanoid!();

    // Get the next version number
    let next_version: i64 = conn.query_row(
        &format!("SELECT COALESCE(MAX(_version), 0) + 1 FROM {} WHERE _parent = ?1", table),
        [parent_id],
        |row| row.get(0),
    ).context("Failed to get next version number")?;

    // Clear previous _latest flag
    conn.execute(
        &format!("UPDATE {} SET _latest = 0 WHERE _parent = ?1 AND _latest = 1", table),
        [parent_id],
    ).context("Failed to clear previous latest flag")?;

    // Insert new version
    let snapshot_str = serde_json::to_string(snapshot)
        .context("Failed to serialize snapshot")?;
    conn.execute(
        &format!(
            "INSERT INTO {} (id, _parent, _version, _status, _latest, snapshot) VALUES (?1, ?2, ?3, ?4, 1, ?5)",
            table
        ),
        rusqlite::params![id, parent_id, next_version, status, snapshot_str],
    ).context("Failed to insert version")?;

    Ok(VersionSnapshot {
        id,
        parent: parent_id.to_string(),
        version: next_version,
        status: status.to_string(),
        latest: true,
        snapshot: snapshot.clone(),
        created_at: None,
        updated_at: None,
    })
}

/// Find the latest version for a parent document.
pub fn find_latest_version(
    conn: &rusqlite::Connection,
    slug: &str,
    parent_id: &str,
) -> Result<Option<VersionSnapshot>> {
    let table = format!("_versions_{}", slug);
    let mut stmt = conn.prepare(
        &format!(
            "SELECT id, _parent, _version, _status, _latest, snapshot, created_at, updated_at \
             FROM {} WHERE _parent = ?1 AND _latest = 1 LIMIT 1",
            table
        ),
    )?;
    let result = stmt.query_row([parent_id], |row| {
        let snapshot_str: String = row.get(5)?;
        Ok(VersionSnapshot {
            id: row.get(0)?,
            parent: row.get(1)?,
            version: row.get(2)?,
            status: row.get(3)?,
            latest: row.get::<_, i32>(4)? != 0,
            snapshot: serde_json::from_str(&snapshot_str).unwrap_or(serde_json::Value::Null),
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    });

    match result {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Count total versions for a parent document.
pub fn count_versions(
    conn: &rusqlite::Connection,
    slug: &str,
    parent_id: &str,
) -> Result<i64> {
    let table = format!("_versions_{}", slug);
    let count: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM {} WHERE _parent = ?1", table),
        [parent_id],
        |row| row.get(0),
    ).context("Failed to count versions")?;
    Ok(count)
}

/// List versions for a parent document, newest first.
pub fn list_versions(
    conn: &rusqlite::Connection,
    slug: &str,
    parent_id: &str,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<VersionSnapshot>> {
    let table = format!("_versions_{}", slug);
    let limit_clause = limit.map(|l| format!(" LIMIT {}", l)).unwrap_or_default();
    let offset_clause = offset.map(|o| format!(" OFFSET {}", o)).unwrap_or_default();
    let mut stmt = conn.prepare(
        &format!(
            "SELECT id, _parent, _version, _status, _latest, snapshot, created_at, updated_at \
             FROM {} WHERE _parent = ?1 ORDER BY _version DESC{}{}",
            table, limit_clause, offset_clause
        ),
    )?;
    let rows = stmt.query_map([parent_id], |row| {
        let snapshot_str: String = row.get(5)?;
        Ok(VersionSnapshot {
            id: row.get(0)?,
            parent: row.get(1)?,
            version: row.get(2)?,
            status: row.get(3)?,
            latest: row.get::<_, i32>(4)? != 0,
            snapshot: serde_json::from_str(&snapshot_str).unwrap_or(serde_json::Value::Null),
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    })?;
    let mut versions = Vec::new();
    for row in rows {
        versions.push(row?);
    }
    Ok(versions)
}

/// Find a specific version by its ID.
pub fn find_version_by_id(
    conn: &rusqlite::Connection,
    slug: &str,
    version_id: &str,
) -> Result<Option<VersionSnapshot>> {
    let table = format!("_versions_{}", slug);
    let mut stmt = conn.prepare(
        &format!(
            "SELECT id, _parent, _version, _status, _latest, snapshot, created_at, updated_at \
             FROM {} WHERE id = ?1 LIMIT 1",
            table
        ),
    )?;
    let result = stmt.query_row([version_id], |row| {
        let snapshot_str: String = row.get(5)?;
        Ok(VersionSnapshot {
            id: row.get(0)?,
            parent: row.get(1)?,
            version: row.get(2)?,
            status: row.get(3)?,
            latest: row.get::<_, i32>(4)? != 0,
            snapshot: serde_json::from_str(&snapshot_str).unwrap_or(serde_json::Value::Null),
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    });

    match result {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Convert a JSON value to a string for the data HashMap.
/// Returns None for complex types (arrays/objects) that are handled via join tables.
fn snapshot_val_to_string(val: Option<&serde_json::Value>) -> Option<String> {
    match val {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Number(n)) => Some(n.to_string()),
        Some(serde_json::Value::Bool(b)) => Some(b.to_string()),
        Some(serde_json::Value::Null) | None => Some(String::new()),
        _ => None, // complex types (arrays/objects) handled via join tables
    }
}

/// Extract flat field data from a snapshot for the UPDATE statement.
/// Group fields are always expanded to `field__subfield` sub-columns.
/// Handles both flat (`seo__meta_title`) and nested (`seo: { meta_title: ... }`) snapshot formats.
fn extract_snapshot_data(
    obj: &serde_json::Map<String, serde_json::Value>,
    fields: &[FieldDefinition],
    locales_enabled: bool,
) -> HashMap<String, String> {
    let mut data: HashMap<String, String> = HashMap::new();
    for field in fields {
        if field.field_type == crate::core::field::FieldType::Group {
            let nested_obj = obj.get(&field.name).and_then(|v| v.as_object());
            for sub in &field.fields {
                let is_localized = (field.localized || sub.localized) && locales_enabled;
                if is_localized {
                    continue;
                }
                let key = format!("{}__{}", field.name, sub.name);
                // Try flat key first, then nested path
                let val = obj.get(&key)
                    .or_else(|| nested_obj.and_then(|n| n.get(&sub.name)));
                if let Some(s) = snapshot_val_to_string(val) {
                    data.insert(key, s);
                }
            }
            continue;
        }
        if !field.has_parent_column() {
            continue;
        }
        if field.localized && locales_enabled {
            continue;
        }
        if let Some(s) = snapshot_val_to_string(obj.get(&field.name)) {
            data.insert(field.name.clone(), s);
        }
    }
    data
}

/// Restore locale columns and join table data from a snapshot.
/// Group fields are always expanded to `field__subfield` sub-columns.
fn restore_locale_and_join_data(
    conn: &rusqlite::Connection,
    table: &str,
    parent_id: &str,
    fields: &[FieldDefinition],
    obj: &serde_json::Map<String, serde_json::Value>,
    locale_config: &LocaleConfig,
) -> Result<()> {
    let locales_enabled = locale_config.is_enabled();

    // Restore localized main-table columns: clear ALL locale columns, set default from snapshot.
    if locales_enabled {
        let mut set_clauses = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        for field in fields {
            if field.field_type == crate::core::field::FieldType::Group {
                let nested_obj = obj.get(&field.name).and_then(|v| v.as_object());
                for sub in &field.fields {
                    let is_localized = field.localized || sub.localized;
                    if !is_localized { continue; }
                    let base = format!("{}__{}", field.name, sub.name);
                    // Resolve value from flat key or nested path
                    let val = obj.get(&base)
                        .or_else(|| nested_obj.and_then(|n| n.get(&sub.name)));
                    restore_locale_columns(
                        val, &base, locale_config,
                        &mut set_clauses, &mut params, &mut idx,
                    );
                }
                continue;
            }
            if !field.localized || !field.has_parent_column() { continue; }
            restore_locale_columns(
                obj.get(&field.name), &field.name, locale_config,
                &mut set_clauses, &mut params, &mut idx,
            );
        }

        if !set_clauses.is_empty() {
            let sql = format!(
                "UPDATE {} SET {} WHERE id = ?{}",
                table, set_clauses.join(", "), idx
            );
            params.push(Box::new(parent_id.to_string()));
            let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
            conn.execute(&sql, params_from_iter(param_refs.iter()))
                .context("Failed to restore locale columns")?;
        }
    }

    // Restore join table data from snapshot
    let mut join_data: HashMap<String, serde_json::Value> = HashMap::new();
    for field in fields {
        if !field.has_parent_column() {
            if let Some(v) = obj.get(&field.name) {
                join_data.insert(field.name.clone(), v.clone());
            }
        }
    }
    if !join_data.is_empty() {
        super::save_join_table_data(conn, table, fields, parent_id, &join_data, None)?;
    }

    Ok(())
}

/// Restore a version snapshot back to the main table. Updates all regular columns
/// and join tables from the snapshot data. Creates a new version recording the restore.
///
/// When `locale_config` indicates locales are enabled, localized fields are handled
/// specially: ALL locale columns are cleared, then the snapshot value is written to
/// the default locale column. This ensures stale translations from later edits don't
/// persist after restoring an older version.
pub fn restore_version(
    conn: &rusqlite::Connection,
    slug: &str,
    def: &CollectionDefinition,
    parent_id: &str,
    snapshot: &serde_json::Value,
    status: &str,
    locale_config: &LocaleConfig,
) -> Result<crate::core::Document> {
    let obj = snapshot.as_object()
        .ok_or_else(|| anyhow::anyhow!("Snapshot is not a JSON object"))?;

    let locales_enabled = locale_config.is_enabled();
    let data = extract_snapshot_data(obj, &def.fields, locales_enabled);

    // When locales are enabled, use a default locale context so that update()'s
    // internal find_by_id can read back columns with locale suffixes.
    let locale_ctx = if locales_enabled {
        Some(super::LocaleContext {
            mode: super::LocaleMode::Default,
            config: locale_config.clone(),
        })
    } else {
        None
    };
    let doc = super::update(conn, slug, def, parent_id, &data, locale_ctx.as_ref())?;

    restore_locale_and_join_data(conn, slug, parent_id, &def.fields, obj, locale_config)?;

    // Update status and create a new version for the restore
    set_document_status(conn, slug, parent_id, status)?;
    create_version(conn, slug, parent_id, status, snapshot)?;

    Ok(doc)
}

/// Helper: emit SET clauses that NULL all locale columns for a field, then set the
/// default locale column to the snapshot value.
fn restore_locale_columns(
    snapshot_val: Option<&serde_json::Value>,
    field_name: &str,
    locale_config: &LocaleConfig,
    set_clauses: &mut Vec<String>,
    params: &mut Vec<Box<dyn rusqlite::types::ToSql>>,
    idx: &mut usize,
) {
    for locale in &locale_config.locales {
        let col = format!("{}__{}", field_name, locale);
        if *locale == locale_config.default_locale {
            // Set default locale from snapshot
            match snapshot_val {
                Some(serde_json::Value::String(s)) => {
                    set_clauses.push(format!("{} = ?{}", col, idx));
                    params.push(Box::new(s.clone()));
                    *idx += 1;
                }
                Some(serde_json::Value::Number(n)) => {
                    set_clauses.push(format!("{} = ?{}", col, idx));
                    params.push(Box::new(n.to_string()));
                    *idx += 1;
                }
                Some(serde_json::Value::Bool(b)) => {
                    set_clauses.push(format!("{} = ?{}", col, idx));
                    params.push(Box::new(if *b { 1i32 } else { 0i32 }));
                    *idx += 1;
                }
                _ => {
                    set_clauses.push(format!("{} = NULL", col));
                }
            }
        } else {
            // Clear non-default locale columns
            set_clauses.push(format!("{} = NULL", col));
        }
    }
}

/// Set the `_status` column on a document in the main table.
pub fn set_document_status(
    conn: &rusqlite::Connection,
    slug: &str,
    id: &str,
    status: &str,
) -> Result<()> {
    conn.execute(
        &format!("UPDATE {} SET _status = ?1, updated_at = datetime('now') WHERE id = ?2", slug),
        rusqlite::params![status, id],
    ).with_context(|| format!("Failed to set _status on {}.{}", slug, id))?;
    Ok(())
}

/// Get the `_status` column from a document in the main table.
pub fn get_document_status(
    conn: &rusqlite::Connection,
    slug: &str,
    id: &str,
) -> Result<Option<String>> {
    let result = conn.query_row(
        &format!("SELECT _status FROM {} WHERE id = ?1", slug),
        [id],
        |row| row.get(0),
    );
    match result {
        Ok(status) => Ok(Some(status)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Delete oldest versions beyond the max_versions cap for a document.
pub fn prune_versions(
    conn: &rusqlite::Connection,
    slug: &str,
    parent_id: &str,
    max_versions: u32,
) -> Result<()> {
    if max_versions == 0 {
        return Ok(()); // unlimited
    }
    let table = format!("_versions_{}", slug);
    // Delete all versions beyond the cap, keeping the newest ones
    conn.execute(
        &format!(
            "DELETE FROM {} WHERE _parent = ?1 AND id NOT IN (\
                SELECT id FROM {} WHERE _parent = ?1 ORDER BY _version DESC LIMIT ?2\
            )",
            table, table
        ),
        rusqlite::params![parent_id, max_versions],
    ).context("Failed to prune versions")?;
    Ok(())
}

/// Restore a version snapshot back to a global's main table.
/// Group fields use expanded `field__subfield` sub-columns (same as collections).
pub fn restore_global_version(
    conn: &rusqlite::Connection,
    slug: &str,
    def: &GlobalDefinition,
    snapshot: &serde_json::Value,
    status: &str,
    locale_config: &LocaleConfig,
) -> Result<crate::core::Document> {
    let obj = snapshot.as_object()
        .ok_or_else(|| anyhow::anyhow!("Snapshot is not a JSON object"))?;

    let global_table = format!("_global_{}", slug);
    let locales_enabled = locale_config.is_enabled();
    let data = extract_snapshot_data(obj, &def.fields, locales_enabled);

    let locale_ctx = if locales_enabled {
        Some(super::LocaleContext {
            mode: super::LocaleMode::Default,
            config: locale_config.clone(),
        })
    } else {
        None
    };
    let doc = super::update_global(conn, slug, def, &data, locale_ctx.as_ref())?;

    restore_locale_and_join_data(conn, &global_table, "default", &def.fields, obj, locale_config)?;

    // Update status and create a new version for the restore
    set_document_status(conn, &global_table, "default", status)?;
    create_version(conn, &global_table, "default", status, snapshot)?;

    Ok(doc)
}
