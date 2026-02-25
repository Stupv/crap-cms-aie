//! CLI entrypoint for Crap CMS. Parses flags, loads config, and starts the admin + gRPC servers.
//!
//! Subcommands: `serve`, `status`, `user`, `make`, `blueprint`, `db`, `typegen`, `proto`,
//! `migrate`, `backup`, `export`, `import`, `init`, `templates`.
//! Running bare `crap-cms` prints help.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod config;
mod core;
mod db;
mod hooks;
mod admin;
mod api;
mod scheduler;
mod scaffold;
mod service;
mod typegen;
mod commands;

/// Parse a key=value pair for --field arguments.
fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let pos = s.find('=')
        .ok_or_else(|| format!("invalid KEY=VALUE: no `=` found in `{s}`"))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

#[derive(Parser)]
#[command(name = "crap-cms", about = "Crap CMS - Headless CMS with Lua hooks", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the admin UI and gRPC servers
    Serve {
        /// Path to the config directory
        config: PathBuf,

        /// Run in the background (detached)
        #[arg(short, long)]
        detach: bool,
    },

    /// Show project status (collections, globals, migrations)
    Status {
        /// Path to the config directory
        config: PathBuf,
    },

    /// User management for auth collections
    #[command(name = "user")]
    User {
        #[command(subcommand)]
        action: UserAction,
    },

    /// Scaffold a new config directory
    Init {
        /// Directory to create (default: ./crap-cms)
        dir: Option<PathBuf>,
    },

    /// Generate scaffolding files (collection, global, hook, migration)
    Make {
        #[command(subcommand)]
        action: MakeAction,
    },

    /// Manage saved blueprints
    Blueprint {
        #[command(subcommand)]
        action: BlueprintAction,
    },

    /// Generate typed definitions from collection schemas
    Typegen {
        /// Path to the config directory
        config: PathBuf,

        /// Output language: lua, ts, go, py, rs (default: lua). Use "all" for all languages.
        #[arg(short, long, default_value = "lua")]
        lang: String,
    },

    /// Export the embedded content.proto file for gRPC client codegen
    Proto {
        /// Output path (file or directory). Omit to write to stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run database migrations
    #[command(name = "migrate")]
    Migrate {
        /// Path to the config directory
        config: PathBuf,

        #[command(subcommand)]
        action: MigrateAction,
    },

    /// Backup database and optionally uploads
    Backup {
        /// Path to the config directory
        config: PathBuf,

        /// Output directory (default: <config_dir>/backups/)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Also compress the uploads directory
        #[arg(short, long)]
        include_uploads: bool,
    },

    /// Database tools
    Db {
        #[command(subcommand)]
        action: DbAction,
    },

    /// Export collection data to JSON
    Export {
        /// Path to the config directory
        config: PathBuf,

        /// Export only this collection (default: all)
        #[arg(short, long)]
        collection: Option<String>,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Import collection data from JSON
    Import {
        /// Path to the config directory
        config: PathBuf,

        /// JSON file to import
        file: PathBuf,

        /// Import only this collection (default: all in file)
        #[arg(short, long)]
        collection: Option<String>,
    },

    /// List and extract default admin templates and static files
    Templates {
        #[command(subcommand)]
        action: TemplatesAction,
    },

    /// Manage background jobs
    Jobs {
        #[command(subcommand)]
        action: JobsAction,
    },
}

#[derive(Subcommand)]
enum MakeAction {
    /// Generate a collection Lua file
    Collection {
        /// Path to the config directory
        config: PathBuf,

        /// Collection slug (e.g., "posts"). Prompted if omitted.
        slug: Option<String>,

        /// Inline field shorthand (e.g., "title:text:required,status:select,body:textarea")
        #[arg(short = 'F', long)]
        fields: Option<String>,

        /// Set timestamps = false
        #[arg(short = 'T', long)]
        no_timestamps: bool,

        /// Enable auth (email/password login)
        #[arg(long)]
        auth: bool,

        /// Enable uploads (file upload collection)
        #[arg(long)]
        upload: bool,

        /// Enable versioning (draft/publish workflow)
        #[arg(long)]
        versions: bool,

        /// Non-interactive mode — skip all prompts, use flags and defaults only
        #[arg(long)]
        no_input: bool,

        /// Overwrite existing file
        #[arg(short, long)]
        force: bool,
    },

    /// Generate a global Lua file
    Global {
        /// Path to the config directory
        config: PathBuf,

        /// Global slug (e.g., "site_settings"). Prompted if omitted.
        slug: Option<String>,

        /// Overwrite existing file
        #[arg(short, long)]
        force: bool,
    },

    /// Generate a hook file (file-per-hook pattern)
    Hook {
        /// Path to the config directory
        config: PathBuf,

        /// Hook function name (e.g., "auto_slug"). Prompted if omitted.
        name: Option<String>,

        /// Hook type: collection, field, or access
        #[arg(short = 't', long = "type")]
        hook_type: Option<String>,

        /// Target collection slug
        #[arg(short, long)]
        collection: Option<String>,

        /// Lifecycle position (e.g., before_change, after_read)
        #[arg(short = 'l', long)]
        position: Option<String>,

        /// Target field name (field hooks only)
        #[arg(short = 'F', long)]
        field: Option<String>,

        /// Overwrite existing file
        #[arg(long)]
        force: bool,
    },

    /// Generate a job Lua file
    Job {
        /// Path to the config directory
        config: PathBuf,

        /// Job slug (e.g., "cleanup_expired"). Prompted if omitted.
        slug: Option<String>,

        /// Cron schedule expression (e.g., "0 3 * * *")
        #[arg(short, long)]
        schedule: Option<String>,

        /// Queue name (default: "default")
        #[arg(short, long)]
        queue: Option<String>,

        /// Max retry attempts (default: 0)
        #[arg(short, long)]
        retries: Option<u32>,

        /// Timeout in seconds (default: 60)
        #[arg(short, long)]
        timeout: Option<u64>,

        /// Overwrite existing file
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum BlueprintAction {
    /// Save a config directory as a reusable blueprint
    Save {
        /// Path to the config directory
        config: PathBuf,

        /// Blueprint name (e.g., "blog", "saas-starter")
        name: String,

        /// Overwrite existing blueprint
        #[arg(short, long)]
        force: bool,
    },

    /// Create a new project from a saved blueprint
    Use {
        /// Blueprint name to use. Prompted if omitted.
        name: Option<String>,

        /// Directory to create (default: ./crap-cms)
        dir: Option<PathBuf>,
    },

    /// List all saved blueprints
    List,

    /// Remove a saved blueprint
    Remove {
        /// Blueprint name to remove. Prompted if omitted.
        name: Option<String>,
    },
}

#[derive(Subcommand)]
enum UserAction {
    /// Create a new user in an auth collection
    Create {
        /// Path to the config directory
        config: PathBuf,

        /// Auth collection slug
        #[arg(short, long, default_value = "users")]
        collection: String,

        /// User email
        #[arg(short, long)]
        email: Option<String>,

        /// User password (omit for interactive prompt)
        #[arg(short, long)]
        password: Option<String>,

        /// Extra fields as key=value pairs (repeatable)
        #[arg(short, long = "field", value_parser = parse_key_val)]
        fields: Vec<(String, String)>,
    },

    /// List users in an auth collection
    List {
        /// Path to the config directory
        config: PathBuf,

        /// Auth collection slug
        #[arg(short, long, default_value = "users")]
        collection: String,
    },

    /// Delete a user from an auth collection
    Delete {
        /// Path to the config directory
        config: PathBuf,

        /// Auth collection slug
        #[arg(short, long, default_value = "users")]
        collection: String,

        /// User email
        #[arg(short, long)]
        email: Option<String>,

        /// User ID
        #[arg(long)]
        id: Option<String>,

        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        confirm: bool,
    },

    /// Lock a user account (prevent login)
    Lock {
        /// Path to the config directory
        config: PathBuf,

        /// Auth collection slug
        #[arg(short, long, default_value = "users")]
        collection: String,

        /// User email
        #[arg(short, long)]
        email: Option<String>,

        /// User ID
        #[arg(long)]
        id: Option<String>,
    },

    /// Unlock a user account (allow login)
    Unlock {
        /// Path to the config directory
        config: PathBuf,

        /// Auth collection slug
        #[arg(short, long, default_value = "users")]
        collection: String,

        /// User email
        #[arg(short, long)]
        email: Option<String>,

        /// User ID
        #[arg(long)]
        id: Option<String>,
    },

    /// Change a user's password
    ChangePassword {
        /// Path to the config directory
        config: PathBuf,

        /// Auth collection slug
        #[arg(short, long, default_value = "users")]
        collection: String,

        /// User email
        #[arg(short, long)]
        email: Option<String>,

        /// User ID
        #[arg(long)]
        id: Option<String>,

        /// New password (omit for interactive prompt)
        #[arg(short, long)]
        password: Option<String>,
    },
}

#[derive(Subcommand)]
enum MigrateAction {
    /// Create a new migration file
    Create {
        /// Migration name (e.g., "add_categories")
        name: String,
    },
    /// Schema sync + run pending Lua data migrations
    Up,
    /// Rollback last N data migrations
    Down {
        /// Number of migrations to roll back
        #[arg(short, long, default_value = "1")]
        steps: usize,
    },
    /// Show all migration files with applied/pending status
    List,
    /// Drop all tables, recreate from Lua definitions, run all migrations
    Fresh {
        /// Required confirmation flag (destructive operation)
        #[arg(short = 'y', long)]
        confirm: bool,
    },
}

#[derive(Subcommand)]
enum DbAction {
    /// Open an interactive SQLite console
    Console {
        /// Path to the config directory
        config: PathBuf,
    },
}

#[derive(Subcommand)]
enum TemplatesAction {
    /// List all available default templates and static files
    List {
        /// Filter: "templates" or "static" (default: both)
        #[arg(short, long)]
        r#type: Option<String>,
    },
    /// Extract default files into the config directory for customization
    Extract {
        /// Path to the config directory
        config: PathBuf,
        /// File paths to extract (e.g., "layout/base.hbs" "styles.css")
        paths: Vec<String>,
        /// Extract all files
        #[arg(short, long)]
        all: bool,
        /// Filter: "templates" or "static" (default: both, only with --all)
        #[arg(short, long)]
        r#type: Option<String>,
        /// Overwrite existing files
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum JobsAction {
    /// List defined jobs and recent runs
    List {
        /// Path to the config directory
        config: PathBuf,
    },
    /// Trigger a job manually
    Trigger {
        /// Path to the config directory
        config: PathBuf,
        /// Job slug to trigger
        slug: String,
        /// JSON data to pass to the job (default: "{}")
        #[arg(short, long)]
        data: Option<String>,
    },
    /// Show job run history
    Status {
        /// Path to the config directory
        config: PathBuf,
        /// Show a single job run by ID
        #[arg(long)]
        id: Option<String>,
        /// Filter by job slug
        #[arg(short, long)]
        slug: Option<String>,
        /// Max results to show
        #[arg(short, long, default_value = "20")]
        limit: i64,
    },
    /// Clean up old completed/failed job runs
    Purge {
        /// Path to the config directory
        config: PathBuf,
        /// Delete runs older than this (e.g., "7d", "24h", "30m")
        #[arg(long, default_value = "7d")]
        older_than: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing subscriber early so all commands get logging.
    // RUST_LOG env overrides. Default: crap_cms=debug for serve, info for others.
    let default_filter = match &cli.command {
        Command::Serve { .. } => "crap_cms=debug,info",
        _ => "crap_cms=info,warn",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter)),
        )
        .init();

    match cli.command {
        Command::Serve { config, detach } => {
            if detach {
                return commands::serve::detach(&config);
            }
            commands::serve::run(&config).await
        }
        Command::Status { config } => commands::status::run(&config),
        Command::User { action } => commands::user::run(action),
        Command::Init { dir } => commands::init::run(dir),
        Command::Make { action } => commands::make::run(action),
        Command::Blueprint { action } => match action {
            BlueprintAction::Save { config, name, force } => {
                scaffold::blueprint_save(&config, &name, force)
            }
            BlueprintAction::Use { name, dir } => {
                let name = match name {
                    Some(n) => n,
                    None => {
                        use dialoguer::Select;
                        let names = scaffold::list_blueprint_names()?;
                        if names.is_empty() {
                            anyhow::bail!("No blueprints saved yet.\nSave one with: crap-cms blueprint save <dir> <name>");
                        }
                        let selection = Select::new()
                            .with_prompt("Select blueprint")
                            .items(&names)
                            .interact()
                            .context("Failed to read blueprint selection")?;
                        names[selection].clone()
                    }
                };
                scaffold::blueprint_use(&name, dir)
            }
            BlueprintAction::List => scaffold::blueprint_list(),
            BlueprintAction::Remove { name } => {
                let name = match name {
                    Some(n) => n,
                    None => {
                        use dialoguer::Select;
                        let names = scaffold::list_blueprint_names()?;
                        if names.is_empty() {
                            anyhow::bail!("No blueprints saved yet.");
                        }
                        let selection = Select::new()
                            .with_prompt("Select blueprint to remove")
                            .items(&names)
                            .interact()
                            .context("Failed to read blueprint selection")?;
                        names[selection].clone()
                    }
                };
                scaffold::blueprint_remove(&name)
            }
        },
        Command::Typegen { config, lang } => commands::typegen::run(&config, &lang),
        Command::Proto { output } => scaffold::proto_export(output.as_deref()),
        Command::Migrate { config, action } => commands::db::migrate(&config, action),
        Command::Backup { config, output, include_uploads } => {
            commands::db::backup(&config, output, include_uploads)
        }
        Command::Db { action } => match action {
            DbAction::Console { config } => commands::db::console(&config),
        },
        Command::Export { config, collection, output } => {
            commands::export::export(&config, collection, output)
        }
        Command::Import { config, file, collection } => {
            commands::export::import(&config, &file, collection)
        }
        Command::Templates { action } => commands::templates::run(action),
        Command::Jobs { action } => commands::jobs::run(action),
    }
}
