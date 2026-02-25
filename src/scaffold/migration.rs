//! `make migration` command — generate migration Lua files.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Create a new migration file at `<config_dir>/migrations/YYYYMMDDHHMMSS_name.lua`.
pub fn make_migration(config_dir: &Path, name: &str) -> Result<()> {
    let migrations_dir = config_dir.join("migrations");
    fs::create_dir_all(&migrations_dir)
        .context("Failed to create migrations/ directory")?;

    let timestamp = chrono::Local::now().format("%Y%m%d%H%M%S");
    let filename = format!("{}_{}.lua", timestamp, name);
    let file_path = migrations_dir.join(&filename);

    let lua = format!(
        r#"local M = {{}}

function M.up()
    -- TODO: implement migration
    -- crap.* API available (find, create, update, delete)
end

function M.down()
    -- TODO: implement rollback (best-effort)
end

return M
"#,
    );

    fs::write(&file_path, &lua)
        .with_context(|| format!("Failed to write {}", file_path.display()))?;

    println!("Created {}", file_path.display());
    Ok(())
}
