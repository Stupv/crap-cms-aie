//! `make job` command — generate job Lua files.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Scaffold a job Lua file in `jobs/<slug>.lua`.
///
/// Generates `crap.jobs.define()` with the handler function stub.
pub fn make_job(
    config_dir: &Path,
    slug: &str,
    schedule: Option<&str>,
    queue: Option<&str>,
    retries: Option<u32>,
    timeout: Option<u64>,
    force: bool,
) -> Result<()> {
    super::validate_slug(slug)?;

    let jobs_dir = config_dir.join("jobs");
    fs::create_dir_all(&jobs_dir)
        .context("Failed to create jobs/ directory")?;

    let file_path = jobs_dir.join(format!("{}.lua", slug));
    if file_path.exists() && !force {
        anyhow::bail!(
            "File '{}' already exists — use --force to overwrite",
            file_path.display()
        );
    }

    let label = super::to_title_case(slug);
    let handler_ref = format!("jobs.{}.run", slug);

    // Build optional config lines
    let mut config_lines = Vec::new();
    config_lines.push(format!("    handler = \"{}\",", handler_ref));
    if let Some(sched) = schedule {
        config_lines.push(format!("    schedule = \"{}\",", sched));
    }
    if let Some(q) = queue {
        if q != "default" {
            config_lines.push(format!("    queue = \"{}\",", q));
        }
    }
    if let Some(r) = retries {
        if r > 0 {
            config_lines.push(format!("    retries = {},", r));
        }
    }
    if let Some(t) = timeout {
        if t != 60 {
            config_lines.push(format!("    timeout = {},", t));
        }
    }
    config_lines.push(format!("    labels = {{ singular = \"{}\" }},", label));

    let config_body = config_lines.join("\n");

    let lua = format!(
        r#"crap.jobs.define("{slug}", {{
{config_body}
}})

local M = {{}}

---@param ctx crap.JobHandlerContext
---@return table?
function M.run(ctx)
    -- ctx.data = input data from queue() or {{}} for cron
    -- ctx.job  = {{ slug, attempt, max_attempts }}
    -- Full CRUD access: crap.collections.find(), .create(), etc.

    -- TODO: implement
    return nil
end

return M
"#,
        slug = slug,
        config_body = config_body,
    );

    fs::write(&file_path, &lua)
        .with_context(|| format!("Failed to write {}", file_path.display()))?;

    println!("Created {}", file_path.display());
    println!();
    println!("Handler ref: {}", handler_ref);

    if schedule.is_some() {
        println!();
        println!("This job has a cron schedule and will run automatically.");
    } else {
        println!();
        println!("Queue from hooks:");
        println!("  crap.jobs.queue(\"{}\", {{ key = \"value\" }})", slug);
        println!();
        println!("Or trigger from CLI:");
        println!("  crap-cms jobs trigger <config> {}", slug);
    }

    Ok(())
}
