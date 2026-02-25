//! `make global` command — generate global Lua files.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Generate a global Lua file at `<config_dir>/globals/<slug>.lua`.
pub fn make_global(config_dir: &Path, slug: &str, force: bool) -> Result<()> {
    super::validate_slug(slug)?;

    let globals_dir = config_dir.join("globals");
    fs::create_dir_all(&globals_dir)
        .context("Failed to create globals/ directory")?;

    let file_path = globals_dir.join(format!("{}.lua", slug));
    if file_path.exists() && !force {
        anyhow::bail!(
            "File '{}' already exists — use --force to overwrite",
            file_path.display()
        );
    }

    let label = super::to_title_case(slug);

    let lua = format!(
        r#"crap.globals.define("{slug}", {{
    labels = {{
        singular = "{label}",
    }},
    fields = {{
        {{
            name = "title",
            type = "text",
            required = true,
        }},
    }},
}})
"#,
        slug = slug,
        label = label,
    );

    fs::write(&file_path, &lua)
        .with_context(|| format!("Failed to write {}", file_path.display()))?;

    println!("Created {}", file_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_make_global() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_global(tmp.path(), "site_settings", false).unwrap();

        let content = fs::read_to_string(tmp.path().join("globals/site_settings.lua")).unwrap();
        assert!(content.contains("crap.globals.define(\"site_settings\""));
        assert!(content.contains("singular = \"Site Settings\""));
    }
}
