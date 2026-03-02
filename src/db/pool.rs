//! SQLite connection pool with WAL mode and tuned pragmas.

use anyhow::{Context, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::Path;
use std::time::Duration;

use crate::config::CrapConfig;

/// r2d2 connection pool for SQLite.
pub type DbPool = Pool<SqliteConnectionManager>;

/// Create a connection pool, ensuring the database directory exists.
pub fn create_pool(config_dir: &Path, config: &CrapConfig) -> Result<DbPool> {
    let db_path = config.db_path(config_dir);

    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create database directory: {}", parent.display()))?;
    }

    tracing::info!("Database path: {}", db_path.display());

    let manager = SqliteConnectionManager::file(&db_path);

    let pool = Pool::builder()
        .max_size(config.database.pool_max_size)
        .min_idle(Some(1))
        .connection_timeout(Duration::from_secs(5))
        .connection_customizer(Box::new(SqlitePragmas {
            busy_timeout_ms: config.database.busy_timeout_ms,
        }))
        .test_on_check_out(false)
        .build(manager)
        .context("Failed to create connection pool")?;

    Ok(pool)
}

#[derive(Debug)]
struct SqlitePragmas {
    busy_timeout_ms: u64,
}

impl r2d2::CustomizeConnection<rusqlite::Connection, rusqlite::Error> for SqlitePragmas {
    fn on_acquire(&self, conn: &mut rusqlite::Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(&format!(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = {};
             PRAGMA wal_autocheckpoint = 1000;",
            self.busy_timeout_ms
        ))?;
        Ok(())
    }
}
