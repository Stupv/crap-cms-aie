//! Global document query functions.

use anyhow::{Context, Result};
use rusqlite::params_from_iter;
use std::collections::HashMap;

use crate::core::Document;
use crate::core::collection::GlobalDefinition;
use crate::core::field::FieldType;
use crate::db::document::row_to_document;
use super::{
    LocaleMode, LocaleContext,
    get_locale_select_columns, group_locale_fields,
    locale_write_column, coerce_value,
};

/// Get the single global document from `_global_{slug}`.
pub fn get_global(conn: &rusqlite::Connection, slug: &str, def: &GlobalDefinition, locale_ctx: Option<&LocaleContext>) -> Result<Document> {
    let table_name = format!("_global_{}", slug);

    let (select_exprs, result_names) = match locale_ctx {
        Some(ctx) if ctx.config.is_enabled() => get_locale_select_columns(&def.fields, true, ctx),
        _ => {
            let names = get_global_column_names(def);
            (names.clone(), names)
        }
    };

    let sql = format!("SELECT {} FROM {} WHERE id = 'default'", select_exprs.join(", "), table_name);

    let mut doc = conn.query_row(&sql, [], |row| {
        row_to_document(row, &result_names)
    }).with_context(|| format!("Failed to get global '{}'", slug))?;

    if let Some(ctx) = locale_ctx {
        if ctx.config.is_enabled() {
            if let LocaleMode::All = ctx.mode {
                group_locale_fields(&mut doc, &def.fields, &ctx.config);
            }
        }
    }

    Ok(doc)
}

/// Update the single global document in `_global_{slug}`. Returns the updated document.
pub fn update_global(
    conn: &rusqlite::Connection,
    slug: &str,
    def: &GlobalDefinition,
    data: &HashMap<String, String>,
    locale_ctx: Option<&LocaleContext>,
) -> Result<Document> {
    let table_name = format!("_global_{}", slug);
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let mut set_clauses = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    for field in &def.fields {
        let col_name = locale_write_column(&field.name, field, &locale_ctx);
        if let Some(value) = data.get(&field.name) {
            set_clauses.push(format!("{} = ?{}", col_name, idx));
            params.push(coerce_value(&field.field_type, value));
            idx += 1;
        } else if field.field_type == FieldType::Checkbox {
            set_clauses.push(format!("{} = ?{}", col_name, idx));
            params.push(Box::new(0i32));
            idx += 1;
        }
    }

    set_clauses.push(format!("updated_at = ?{}", idx));
    params.push(Box::new(now));

    if set_clauses.is_empty() {
        return get_global(conn, slug, def, locale_ctx);
    }

    let sql = format!(
        "UPDATE {} SET {} WHERE id = 'default'",
        table_name,
        set_clauses.join(", ")
    );

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    conn.execute(&sql, params_from_iter(param_refs.iter()))
        .with_context(|| format!("Failed to update global '{}'", slug))?;

    get_global(conn, slug, def, locale_ctx)
}

fn get_global_column_names(def: &GlobalDefinition) -> Vec<String> {
    let mut names = vec!["id".to_string()];
    for field in &def.fields {
        names.push(field.name.clone());
    }
    names.push("created_at".to_string());
    names.push("updated_at".to_string());
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use crate::core::collection::*;
    use crate::core::field::*;

    fn global_def() -> GlobalDefinition {
        GlobalDefinition {
            slug: "settings".to_string(),
            labels: CollectionLabels::default(),
            fields: vec![
                FieldDefinition {
                    name: "site_name".to_string(),
                    field_type: FieldType::Text,
                    required: false,
                    unique: false,
                    validate: None,
                    default_value: None,
                    options: vec![],
                    admin: FieldAdmin::default(),
                    hooks: FieldHooks::default(),
                    access: FieldAccess::default(),
                    relationship: None,
                    fields: vec![],
                    blocks: vec![],
                    localized: false,
                    picker_appearance: None,
                },
                FieldDefinition {
                    name: "tagline".to_string(),
                    field_type: FieldType::Text,
                    required: false,
                    unique: false,
                    validate: None,
                    default_value: None,
                    options: vec![],
                    admin: FieldAdmin::default(),
                    hooks: FieldHooks::default(),
                    access: FieldAccess::default(),
                    relationship: None,
                    fields: vec![],
                    blocks: vec![],
                    localized: false,
                    picker_appearance: None,
                },
            ],
            hooks: CollectionHooks::default(),
            access: CollectionAccess::default(),
            live: None,
        }
    }

    fn setup_global_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE _global_settings (
                id TEXT PRIMARY KEY,
                site_name TEXT,
                tagline TEXT,
                created_at TEXT,
                updated_at TEXT
            );
            INSERT INTO _global_settings (id, site_name, tagline, created_at, updated_at)
            VALUES ('default', NULL, NULL, '2024-01-01', '2024-01-01');"
        ).unwrap();
        conn
    }

    #[test]
    fn get_global_default_row() {
        let conn = setup_global_db();
        let def = global_def();
        let doc = get_global(&conn, "settings", &def, None).unwrap();
        assert_eq!(doc.id, "default");
        assert!(doc.created_at.is_some(), "created_at should be present");
        assert!(doc.updated_at.is_some(), "updated_at should be present");
        assert_eq!(doc.created_at.as_deref(), Some("2024-01-01"));
        assert_eq!(doc.updated_at.as_deref(), Some("2024-01-01"));
    }

    #[test]
    fn update_global_sets_field() {
        let conn = setup_global_db();
        let def = global_def();
        let mut data = HashMap::new();
        data.insert("site_name".to_string(), "My Site".to_string());
        let doc = update_global(&conn, "settings", &def, &data, None).unwrap();
        assert_eq!(doc.get_str("site_name"), Some("My Site"));
    }

    #[test]
    fn update_global_preserves_unset() {
        let conn = setup_global_db();
        let def = global_def();

        // First update: set site_name
        let mut data1 = HashMap::new();
        data1.insert("site_name".to_string(), "My Site".to_string());
        update_global(&conn, "settings", &def, &data1, None).unwrap();

        // Second update: set only tagline
        let mut data2 = HashMap::new();
        data2.insert("tagline".to_string(), "A great site".to_string());
        let doc = update_global(&conn, "settings", &def, &data2, None).unwrap();

        assert_eq!(doc.get_str("site_name"), Some("My Site"), "site_name should be preserved");
        assert_eq!(doc.get_str("tagline"), Some("A great site"), "tagline should be set");
    }

    #[test]
    fn update_global_updates_timestamp() {
        let conn = setup_global_db();
        let def = global_def();

        let before = get_global(&conn, "settings", &def, None).unwrap();
        assert_eq!(before.updated_at.as_deref(), Some("2024-01-01"));

        let mut data = HashMap::new();
        data.insert("site_name".to_string(), "New Name".to_string());
        let after = update_global(&conn, "settings", &def, &data, None).unwrap();

        assert_ne!(
            after.updated_at.as_deref(),
            Some("2024-01-01"),
            "updated_at should have changed after update"
        );
        assert!(after.updated_at.is_some(), "updated_at should be present");
    }
}
