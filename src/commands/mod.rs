//! CLI command handlers. Each submodule handles one top-level subcommand.

pub mod serve;
pub mod init;
pub mod status;
pub mod user;
pub mod make;
pub mod jobs;
pub mod db;
pub mod export;
pub mod templates;
pub mod typegen;

use anyhow::{Context, Result};
use std::path::Path;

/// Load config, init Lua, create pool, and sync schema. Shared by user, export, import commands.
pub fn load_config_and_sync(config_dir: &Path) -> Result<(crate::db::DbPool, crate::core::SharedRegistry)> {
    let config_dir = config_dir.canonicalize().unwrap_or_else(|_| config_dir.to_path_buf());

    let cfg = crate::config::CrapConfig::load(&config_dir)
        .context("Failed to load config")?;
    let registry = crate::hooks::init_lua(&config_dir, &cfg)
        .context("Failed to initialize Lua VM")?;
    let pool = crate::db::pool::create_pool(&config_dir, &cfg)
        .context("Failed to create database pool")?;

    crate::db::migrate::sync_all(&pool, &registry, &cfg.locale)
        .context("Failed to sync database schema")?;

    Ok((pool, registry))
}
