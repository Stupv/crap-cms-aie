//! Build field context objects for template rendering (no DB access).

use std::collections::HashMap;

use serde_json::Value;

use crate::core::FieldDefinition;

mod field_type_extras;
mod single;

pub use field_type_extras::{FieldRecursionCtx, apply_field_type_extras};
pub use single::build_single_field_context;

/// Build field context objects for template rendering.
///
/// `non_default_locale`: when true, non-localized fields are rendered readonly
/// (locked) because they are shared across all locales and should only be edited
/// from the default locale.
pub fn build_field_contexts(
    fields: &[FieldDefinition],
    values: &HashMap<String, String>,
    errors: &HashMap<String, String>,
    filter_hidden: bool,
    non_default_locale: bool,
) -> Vec<Value> {
    let iter: Box<dyn Iterator<Item = &FieldDefinition>> = if filter_hidden {
        Box::new(fields.iter().filter(|field| !field.admin.hidden))
    } else {
        Box::new(fields.iter())
    };
    iter.map(|field| build_single_field_context(field, values, errors, "", non_default_locale, 0))
        .collect()
}

#[cfg(test)]
mod tests;
