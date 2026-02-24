//! Type generation for multiple languages from the collection registry.
//!
//! - `lua` — LuaLS annotations for hook/init IDE support (internal)
//! - `typescript` — TypeScript interfaces for gRPC clients
//! - `go` — Go structs with json tags
//! - `python` — Python dataclasses
//! - `rust` — Rust structs with serde derives

mod lua;
mod typescript;
mod go;
mod python;
mod rust_types;

use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::core::field::{FieldDefinition, FieldType};
use crate::core::Registry;

/// Embedded Lua API type definitions — kept in sync with the CMS binary version.
const LUA_API_TYPES: &str = include_str!("../../types/crap.lua");

/// Supported output languages for type generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Lua,
    Typescript,
    Go,
    Python,
    Rust,
}

impl Language {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "lua" => Some(Self::Lua),
            "ts" | "typescript" => Some(Self::Typescript),
            "go" | "golang" => Some(Self::Go),
            "py" | "python" => Some(Self::Python),
            "rs" | "rust" => Some(Self::Rust),
            _ => None,
        }
    }

    pub fn file_extension(&self) -> &'static str {
        match self {
            Self::Lua => "lua",
            Self::Typescript => "ts",
            Self::Go => "go",
            Self::Python => "py",
            Self::Rust => "rs",
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::Lua, Self::Typescript, Self::Go, Self::Python, Self::Rust]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Lua => "lua",
            Self::Typescript => "ts",
            Self::Go => "go",
            Self::Python => "py",
            Self::Rust => "rs",
        }
    }
}

/// Generate Lua type definitions (default behavior, used on server startup).
/// Writes to `<config_dir>/types/generated.lua`.
pub fn generate(config_dir: &Path, registry: &Registry) -> Result<PathBuf> {
    generate_lang(config_dir, registry, Language::Lua)
}

/// Generate type definitions for a specific language.
/// Writes to `<config_dir>/types/generated.<ext>`.
/// Also writes `crap.lua` API surface types (keeps them in sync with CMS binary version).
pub fn generate_lang(config_dir: &Path, registry: &Registry, lang: Language) -> Result<PathBuf> {
    let types_dir = config_dir.join("types");
    std::fs::create_dir_all(&types_dir)?;

    // Always write the API surface types (keeps them in sync with CMS version)
    std::fs::write(types_dir.join("crap.lua"), LUA_API_TYPES)?;

    let output = render(registry, lang);
    let filename = format!("generated.{}", lang.file_extension());
    let path = types_dir.join(filename);
    std::fs::write(&path, output)?;
    Ok(path)
}

/// Render type definitions for the given language.
fn render(registry: &Registry, lang: Language) -> String {
    match lang {
        Language::Lua => lua::render(registry),
        Language::Typescript => typescript::render(registry),
        Language::Go => go::render(registry),
        Language::Python => python::render(registry),
        Language::Rust => rust_types::render(registry),
    }
}

// ---------------------------------------------------------------------------
// Shared helpers used by multiple generators
// ---------------------------------------------------------------------------

/// Convert a slug like "site_settings" to PascalCase "SiteSettings".
pub(crate) fn to_pascal_case(slug: &str) -> String {
    slug.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => {
                    let mut s = c.to_uppercase().to_string();
                    s.push_str(&chars.collect::<String>());
                    s
                }
                None => String::new(),
            }
        })
        .collect()
}

/// Whether a field should be treated as optional in generated types.
pub(crate) fn is_optional(field: &FieldDefinition) -> bool {
    !field.required || field.field_type == FieldType::Checkbox
}

/// Get sorted collection slugs from the registry.
pub(crate) fn sorted_collection_slugs(registry: &Registry) -> Vec<&String> {
    let mut slugs: Vec<&String> = registry.collections.keys().collect();
    slugs.sort();
    slugs
}

/// Get sorted global slugs from the registry.
pub(crate) fn sorted_global_slugs(registry: &Registry) -> Vec<&String> {
    let mut slugs: Vec<&String> = registry.globals.keys().collect();
    slugs.sort();
    slugs
}
