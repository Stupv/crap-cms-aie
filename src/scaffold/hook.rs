//! `make hook` command — generate hook Lua files.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Hook type for the `make hook` command.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HookType {
    Collection,
    Field,
    Access,
}

impl HookType {
    /// Parse from string (CLI input).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "collection" => Some(Self::Collection),
            "field" => Some(Self::Field),
            "access" => Some(Self::Access),
            _ => None,
        }
    }

    /// Valid lifecycle positions for this hook type.
    pub fn valid_positions(&self) -> &'static [&'static str] {
        match self {
            Self::Collection => &[
                "before_validate", "before_change", "after_change",
                "before_read", "after_read",
                "before_delete", "after_delete", "before_broadcast",
            ],
            Self::Field => &[
                "before_validate", "before_change", "after_change", "after_read",
            ],
            Self::Access => &["read", "create", "update", "delete"],
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Collection => "collection",
            Self::Field => "field",
            Self::Access => "access",
        }
    }
}

/// Options for `make_hook()`. Fully resolved — no prompts.
pub struct MakeHookOptions<'a> {
    pub config_dir: &'a Path,
    pub name: &'a str,
    pub hook_type: HookType,
    pub collection: &'a str,
    pub position: &'a str,
    pub field: Option<&'a str>,
    pub force: bool,
}

/// Generate a hook file at `<config_dir>/hooks/<collection>/<name>.lua`.
///
/// Creates a single-function file that returns the function directly (no module table).
/// The template varies by hook type (collection, field, or access).
pub fn make_hook(opts: &MakeHookOptions) -> Result<()> {
    // Validate inputs
    super::validate_slug(opts.collection)?;
    if opts.name.is_empty() || !opts.name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        anyhow::bail!(
            "Invalid hook name '{}' — use alphanumeric characters and underscores only",
            opts.name
        );
    }
    if !opts.hook_type.valid_positions().contains(&opts.position) {
        anyhow::bail!(
            "Invalid position '{}' for {} hook — valid: {}",
            opts.position,
            opts.hook_type.label(),
            opts.hook_type.valid_positions().join(", ")
        );
    }
    if opts.hook_type == HookType::Field && opts.field.is_none() {
        anyhow::bail!("Field hooks require --field to be specified");
    }

    let hooks_dir = opts.config_dir.join("hooks").join(opts.collection);
    fs::create_dir_all(&hooks_dir)
        .context("Failed to create hooks/ subdirectory")?;

    let file_path = hooks_dir.join(format!("{}.lua", opts.name));
    if file_path.exists() && !opts.force {
        anyhow::bail!(
            "File '{}' already exists — use --force to overwrite",
            file_path.display()
        );
    }

    let lua = match opts.hook_type {
        HookType::Collection => format!(
            r#"--- {position} hook for {collection}.
---@param context crap.HookContext
---@return crap.HookContext
return function(context)
    -- TODO: implement
    return context
end
"#,
            position = opts.position,
            collection = opts.collection,
        ),
        HookType::Field => format!(
            r#"--- {position} field hook for {collection}.{field}.
---@param value any
---@param context crap.FieldHookContext
---@return any
return function(value, context)
    -- TODO: implement
    return value
end
"#,
            position = opts.position,
            collection = opts.collection,
            field = opts.field.unwrap_or("?"),
        ),
        HookType::Access => format!(
            r#"--- {position} access control for {collection}.
---@param context crap.AccessContext
---@return boolean | table
return function(context)
    -- TODO: implement
    return true
end
"#,
            position = opts.position,
            collection = opts.collection,
        ),
    };

    fs::write(&file_path, &lua)
        .with_context(|| format!("Failed to write {}", file_path.display()))?;

    let hook_ref = format!("hooks.{}.{}", opts.collection, opts.name);

    println!("Created {}", file_path.display());
    println!();
    println!("Hook ref: {}", hook_ref);
    println!();

    match opts.hook_type {
        HookType::Collection => {
            println!("Add to your collection definition:");
            println!("  hooks = {{");
            println!("      {} = {{ \"{}\" }},", opts.position, hook_ref);
            println!("  }},");
        }
        HookType::Field => {
            println!("Add to your field definition:");
            println!("  hooks = {{");
            println!("      {} = {{ \"{}\" }},", opts.position, hook_ref);
            println!("  }},");
        }
        HookType::Access => {
            println!("Add to your collection definition:");
            println!("  access = {{");
            println!("      {} = \"{}\",", opts.position, hook_ref);
            println!("  }},");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn make_hook_opts<'a>(
        config_dir: &'a Path,
        name: &'a str,
        hook_type: HookType,
        collection: &'a str,
        position: &'a str,
        field: Option<&'a str>,
        force: bool,
    ) -> MakeHookOptions<'a> {
        MakeHookOptions { config_dir, name, hook_type, collection, position, field, force }
    }

    #[test]
    fn test_make_hook_collection() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let opts = make_hook_opts(
            tmp.path(), "auto_slug", HookType::Collection,
            "posts", "before_change", None, false,
        );
        make_hook(&opts).unwrap();

        let content = fs::read_to_string(tmp.path().join("hooks/posts/auto_slug.lua")).unwrap();
        assert!(content.contains("before_change hook for posts"));
        assert!(content.contains("crap.HookContext"));
        assert!(content.contains("return function(context)"));
    }

    #[test]
    fn test_make_hook_field() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let opts = make_hook_opts(
            tmp.path(), "normalize", HookType::Field,
            "posts", "before_validate", Some("title"), false,
        );
        make_hook(&opts).unwrap();

        let content = fs::read_to_string(tmp.path().join("hooks/posts/normalize.lua")).unwrap();
        assert!(content.contains("before_validate field hook for posts.title"));
        assert!(content.contains("crap.FieldHookContext"));
        assert!(content.contains("return function(value, context)"));
    }

    #[test]
    fn test_make_hook_access() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let opts = make_hook_opts(
            tmp.path(), "admin_only", HookType::Access,
            "posts", "read", None, false,
        );
        make_hook(&opts).unwrap();

        let content = fs::read_to_string(tmp.path().join("hooks/posts/admin_only.lua")).unwrap();
        assert!(content.contains("read access control for posts"));
        assert!(content.contains("crap.AccessContext"));
        assert!(content.contains("return true"));
    }

    #[test]
    fn test_make_hook_refuses_overwrite() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let opts = make_hook_opts(
            tmp.path(), "auto_slug", HookType::Collection,
            "posts", "before_change", None, false,
        );
        make_hook(&opts).unwrap();
        let result = make_hook(&opts);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("--force"));
    }

    #[test]
    fn test_make_hook_force_overwrite() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let opts = make_hook_opts(
            tmp.path(), "auto_slug", HookType::Collection,
            "posts", "before_change", None, false,
        );
        make_hook(&opts).unwrap();
        let opts_force = make_hook_opts(
            tmp.path(), "auto_slug", HookType::Collection,
            "posts", "before_change", None, true,
        );
        assert!(make_hook(&opts_force).is_ok());
    }

    #[test]
    fn test_make_hook_invalid_position() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let opts = make_hook_opts(
            tmp.path(), "bad", HookType::Collection,
            "posts", "invalid_position", None, false,
        );
        let result = make_hook(&opts);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid position"));
    }

    #[test]
    fn test_make_hook_invalid_name() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let opts = make_hook_opts(
            tmp.path(), "", HookType::Collection,
            "posts", "before_change", None, false,
        );
        assert!(make_hook(&opts).is_err());

        let opts2 = make_hook_opts(
            tmp.path(), "bad-name", HookType::Collection,
            "posts", "before_change", None, false,
        );
        assert!(make_hook(&opts2).is_err());
    }

    #[test]
    fn test_make_hook_field_requires_field_name() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let opts = make_hook_opts(
            tmp.path(), "hook", HookType::Field,
            "posts", "before_validate", None, false,
        );
        let result = make_hook(&opts);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("--field"));
    }
}
