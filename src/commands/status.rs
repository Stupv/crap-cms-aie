//! `status` command — show project status (collections, globals, migrations).

use anyhow::{Context, Result};
use std::path::Path;

/// Print project status: collections, globals, migrations, DB info.
pub fn run(config_dir: &Path) -> Result<()> {
    let config_dir = config_dir.canonicalize().unwrap_or_else(|_| config_dir.to_path_buf());

    let cfg = crate::config::CrapConfig::load(&config_dir)
        .context("Failed to load config")?;
    let registry = crate::hooks::init_lua(&config_dir, &cfg)
        .context("Failed to initialize Lua VM")?;
    let pool = crate::db::pool::create_pool(&config_dir, &cfg)
        .context("Failed to create database pool")?;

    crate::db::migrate::sync_all(&pool, &registry, &cfg.locale)
        .context("Failed to sync database schema")?;

    let reg = registry.read()
        .map_err(|e| anyhow::anyhow!("Registry lock poisoned: {}", e))?;

    // Config dir
    println!("Config:  {}", config_dir.display());

    // DB file + size
    let db_path = cfg.db_path(&config_dir);
    let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
    println!("Database: {} ({} bytes)", db_path.display(), db_size);
    println!();

    // Collections with row counts
    let conn = pool.get().context("Failed to get database connection")?;

    if reg.collections.is_empty() {
        println!("Collections: (none)");
    } else {
        println!("Collections:");
        let mut slugs: Vec<_> = reg.collections.keys().collect();
        slugs.sort();
        for slug in slugs {
            let def = &reg.collections[slug];
            let count = crate::db::query::count(&conn, slug, def, &[], None).unwrap_or(0);
            let mut tags = Vec::new();
            if def.is_auth_collection() {
                tags.push("auth");
            }
            if def.is_upload_collection() {
                tags.push("upload");
            }
            let tag_str = if tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", tags.join(", "))
            };
            println!("  {:<20} {} row(s){}", slug, count, tag_str);
        }
    }
    println!();

    // Globals
    if reg.globals.is_empty() {
        println!("Globals: (none)");
    } else {
        println!("Globals:");
        let mut slugs: Vec<_> = reg.globals.keys().collect();
        slugs.sort();
        for slug in slugs {
            println!("  {}", slug);
        }
    }
    println!();

    // Migrations
    let migrations_dir = config_dir.join("migrations");
    let all_files = crate::db::migrate::list_migration_files(&migrations_dir).unwrap_or_default();
    let applied = crate::db::migrate::get_applied_migrations(&pool).unwrap_or_default();
    let pending = all_files.iter().filter(|f| !applied.contains(*f)).count();

    println!("Migrations: {} total, {} applied, {} pending",
        all_files.len(), applied.len(), pending);

    Ok(())
}
