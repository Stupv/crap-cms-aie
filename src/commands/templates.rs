//! `templates` command — list and extract default admin templates and static files.

use anyhow::Result;

/// Handle the `templates` subcommand.
pub fn run(action: crate::TemplatesAction) -> Result<()> {
    match action {
        crate::TemplatesAction::List { r#type } => {
            crate::scaffold::templates_list(r#type.as_deref())
        }
        crate::TemplatesAction::Extract { config, paths, all, r#type, force } => {
            crate::scaffold::templates_extract(&config, &paths, all, r#type.as_deref(), force)
        }
    }
}
