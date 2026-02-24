//! CLI entrypoint for Crap CMS. Parses flags, loads config, and starts the admin + gRPC servers.
//!
//! Subcommands: `serve`, `status`, `user`, `make`, `blueprint`, `db`, `typegen`, `proto`,
//! `migrate`, `backup`, `export`, `import`, `init`.
//! Running bare `crap-cms` prints help.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn, error};

mod config;
mod core;
mod db;
mod hooks;
mod admin;
mod api;
mod scaffold;
mod service;
mod typegen;

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
}

#[derive(Subcommand)]
enum MakeAction {
    /// Generate a collection Lua file
    Collection {
        /// Path to the config directory
        config: PathBuf,

        /// Collection slug (e.g., "posts")
        slug: String,

        /// Inline field shorthand (e.g., "title:text:required,status:select,body:textarea")
        #[arg(short = 'F', long)]
        fields: Option<String>,

        /// Set timestamps = false
        #[arg(short = 'T', long)]
        no_timestamps: bool,

        /// Overwrite existing file
        #[arg(short, long)]
        force: bool,
    },

    /// Generate a global Lua file
    Global {
        /// Path to the config directory
        config: PathBuf,

        /// Global slug (e.g., "site_settings")
        slug: String,

        /// Overwrite existing file
        #[arg(short, long)]
        force: bool,
    },

    /// Generate a hook stub (name as module.function, e.g., "posts.auto_slug")
    Hook {
        /// Path to the config directory
        config: PathBuf,

        /// Hook name as module.function (e.g., "posts.auto_slug")
        name: String,
    },

    /// Generate a new migration file
    Migration {
        /// Path to the config directory
        config: PathBuf,

        /// Migration name (e.g., "backfill_slugs")
        name: String,
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
        /// Blueprint name to use
        name: String,

        /// Directory to create (default: ./crap-cms)
        dir: Option<PathBuf>,
    },

    /// List all saved blueprints
    List,

    /// Remove a saved blueprint
    Remove {
        /// Blueprint name to remove
        name: String,
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Serve { config, detach } => {
            if detach {
                return detach_serve(&config);
            }
            serve_command(&config).await
        }
        Command::Status { config } => {
            status_command(&config)
        }
        Command::User { action } => {
            user_command(action)
        }
        Command::Init { dir } => {
            scaffold::init(dir)
        }
        Command::Make { action } => match action {
            MakeAction::Collection { config, slug, fields, no_timestamps, force } => {
                scaffold::make_collection(&config, &slug, fields.as_deref(), no_timestamps, force)
            }
            MakeAction::Global { config, slug, force } => {
                scaffold::make_global(&config, &slug, force)
            }
            MakeAction::Hook { config, name } => {
                scaffold::make_hook(&config, &name)
            }
            MakeAction::Migration { config, name } => {
                scaffold::make_migration(&config, &name)
            }
        },
        Command::Blueprint { action } => match action {
            BlueprintAction::Save { config, name, force } => {
                scaffold::blueprint_save(&config, &name, force)
            }
            BlueprintAction::Use { name, dir } => {
                scaffold::blueprint_use(&name, dir)
            }
            BlueprintAction::List => {
                scaffold::blueprint_list()
            }
            BlueprintAction::Remove { name } => {
                scaffold::blueprint_remove(&name)
            }
        },
        Command::Typegen { config, lang } => {
            typegen_command(&config, &lang)
        }
        Command::Proto { output } => {
            scaffold::proto_export(output.as_deref())
        }
        Command::Migrate { config, action } => {
            migrate_command(&config, action)
        }
        Command::Backup { config, output, include_uploads } => {
            backup_command(&config, output, include_uploads)
        }
        Command::Db { action } => match action {
            DbAction::Console { config } => {
                db_console_command(&config)
            }
        },
        Command::Export { config, collection, output } => {
            export_command(&config, collection, output)
        }
        Command::Import { config, file, collection } => {
            import_command(&config, &file, collection)
        }
    }
}

// ── serve command ─────────────────────────────────────────────────────────

/// Re-exec the current binary as a detached background process.
fn detach_serve(config_dir: &Path) -> Result<()> {
    let exe = std::env::current_exe().context("Failed to determine executable path")?;

    let child = std::process::Command::new(&exe)
        .arg("serve")
        .arg(config_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to spawn detached process")?;

    println!("Started crap-cms in background (PID {})", child.id());
    Ok(())
}

/// Start the admin UI and gRPC servers.
async fn serve_command(config_dir: &Path) -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("crap_cms=debug,info")),
        )
        .init();

    let config_dir = config_dir.canonicalize().unwrap_or_else(|_| config_dir.to_path_buf());

    info!("Config directory: {}", config_dir.display());

    // Load config
    let cfg = config::CrapConfig::load(&config_dir)
        .context("Failed to load config")?;
    info!(?cfg, "Configuration loaded");

    // Initialize Lua VM and load collections/globals
    let registry = hooks::init_lua(&config_dir, &cfg)
        .context("Failed to initialize Lua VM")?;

    {
        let reg = registry.read()
            .map_err(|e| anyhow::anyhow!("Registry lock poisoned: {}", e))?;
        info!("Loaded {} collection(s), {} global(s)",
            reg.collections.len(), reg.globals.len());
        for (slug, col) in &reg.collections {
            info!("  Collection '{}': {} field(s)", slug, col.fields.len());
        }
    }

    // Auto-generate Lua type definitions on startup
    {
        let reg = registry.read()
            .map_err(|e| anyhow::anyhow!("Registry lock poisoned: {}", e))?;
        match typegen::generate(&config_dir, &reg) {
            Ok(path) => info!("Generated type definitions: {}", path.display()),
            Err(e) => warn!("Failed to generate type definitions: {}", e),
        }
    }

    // Initialize database
    let pool = db::pool::create_pool(&config_dir, &cfg)
        .context("Failed to create database pool")?;

    // Sync database schema from Lua definitions
    db::migrate::sync_all(&pool, &registry, &cfg.locale)
        .context("Failed to sync database schema")?;

    // Initialize Lua hook runner (with registry for CRUD access in hooks)
    let hook_runner = hooks::lifecycle::HookRunner::new(&config_dir, registry.clone(), &cfg)?;

    // Run on_init hooks (synchronous — failure aborts startup)
    if !cfg.hooks.on_init.is_empty() {
        info!("Running on_init hooks...");
        let conn = pool.get().context("DB connection for on_init")?;
        hook_runner.run_system_hooks_with_conn(&cfg.hooks.on_init, &conn)
            .context("on_init hooks failed")?;
        info!("on_init hooks completed");
    }

    // Resolve JWT secret
    let jwt_secret = if cfg.auth.secret.is_empty() {
        let secret = nanoid::nanoid!(64);
        warn!("No auth.secret in crap.toml — generated random JWT secret (tokens won't survive restarts)");
        secret
    } else {
        cfg.auth.secret.clone()
    };

    // Log auth collection info
    {
        let reg = registry.read()
            .map_err(|e| anyhow::anyhow!("Registry lock poisoned: {}", e))?;
        let auth_collections: Vec<_> = reg.collections.values()
            .filter(|d| d.is_auth_collection())
            .map(|d| d.slug.as_str())
            .collect();
        if auth_collections.is_empty() {
            info!("No auth collections — admin UI and API are open");
        } else {
            info!("Auth collections: {:?} — admin login required", auth_collections);
        }
    }

    // Create EventBus for live updates (if enabled)
    let event_bus = if cfg.live.enabled {
        let bus = core::event::EventBus::new(cfg.live.channel_capacity);
        info!("Live event streaming enabled (capacity: {})", cfg.live.channel_capacity);
        Some(bus)
    } else {
        info!("Live event streaming disabled");
        None
    };

    // Start servers
    let admin_addr = format!("{}:{}", cfg.server.host, cfg.server.admin_port);
    let grpc_addr = format!("{}:{}", cfg.server.host, cfg.server.grpc_port);

    info!("Starting Admin UI on http://{}", admin_addr);
    info!("Starting gRPC API on {}", grpc_addr);

    let admin_handle = admin::server::start(
        &admin_addr,
        cfg.clone(),
        config_dir.clone(),
        pool.clone(),
        registry.clone(),
        hook_runner.clone(),
        jwt_secret.clone(),
        event_bus.clone(),
    );

    let grpc_handle = api::start_server(
        &grpc_addr,
        pool.clone(),
        registry.clone(),
        hook_runner.clone(),
        jwt_secret,
        &cfg.depth,
        &cfg,
        &config_dir,
        event_bus,
    );

    tokio::try_join!(admin_handle, grpc_handle)
        .map_err(|e| {
            error!("Server error: {}", e);
            e
        })?;

    Ok(())
}

// ── status command ────────────────────────────────────────────────────────

/// Print project status: collections, globals, migrations, DB info.
fn status_command(config_dir: &Path) -> Result<()> {
    let config_dir = config_dir.canonicalize().unwrap_or_else(|_| config_dir.to_path_buf());

    let cfg = config::CrapConfig::load(&config_dir)
        .context("Failed to load config")?;
    let registry = hooks::init_lua(&config_dir, &cfg)
        .context("Failed to initialize Lua VM")?;
    let pool = db::pool::create_pool(&config_dir, &cfg)
        .context("Failed to create database pool")?;

    db::migrate::sync_all(&pool, &registry, &cfg.locale)
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
            let count = db::query::count(&conn, slug, def, &[], None).unwrap_or(0);
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
    let all_files = db::migrate::list_migration_files(&migrations_dir).unwrap_or_default();
    let applied = db::migrate::get_applied_migrations(&pool).unwrap_or_default();
    let pending = all_files.iter().filter(|f| !applied.contains(*f)).count();

    println!("Migrations: {} total, {} applied, {} pending",
        all_files.len(), applied.len(), pending);

    Ok(())
}

// ── db console command ────────────────────────────────────────────────────

/// Open an interactive SQLite console.
fn db_console_command(config_dir: &Path) -> Result<()> {
    let config_dir = config_dir.canonicalize().unwrap_or_else(|_| config_dir.to_path_buf());

    let cfg = config::CrapConfig::load(&config_dir)
        .context("Failed to load config")?;

    let db_path = cfg.db_path(&config_dir);
    if !db_path.exists() {
        anyhow::bail!("Database file not found: {}", db_path.display());
    }

    println!("Opening SQLite console: {}", db_path.display());

    let status = std::process::Command::new("sqlite3")
        .arg(&db_path)
        .status()
        .context("Failed to launch sqlite3 — is it installed?")?;

    if !status.success() {
        anyhow::bail!("sqlite3 exited with status {}", status);
    }

    Ok(())
}

// ── user commands ─────────────────────────────────────────────────────────

/// Dispatch user management subcommands.
fn user_command(action: UserAction) -> Result<()> {
    match action {
        UserAction::Create { config, collection, email, password, fields } => {
            let (pool, registry) = load_config_and_sync(&config)?;
            user_create(&pool, &registry, &collection, email, password, fields)
        }
        UserAction::List { config, collection } => {
            let (pool, registry) = load_config_and_sync(&config)?;
            user_list(&pool, &registry, &collection)
        }
        UserAction::Delete { config, collection, email, id, confirm } => {
            let (pool, registry) = load_config_and_sync(&config)?;
            user_delete(&pool, &registry, &collection, email, id, confirm)
        }
        UserAction::Lock { config, collection, email, id } => {
            let (pool, registry) = load_config_and_sync(&config)?;
            user_lock(&pool, &registry, &collection, email, id)
        }
        UserAction::Unlock { config, collection, email, id } => {
            let (pool, registry) = load_config_and_sync(&config)?;
            user_unlock(&pool, &registry, &collection, email, id)
        }
        UserAction::ChangePassword { config, collection, email, id, password } => {
            let (pool, registry) = load_config_and_sync(&config)?;
            user_change_password(&pool, &registry, &collection, email, id, password)
        }
    }
}

/// Load config, init Lua, create pool, and sync schema. Used by user and other commands.
fn load_config_and_sync(config_dir: &Path) -> Result<(db::DbPool, core::SharedRegistry)> {
    let config_dir = config_dir.canonicalize().unwrap_or_else(|_| config_dir.to_path_buf());

    let cfg = config::CrapConfig::load(&config_dir)
        .context("Failed to load config")?;
    let registry = hooks::init_lua(&config_dir, &cfg)
        .context("Failed to initialize Lua VM")?;
    let pool = db::pool::create_pool(&config_dir, &cfg)
        .context("Failed to create database pool")?;

    db::migrate::sync_all(&pool, &registry, &cfg.locale)
        .context("Failed to sync database schema")?;

    Ok((pool, registry))
}

/// Resolve a user by --email or --id. Returns (def, document).
fn resolve_user(
    pool: &db::DbPool,
    registry: &core::SharedRegistry,
    collection: &str,
    email: Option<String>,
    id: Option<String>,
) -> Result<(core::CollectionDefinition, core::Document)> {
    let reg = registry.read()
        .map_err(|e| anyhow::anyhow!("Registry lock poisoned: {}", e))?;

    let def = reg.get_collection(collection)
        .ok_or_else(|| anyhow::anyhow!("Collection '{}' not found", collection))?;

    if !def.is_auth_collection() {
        anyhow::bail!("Collection '{}' is not an auth collection (auth must be enabled)", collection);
    }

    let def = def.clone();
    drop(reg);

    let conn = pool.get().context("Failed to get database connection")?;

    if let Some(email) = email {
        let doc = db::query::find_by_email(&conn, collection, &def, &email)?
            .ok_or_else(|| anyhow::anyhow!("No user found with email '{}' in '{}'", email, collection))?;
        Ok((def, doc))
    } else if let Some(id) = id {
        let doc = db::query::find_by_id(&conn, collection, &def, &id, None)?
            .ok_or_else(|| anyhow::anyhow!("No user found with id '{}' in '{}'", id, collection))?;
        Ok((def, doc))
    } else {
        anyhow::bail!("Either --email or --id is required to identify a user")
    }
}

/// Create a new user in an auth collection.
fn user_create(
    pool: &db::DbPool,
    registry: &core::SharedRegistry,
    collection: &str,
    email: Option<String>,
    password: Option<String>,
    fields: Vec<(String, String)>,
) -> Result<()> {
    let reg = registry.read()
        .map_err(|e| anyhow::anyhow!("Registry lock poisoned: {}", e))?;

    let def = reg.get_collection(collection)
        .ok_or_else(|| anyhow::anyhow!("Collection '{}' not found", collection))?;

    if !def.is_auth_collection() {
        anyhow::bail!("Collection '{}' is not an auth collection (auth must be enabled)", collection);
    }

    let def = def.clone();
    drop(reg);

    // Get email — from flag or interactive prompt
    let email = match email {
        Some(e) => e,
        None => {
            eprint!("Email: ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)
                .context("Failed to read email")?;
            let trimmed = input.trim().to_string();
            if trimmed.is_empty() {
                anyhow::bail!("Email cannot be empty");
            }
            trimmed
        }
    };

    // Get password — from flag or interactive prompt
    let password = match password {
        Some(p) => {
            eprintln!("Warning: password provided via command line — it may be visible in shell history");
            p
        }
        None => {
            eprint!("Password: ");
            let p1 = rpassword::read_password()
                .context("Failed to read password")?;
            if p1.is_empty() {
                anyhow::bail!("Password cannot be empty");
            }
            eprint!("Confirm password: ");
            let p2 = rpassword::read_password()
                .context("Failed to read password confirmation")?;
            if p1 != p2 {
                anyhow::bail!("Passwords do not match");
            }
            p1
        }
    };

    // Build data map from email + extra --field args
    let mut data: HashMap<String, String> = fields.into_iter().collect();
    data.insert("email".to_string(), email);

    // Prompt for any required fields not already provided
    for field in &def.fields {
        if field.name == "email" {
            continue; // already handled above
        }
        if field.field_type == core::field::FieldType::Checkbox {
            continue; // absent checkbox = false, always valid
        }
        if data.contains_key(&field.name) {
            continue; // already provided via --field
        }
        if !field.required && field.default_value.is_none() {
            continue; // optional with no default — skip
        }
        // Use default_value if available and field is not required
        if !field.required {
            if let Some(ref dv) = field.default_value {
                let val = match dv {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                data.insert(field.name.clone(), val);
                continue;
            }
        }
        // Required field with a default — use it automatically
        if let Some(ref dv) = field.default_value {
            let val = match dv {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            eprint!("{} [{}]: ", field.name, val);
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)
                .with_context(|| format!("Failed to read {}", field.name))?;
            let trimmed = input.trim();
            if trimmed.is_empty() {
                data.insert(field.name.clone(), val);
            } else {
                data.insert(field.name.clone(), trimmed.to_string());
            }
            continue;
        }
        // Required field, no default — must prompt
        eprint!("{}: ", field.name);
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)
            .with_context(|| format!("Failed to read {}", field.name))?;
        let trimmed = input.trim().to_string();
        if trimmed.is_empty() {
            anyhow::bail!("{} is required", field.name);
        }
        data.insert(field.name.clone(), trimmed);
    }

    // Create user in a transaction
    let mut conn = pool.get().context("Failed to get database connection")?;
    let tx = conn.transaction().context("Failed to begin transaction")?;

    let doc = db::query::create(&tx, collection, &def, &data, None)
        .context("Failed to create user")?;

    db::query::update_password(&tx, collection, &doc.id, &password)
        .context("Failed to set password")?;

    tx.commit().context("Failed to commit transaction")?;

    println!("Created user {} in '{}'", doc.id, collection);

    Ok(())
}

/// List users in an auth collection.
fn user_list(
    pool: &db::DbPool,
    registry: &core::SharedRegistry,
    collection: &str,
) -> Result<()> {
    let reg = registry.read()
        .map_err(|e| anyhow::anyhow!("Registry lock poisoned: {}", e))?;

    let def = reg.get_collection(collection)
        .ok_or_else(|| anyhow::anyhow!("Collection '{}' not found", collection))?;

    if !def.is_auth_collection() {
        anyhow::bail!("Collection '{}' is not an auth collection (auth must be enabled)", collection);
    }

    let def = def.clone();
    let verify_email = def.auth.as_ref().map(|a| a.verify_email).unwrap_or(false);
    drop(reg);

    let conn = pool.get().context("Failed to get database connection")?;

    let query = db::query::FindQuery {
        filters: vec![],
        order_by: None,
        limit: None,
        offset: None,
        select: None,
    };

    let users = db::query::find(&conn, collection, &def, &query, None)?;

    if users.is_empty() {
        println!("No users in '{}'.", collection);
        return Ok(());
    }

    // Print header
    if verify_email {
        println!("{:<24} {:<30} {:<8} {:<8}", "ID", "Email", "Locked", "Verified");
        println!("{}", "-".repeat(72));
    } else {
        println!("{:<24} {:<30} {:<8}", "ID", "Email", "Locked");
        println!("{}", "-".repeat(64));
    }

    for user in &users {
        let email = user.fields.get("email")
            .and_then(|v| v.as_str())
            .unwrap_or("-");
        let locked = db::query::is_locked(&conn, collection, &user.id).unwrap_or(false);
        let locked_str = if locked { "yes" } else { "no" };

        if verify_email {
            let verified = db::query::is_verified(&conn, collection, &user.id).unwrap_or(false);
            let verified_str = if verified { "yes" } else { "no" };
            println!("{:<24} {:<30} {:<8} {:<8}", user.id, email, locked_str, verified_str);
        } else {
            println!("{:<24} {:<30} {:<8}", user.id, email, locked_str);
        }
    }

    println!("\n{} user(s)", users.len());

    Ok(())
}

/// Delete a user from an auth collection.
fn user_delete(
    pool: &db::DbPool,
    registry: &core::SharedRegistry,
    collection: &str,
    email: Option<String>,
    id: Option<String>,
    confirm: bool,
) -> Result<()> {
    let (_, doc) = resolve_user(pool, registry, collection, email, id)?;

    let user_email = doc.fields.get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    if !confirm {
        eprint!("Delete user {} ({}) from '{}'? [y/N] ", doc.id, user_email, collection);
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)
            .context("Failed to read confirmation")?;
        if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let conn = pool.get().context("Failed to get database connection")?;
    db::query::delete(&conn, collection, &doc.id)
        .context("Failed to delete user")?;

    println!("Deleted user {} ({}) from '{}'", doc.id, user_email, collection);

    Ok(())
}

/// Lock a user account.
fn user_lock(
    pool: &db::DbPool,
    registry: &core::SharedRegistry,
    collection: &str,
    email: Option<String>,
    id: Option<String>,
) -> Result<()> {
    let (_, doc) = resolve_user(pool, registry, collection, email, id)?;

    let conn = pool.get().context("Failed to get database connection")?;
    db::query::lock_user(&conn, collection, &doc.id)
        .context("Failed to lock user")?;

    let user_email = doc.fields.get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    println!("Locked user {} ({}) in '{}'", doc.id, user_email, collection);

    Ok(())
}

/// Unlock a user account.
fn user_unlock(
    pool: &db::DbPool,
    registry: &core::SharedRegistry,
    collection: &str,
    email: Option<String>,
    id: Option<String>,
) -> Result<()> {
    let (_, doc) = resolve_user(pool, registry, collection, email, id)?;

    let conn = pool.get().context("Failed to get database connection")?;
    db::query::unlock_user(&conn, collection, &doc.id)
        .context("Failed to unlock user")?;

    let user_email = doc.fields.get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    println!("Unlocked user {} ({}) in '{}'", doc.id, user_email, collection);

    Ok(())
}

/// Change a user's password.
fn user_change_password(
    pool: &db::DbPool,
    registry: &core::SharedRegistry,
    collection: &str,
    email: Option<String>,
    id: Option<String>,
    password: Option<String>,
) -> Result<()> {
    let (_, doc) = resolve_user(pool, registry, collection, email, id)?;

    let password = match password {
        Some(p) => {
            eprintln!("Warning: password provided via command line — it may be visible in shell history");
            p
        }
        None => {
            eprint!("New password: ");
            let p1 = rpassword::read_password()
                .context("Failed to read password")?;
            if p1.is_empty() {
                anyhow::bail!("Password cannot be empty");
            }
            eprint!("Confirm password: ");
            let p2 = rpassword::read_password()
                .context("Failed to read password confirmation")?;
            if p1 != p2 {
                anyhow::bail!("Passwords do not match");
            }
            p1
        }
    };

    let conn = pool.get().context("Failed to get database connection")?;
    db::query::update_password(&conn, collection, &doc.id, &password)
        .context("Failed to update password")?;

    let user_email = doc.fields.get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    println!("Password changed for user {} ({}) in '{}'", doc.id, user_email, collection);

    Ok(())
}

// ── export command ────────────────────────────────────────────────────────

/// Export collection data to JSON.
fn export_command(
    config_dir: &Path,
    collection_filter: Option<String>,
    output: Option<PathBuf>,
) -> Result<()> {
    let (pool, registry) = load_config_and_sync(config_dir)?;

    let reg = registry.read()
        .map_err(|e| anyhow::anyhow!("Registry lock poisoned: {}", e))?;

    let conn = pool.get().context("Failed to get database connection")?;

    let mut collections_data = serde_json::Map::new();

    let slugs: Vec<String> = if let Some(ref slug) = collection_filter {
        if reg.get_collection(slug).is_none() {
            anyhow::bail!("Collection '{}' not found", slug);
        }
        vec![slug.clone()]
    } else {
        let mut s: Vec<_> = reg.collections.keys().cloned().collect();
        s.sort();
        s
    };

    for slug in &slugs {
        let def = &reg.collections[slug];

        let query = db::query::FindQuery {
            filters: vec![],
            order_by: None,
            limit: None,
            offset: None,
            select: None,
        };

        let mut docs = db::query::find(&conn, slug, def, &query, None)?;

        for doc in &mut docs {
            db::query::hydrate_document(&conn, slug, def, doc, None)?;
        }

        let docs_json: Vec<serde_json::Value> = docs.into_iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()?;

        collections_data.insert(slug.clone(), serde_json::Value::Array(docs_json));
    }

    let output_json = serde_json::json!({ "collections": collections_data });

    match output {
        Some(path) => {
            let content = serde_json::to_string_pretty(&output_json)?;
            std::fs::write(&path, content)
                .with_context(|| format!("Failed to write {}", path.display()))?;
            eprintln!("Exported {} collection(s) to {}", slugs.len(), path.display());
        }
        None => {
            println!("{}", serde_json::to_string_pretty(&output_json)?);
        }
    }

    Ok(())
}

// ── import command ────────────────────────────────────────────────────────

/// Import collection data from JSON.
fn import_command(
    config_dir: &Path,
    file: &Path,
    collection_filter: Option<String>,
) -> Result<()> {
    let (pool, registry) = load_config_and_sync(config_dir)?;

    let content = std::fs::read_to_string(file)
        .with_context(|| format!("Failed to read {}", file.display()))?;
    let data: serde_json::Value = serde_json::from_str(&content)
        .context("Failed to parse JSON")?;

    let collections_obj = data.get("collections")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow::anyhow!("Expected top-level \"collections\" object in JSON"))?;

    let reg = registry.read()
        .map_err(|e| anyhow::anyhow!("Registry lock poisoned: {}", e))?;

    let slugs: Vec<String> = if let Some(ref slug) = collection_filter {
        if !collections_obj.contains_key(slug) {
            anyhow::bail!("Collection '{}' not found in import file", slug);
        }
        vec![slug.clone()]
    } else {
        collections_obj.keys().cloned().collect()
    };

    let mut total_imported = 0usize;

    for slug in &slugs {
        let def = reg.get_collection(slug)
            .ok_or_else(|| anyhow::anyhow!("Collection '{}' exists in import file but not in schema", slug))?;

        let docs_array = collections_obj.get(slug)
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Expected array for collection '{}'", slug))?;

        let mut conn = pool.get().context("Failed to get database connection")?;
        let tx = conn.transaction().context("Failed to begin transaction")?;

        for doc_val in docs_array {
            let doc_obj = doc_val.as_object()
                .ok_or_else(|| anyhow::anyhow!("Expected document object in '{}'", slug))?;

            let id = doc_obj.get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Document missing 'id' in '{}'", slug))?;

            // Separate parent-column fields from join-table fields
            let mut parent_cols: Vec<String> = vec!["id".to_string()];
            let mut parent_vals: Vec<String> = vec![id.to_string()];
            let mut join_data: HashMap<String, serde_json::Value> = HashMap::new();

            // Handle timestamps
            if def.timestamps {
                if let Some(v) = doc_obj.get("created_at").and_then(|v| v.as_str()) {
                    parent_cols.push("created_at".to_string());
                    parent_vals.push(v.to_string());
                }
                if let Some(v) = doc_obj.get("updated_at").and_then(|v| v.as_str()) {
                    parent_cols.push("updated_at".to_string());
                    parent_vals.push(v.to_string());
                }
            }

            for field in &def.fields {
                if field.has_parent_column() {
                    if field.field_type == core::field::FieldType::Group {
                        // Group fields have prefixed columns: group__sub
                        continue; // handled below
                    }
                    // Try direct key first, then flattened
                    if let Some(val) = doc_obj.get(&field.name) {
                        let str_val = match val {
                            serde_json::Value::String(s) => s.clone(),
                            serde_json::Value::Null => continue,
                            other => other.to_string(),
                        };
                        parent_cols.push(field.name.clone());
                        parent_vals.push(str_val);
                    }
                } else {
                    // Join table fields (array, blocks, has-many relationship)
                    if let Some(val) = doc_obj.get(&field.name) {
                        if !val.is_null() {
                            join_data.insert(field.name.clone(), val.clone());
                        }
                    }
                }

                // Handle group sub-fields (they use parent columns with prefix)
                if field.field_type == core::field::FieldType::Group {
                    for sub in &field.fields {
                        let col_name = format!("{}__{}", field.name, sub.name);
                        // Try nested object first (hydrated export format)
                        let val = doc_obj.get(&field.name)
                            .and_then(|g| g.get(&sub.name))
                            // Then try flattened format
                            .or_else(|| doc_obj.get(&col_name));

                        if let Some(val) = val {
                            let str_val = match val {
                                serde_json::Value::String(s) => s.clone(),
                                serde_json::Value::Null => continue,
                                other => other.to_string(),
                            };
                            parent_cols.push(col_name);
                            parent_vals.push(str_val);
                        }
                    }
                }
            }

            // INSERT OR REPLACE
            let placeholders: Vec<String> = (0..parent_cols.len()).map(|i| format!("?{}", i + 1)).collect();
            let sql = format!(
                "INSERT OR REPLACE INTO \"{}\" ({}) VALUES ({})",
                slug,
                parent_cols.iter().map(|c| format!("\"{}\"", c)).collect::<Vec<_>>().join(", "),
                placeholders.join(", ")
            );

            let params: Vec<Box<dyn rusqlite::types::ToSql>> = parent_vals.iter()
                .map(|v| Box::new(v.clone()) as Box<dyn rusqlite::types::ToSql>)
                .collect();
            let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter()
                .map(|p| p.as_ref())
                .collect();

            tx.execute(&sql, param_refs.as_slice())
                .with_context(|| format!("Failed to insert document {} into '{}'", id, slug))?;

            // Save join table data
            if !join_data.is_empty() {
                db::query::save_join_table_data(&tx, slug, def, id, &join_data)?;
            }

            total_imported += 1;
        }

        tx.commit()
            .with_context(|| format!("Failed to commit import for '{}'", slug))?;

        println!("Imported {} document(s) into '{}'", docs_array.len(), slug);
    }

    println!("\nTotal: {} document(s) imported", total_imported);

    Ok(())
}

// ── other subcommand handlers ─────────────────────────────────────────────

/// Handle the `typegen` subcommand — loads the Lua registry and generates types.
fn typegen_command(config_dir: &Path, lang_str: &str) -> Result<()> {
    let config_dir = config_dir.canonicalize().unwrap_or_else(|_| config_dir.to_path_buf());

    // Load config + Lua VM to get registry
    let cfg = config::CrapConfig::load(&config_dir)
        .context("Failed to load config")?;
    let registry = hooks::init_lua(&config_dir, &cfg)
        .context("Failed to initialize Lua VM")?;
    let reg = registry.read()
        .map_err(|e| anyhow::anyhow!("Registry lock poisoned: {}", e))?;

    if lang_str == "all" {
        for lang in typegen::Language::all() {
            let path = typegen::generate_lang(&config_dir, &reg, *lang)
                .with_context(|| format!("Failed to generate {} types", lang.label()))?;
            println!("{}", path.display());
        }
    } else {
        let lang = typegen::Language::from_str(lang_str)
            .ok_or_else(|| anyhow::anyhow!(
                "Unknown language '{}'. Valid: lua, ts, go, py, rs, all",
                lang_str
            ))?;
        let path = typegen::generate_lang(&config_dir, &reg, lang)
            .context("Failed to generate type definitions")?;
        println!("{}", path.display());
    }

    Ok(())
}

/// Handle the `migrate` subcommand.
fn migrate_command(config_dir: &Path, action: MigrateAction) -> Result<()> {
    let config_dir = config_dir.canonicalize().unwrap_or_else(|_| config_dir.to_path_buf());

    let cfg = config::CrapConfig::load(&config_dir)
        .context("Failed to load config")?;
    let registry = hooks::init_lua(&config_dir, &cfg)
        .context("Failed to initialize Lua VM")?;
    let pool = db::pool::create_pool(&config_dir, &cfg)
        .context("Failed to create database pool")?;

    match action {
        MigrateAction::Up => {
            // Schema sync from Lua definitions
            println!("Syncing schema from Lua definitions...");
            db::migrate::sync_all(&pool, &registry, &cfg.locale)
                .context("Failed to sync database schema")?;
            println!("Schema sync complete.");

            // Run pending Lua data migrations
            let migrations_dir = config_dir.join("migrations");
            let pending = db::migrate::get_pending_migrations(&pool, &migrations_dir)?;

            if pending.is_empty() {
                println!("No pending migrations.");
            } else {
                let hook_runner = hooks::lifecycle::HookRunner::new(&config_dir, registry, &cfg)?;
                for filename in &pending {
                    let path = migrations_dir.join(filename);
                    let mut conn = pool.get().context("Failed to get DB connection")?;
                    let tx = conn.transaction().context("Failed to begin transaction")?;
                    hook_runner.run_migration(&path, "up", &tx)?;
                    db::migrate::record_migration(&tx, filename)?;
                    tx.commit()
                        .with_context(|| format!("Failed to commit migration {}", filename))?;
                    println!("Applied: {}", filename);
                }
                println!("{} migration(s) applied.", pending.len());
            }
        }
        MigrateAction::Down { steps } => {
            let applied = db::migrate::get_applied_migrations_desc(&pool)?;
            let to_rollback: Vec<_> = applied.into_iter().take(steps).collect();

            if to_rollback.is_empty() {
                println!("No migrations to roll back.");
            } else {
                let hook_runner = hooks::lifecycle::HookRunner::new(&config_dir, registry, &cfg)?;
                let migrations_dir = config_dir.join("migrations");
                for filename in &to_rollback {
                    let path = migrations_dir.join(filename);
                    if !path.exists() {
                        anyhow::bail!("Migration file not found: {}", path.display());
                    }
                    let mut conn = pool.get().context("Failed to get DB connection")?;
                    let tx = conn.transaction().context("Failed to begin transaction")?;
                    hook_runner.run_migration(&path, "down", &tx)?;
                    db::migrate::remove_migration(&tx, filename)?;
                    tx.commit()
                        .with_context(|| format!("Failed to commit rollback of {}", filename))?;
                    println!("Rolled back: {}", filename);
                }
                println!("{} migration(s) rolled back.", to_rollback.len());
            }
        }
        MigrateAction::List => {
            let migrations_dir = config_dir.join("migrations");
            let all_files = db::migrate::list_migration_files(&migrations_dir)?;
            let applied = db::migrate::get_applied_migrations(&pool)?;

            if all_files.is_empty() {
                println!("No migration files found in {}", migrations_dir.display());
            } else {
                println!("{:<50} Status", "Migration");
                println!("{}", "-".repeat(60));
                for f in &all_files {
                    let status = if applied.contains(f) { "applied" } else { "pending" };
                    println!("{:<50} {}", f, status);
                }
            }
        }
        MigrateAction::Fresh { confirm } => {
            if !confirm {
                anyhow::bail!(
                    "migrate fresh is destructive — it drops ALL tables and recreates them.\n\
                     Pass --confirm to proceed."
                );
            }

            println!("Dropping all tables...");
            db::migrate::drop_all_tables(&pool)?;
            println!("Tables dropped.");

            println!("Recreating schema from Lua definitions...");
            db::migrate::sync_all(&pool, &registry, &cfg.locale)
                .context("Failed to sync database schema")?;
            println!("Schema sync complete.");

            // Run all migrations from scratch
            let migrations_dir = config_dir.join("migrations");
            let all_files = db::migrate::list_migration_files(&migrations_dir)?;
            if !all_files.is_empty() {
                let hook_runner = hooks::lifecycle::HookRunner::new(&config_dir, registry, &cfg)?;
                for filename in &all_files {
                    let path = migrations_dir.join(filename);
                    let mut conn = pool.get().context("Failed to get DB connection")?;
                    let tx = conn.transaction().context("Failed to begin transaction")?;
                    hook_runner.run_migration(&path, "up", &tx)?;
                    db::migrate::record_migration(&tx, filename)?;
                    tx.commit()
                        .with_context(|| format!("Failed to commit migration {}", filename))?;
                    println!("Applied: {}", filename);
                }
                println!("{} migration(s) applied.", all_files.len());
            }

            println!("Fresh migration complete.");
        }
    }

    Ok(())
}

/// Handle the `backup` subcommand.
fn backup_command(
    config_dir: &Path,
    output: Option<PathBuf>,
    include_uploads: bool,
) -> Result<()> {
    let config_dir = config_dir.canonicalize().unwrap_or_else(|_| config_dir.to_path_buf());

    let cfg = config::CrapConfig::load(&config_dir)
        .context("Failed to load config")?;

    let db_path = cfg.db_path(&config_dir);
    if !db_path.exists() {
        anyhow::bail!("Database file not found: {}", db_path.display());
    }

    // Determine backup directory
    let timestamp = chrono::Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    let backup_dir_name = format!("backup-{}", timestamp);
    let backup_base = output.unwrap_or_else(|| config_dir.join("backups"));
    let backup_dir = backup_base.join(&backup_dir_name);

    std::fs::create_dir_all(&backup_dir)
        .with_context(|| format!("Failed to create backup directory: {}", backup_dir.display()))?;

    // VACUUM INTO for a consistent snapshot
    let backup_db_path = backup_dir.join("crap.db");
    println!("Creating database snapshot...");
    {
        let conn = rusqlite::Connection::open(&db_path)
            .context("Failed to open database for backup")?;
        conn.execute("VACUUM INTO ?1", [backup_db_path.to_string_lossy().as_ref()])
            .context("VACUUM INTO failed")?;
    }
    let db_size = std::fs::metadata(&backup_db_path)
        .map(|m| m.len())
        .unwrap_or(0);
    println!("Database snapshot: {} ({} bytes)", backup_db_path.display(), db_size);

    // Optionally backup uploads
    let mut uploads_size: Option<u64> = None;
    if include_uploads {
        let uploads_dir = config_dir.join("uploads");
        if uploads_dir.exists() && uploads_dir.is_dir() {
            let archive_path = backup_dir.join("uploads.tar.gz");
            println!("Compressing uploads...");
            let status = std::process::Command::new("tar")
                .args(["czf", &archive_path.to_string_lossy(), "-C", &config_dir.to_string_lossy(), "uploads"])
                .status();
            match status {
                Ok(s) if s.success() => {
                    uploads_size = std::fs::metadata(&archive_path).map(|m| m.len()).ok();
                    println!("Uploads archive: {} ({} bytes)",
                        archive_path.display(),
                        uploads_size.unwrap_or(0));
                }
                Ok(s) => {
                    eprintln!("Warning: tar exited with status {}", s);
                }
                Err(e) => {
                    eprintln!("Warning: tar not found or failed: {}. Skipping uploads backup.", e);
                }
            }
        } else {
            println!("No uploads directory found — skipping.");
        }
    }

    // Write manifest.json
    let manifest = serde_json::json!({
        "timestamp": chrono::Local::now().to_rfc3339(),
        "db_size": db_size,
        "uploads_size": uploads_size,
        "include_uploads": include_uploads,
        "source_db": db_path.to_string_lossy(),
        "source_config": config_dir.to_string_lossy(),
    });
    let manifest_path = backup_dir.join("manifest.json");
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)
        .context("Failed to write manifest.json")?;

    println!("\nBackup complete: {}", backup_dir.display());
    Ok(())
}
