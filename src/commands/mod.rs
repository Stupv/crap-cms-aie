//! CLI command handlers. Each submodule handles one top-level subcommand.

pub mod db;
pub mod export;
pub mod images;
pub mod init;
pub mod jobs;
pub mod make;
pub mod mcp;
pub mod serve;
pub mod status;
pub mod templates;
pub mod typegen;
pub mod user;

mod cli_types;
mod helpers;

pub use cli_types::{
    BlueprintAction, DbAction, ImagesAction, JobsAction, MakeAction, MigrateAction,
    TemplatesAction, UserAction, parse_key_val,
};
pub use helpers::load_config_and_sync;
