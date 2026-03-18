//! CLI output primitives: colors, themed prompts, tables, spinners.

pub mod output;
pub mod spinner;
pub mod table;
pub mod theme;

pub use output::{dim, error, header, hint, info, kv, kv_status, step, success, warning};
pub use spinner::Spinner;
pub use table::Table;
pub use theme::crap_theme;
