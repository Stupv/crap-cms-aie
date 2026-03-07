//! Build field context objects for template rendering (no DB access).

use std::collections::HashMap;
use crate::core::field::FieldType;
use super::{safe_template_id, count_errors_in_fields, MAX_FIELD_DEPTH};
use super::super::shared::auto_label_from_name;

/// Build a field context for a single field definition, recursing into composite sub-fields.
///
/// `name_prefix`: the full form-name prefix for this field (e.g. `"content[0]"` for a
/// field inside a blocks row at index 0). Top-level fields use an empty prefix.
/// `depth`: current nesting depth (0 = top-level). Stops recursing at MAX_FIELD_DEPTH.
pub fn build_single_field_context(
    field: &crate::core::field::FieldDefinition,
    values: &HashMap<String, String>,
    errors: &HashMap<String, String>,
    name_prefix: &str,
    non_default_locale: bool,
    depth: usize,
) -> serde_json::Value {
    let full_name = if name_prefix.is_empty() {
        field.name.clone()
    } else if matches!(field.field_type, FieldType::Tabs | FieldType::Row | FieldType::Collapsible) {
        name_prefix.to_string() // transparent — layout wrappers don't add their name
    } else {
        format!("{}[{}]", name_prefix, field.name)
    };
    let value = values.get(&full_name).cloned().unwrap_or_default();
    let label = field.admin.label.as_ref()
        .map(|ls| ls.resolve_default().to_string())
        .unwrap_or_else(|| auto_label_from_name(&field.name));
    let locale_locked = non_default_locale && !field.localized;

    let mut ctx = serde_json::json!({
        "name": full_name,
        "field_type": field.field_type.as_str(),
        "label": label,
        "required": field.required,
        "value": value,
        "placeholder": field.admin.placeholder.as_ref().map(|ls| ls.resolve_default()),
        "description": field.admin.description.as_ref().map(|ls| ls.resolve_default()),
        "readonly": field.admin.readonly || locale_locked,
        "localized": field.localized,
        "locale_locked": locale_locked,
    });

    if let Some(ref pos) = field.admin.position {
        ctx["position"] = serde_json::json!(pos);
    }

    if let Some(err) = errors.get(&full_name) {
        ctx["error"] = serde_json::json!(err);
    }

    // Validation property context: min_length, max_length, min, max, step, rows
    if let Some(ml) = field.min_length {
        ctx["min_length"] = serde_json::json!(ml);
    }
    if let Some(ml) = field.max_length {
        ctx["max_length"] = serde_json::json!(ml);
    }
    if let Some(v) = field.min {
        ctx["min"] = serde_json::json!(v);
        ctx["has_min"] = serde_json::json!(true);
    }
    if let Some(v) = field.max {
        ctx["max"] = serde_json::json!(v);
        ctx["has_max"] = serde_json::json!(true);
    }
    // Number step: use admin.step or default "any"
    if field.field_type == FieldType::Number {
        let step = field.admin.step.as_deref().unwrap_or("any");
        ctx["step"] = serde_json::json!(step);
    }
    // Textarea rows: use admin.rows or default 8
    if field.field_type == FieldType::Textarea {
        let rows = field.admin.rows.unwrap_or(8);
        ctx["rows"] = serde_json::json!(rows);
    }
    // Date bounds: min_date / max_date
    if field.field_type == FieldType::Date {
        if let Some(ref md) = field.min_date {
            ctx["min_date"] = serde_json::json!(md);
        }
        if let Some(ref md) = field.max_date {
            ctx["max_date"] = serde_json::json!(md);
        }
    }
    // Code language: use admin.language or default "json"
    if field.field_type == FieldType::Code {
        let lang = field.admin.language.as_deref().unwrap_or("json");
        ctx["language"] = serde_json::json!(lang);
    }

    // Beyond max depth, render as a simple text input
    if depth >= MAX_FIELD_DEPTH {
        return ctx;
    }

    match &field.field_type {
        FieldType::Select | FieldType::Radio => {
            if field.has_many {
                // Multi-select: value is a JSON array like ["val1","val2"]
                let selected_values: std::collections::HashSet<String> =
                    serde_json::from_str(&value)
                        .unwrap_or_default();
                let options: Vec<_> = field.options.iter().map(|opt| {
                    serde_json::json!({
                        "label": opt.label.resolve_default(),
                        "value": opt.value,
                        "selected": selected_values.contains(&opt.value),
                    })
                }).collect();
                ctx["options"] = serde_json::json!(options);
                ctx["has_many"] = serde_json::json!(true);
            } else {
                let options: Vec<_> = field.options.iter().map(|opt| {
                    serde_json::json!({
                        "label": opt.label.resolve_default(),
                        "value": opt.value,
                        "selected": opt.value == value,
                    })
                }).collect();
                ctx["options"] = serde_json::json!(options);
            }
        }
        FieldType::Checkbox => {
            let checked = matches!(value.as_str(), "1" | "true" | "on" | "yes");
            ctx["checked"] = serde_json::json!(checked);
        }
        FieldType::Relationship => {
            if let Some(ref rc) = field.relationship {
                ctx["relationship_collection"] = serde_json::json!(rc.collection);
                ctx["has_many"] = serde_json::json!(rc.has_many);
                if rc.is_polymorphic() {
                    ctx["polymorphic"] = serde_json::json!(true);
                    ctx["collections"] = serde_json::json!(rc.polymorphic);
                }
            }
            if let Some(ref p) = field.admin.picker {
                ctx["picker"] = serde_json::json!(p);
            }
        }
        FieldType::Array => {
            // Build sub_field contexts for the <template> section (with __INDEX__ placeholder)
            let template_prefix = format!("{}[__INDEX__]", full_name);
            let sub_fields: Vec<_> = field.fields.iter().map(|sf| {
                build_single_field_context(sf, &HashMap::new(), &HashMap::new(), &template_prefix, non_default_locale, depth + 1)
            }).collect();
            ctx["sub_fields"] = serde_json::json!(sub_fields);
            ctx["row_count"] = serde_json::json!(0);
            ctx["template_id"] = serde_json::json!(safe_template_id(&full_name));
            if let Some(ref lf) = field.admin.label_field {
                ctx["label_field"] = serde_json::json!(lf);
            }
            if let Some(max) = field.max_rows {
                ctx["max_rows"] = serde_json::json!(max);
            }
            if let Some(min) = field.min_rows {
                ctx["min_rows"] = serde_json::json!(min);
            }
            ctx["init_collapsed"] = serde_json::json!(field.admin.collapsed);
            if let Some(ref ls) = field.admin.labels_singular {
                ctx["add_label"] = serde_json::json!(ls.resolve_default());
            }
        }
        FieldType::Group => {
            // Group sub-fields use double-underscore naming at top level,
            // but when nested inside Array/Blocks they use bracketed names.
            let sub_fields: Vec<_> = if name_prefix.is_empty() {
                // Top-level group: use col_name pattern (group__subfield)
                field.fields.iter().map(|sf| {
                    let col_name = format!("{}__{}", field.name, sf.name);
                    let sub_value = values.get(&col_name).cloned().unwrap_or_default();
                    let sub_label = sf.admin.label.as_ref()
                        .map(|ls| ls.resolve_default().to_string())
                        .unwrap_or_else(|| auto_label_from_name(&sf.name));
                    let sf_locale_locked = non_default_locale && !field.localized;
                    let mut sub_ctx = serde_json::json!({
                        "name": col_name,
                        "field_type": sf.field_type.as_str(),
                        "label": sub_label,
                        "required": sf.required,
                        "value": sub_value,
                        "placeholder": sf.admin.placeholder.as_ref().map(|ls| ls.resolve_default()),
                        "description": sf.admin.description.as_ref().map(|ls| ls.resolve_default()),
                        "readonly": sf.admin.readonly || sf_locale_locked,
                        "localized": field.localized,
                        "locale_locked": sf_locale_locked,
                    });
                    // Recurse for nested composites
                    apply_field_type_extras(sf, &sub_value, &mut sub_ctx, values, errors, &col_name, non_default_locale, depth + 1);
                    sub_ctx
                }).collect()
            } else {
                // Nested group: use bracketed naming via recursion
                field.fields.iter().map(|sf| {
                    build_single_field_context(sf, values, errors, &full_name, non_default_locale, depth + 1)
                }).collect()
            };
            ctx["sub_fields"] = serde_json::json!(sub_fields);
            ctx["collapsed"] = serde_json::json!(field.admin.collapsed);
        }
        FieldType::Row => {
            // Row is a layout-only container; sub-fields are promoted to top level.
            // Top-level row promotes sub-fields to the same level as the parent,
            // so we delegate to build_single_field_context with the same prefix.
            // This correctly handles Group (double-underscore), Collapsible, etc.
            let sub_fields: Vec<_> = if name_prefix.is_empty() {
                field.fields.iter().map(|sf| {
                    build_single_field_context(sf, values, errors, "", non_default_locale, depth + 1)
                }).collect()
            } else {
                // Nested row: use bracketed naming via recursion
                field.fields.iter().map(|sf| {
                    build_single_field_context(sf, values, errors, &full_name, non_default_locale, depth + 1)
                }).collect()
            };
            ctx["sub_fields"] = serde_json::json!(sub_fields);
        }
        FieldType::Collapsible => {
            // Collapsible is a layout-only container like Row but with a toggle header.
            // Top-level collapsible promotes sub-fields to the same level as the parent,
            // so we delegate to build_single_field_context with the same prefix.
            // This correctly handles Group (double-underscore), Row, etc.
            let sub_fields: Vec<_> = if name_prefix.is_empty() {
                field.fields.iter().map(|sf| {
                    build_single_field_context(sf, values, errors, "", non_default_locale, depth + 1)
                }).collect()
            } else {
                field.fields.iter().map(|sf| {
                    build_single_field_context(sf, values, errors, &full_name, non_default_locale, depth + 1)
                }).collect()
            };
            ctx["sub_fields"] = serde_json::json!(sub_fields);
            ctx["collapsed"] = serde_json::json!(field.admin.collapsed);
        }
        FieldType::Tabs => {
            // Tabs is a layout-only container with multiple tab panels.
            // Top-level tabs promote sub-fields to the same level as the parent,
            // so we delegate to build_single_field_context with the same prefix.
            // This correctly handles Group (double-underscore), Row, Collapsible, etc.
            let tabs_ctx: Vec<_> = field.tabs.iter().map(|tab| {
                let tab_sub_fields: Vec<_> = if name_prefix.is_empty() {
                    tab.fields.iter().map(|sf| {
                        build_single_field_context(sf, values, errors, "", non_default_locale, depth + 1)
                    }).collect()
                } else {
                    tab.fields.iter().map(|sf| {
                        build_single_field_context(sf, values, errors, &full_name, non_default_locale, depth + 1)
                    }).collect()
                };
                let error_count = count_errors_in_fields(&tab_sub_fields);
                let mut tab_ctx = serde_json::json!({
                    "label": &tab.label,
                    "sub_fields": tab_sub_fields,
                });
                if error_count > 0 {
                    tab_ctx["error_count"] = serde_json::json!(error_count);
                }
                if let Some(ref desc) = tab.description {
                    tab_ctx["description"] = serde_json::json!(desc);
                }
                tab_ctx
            }).collect();
            ctx["tabs"] = serde_json::json!(tabs_ctx);
        }
        FieldType::Date => {
            let appearance = field.picker_appearance.as_deref().unwrap_or("dayOnly");
            ctx["picker_appearance"] = serde_json::json!(appearance);
            match appearance {
                "dayOnly" => {
                    let date_val = if value.len() >= 10 { &value[..10] } else { &value };
                    ctx["date_only_value"] = serde_json::json!(date_val);
                }
                "dayAndTime" => {
                    let dt_val = if value.len() >= 16 { &value[..16] } else { &value };
                    ctx["datetime_local_value"] = serde_json::json!(dt_val);
                }
                _ => {}
            }
        }
        FieldType::Upload => {
            if let Some(ref rc) = field.relationship {
                ctx["relationship_collection"] = serde_json::json!(rc.collection);
                if rc.has_many {
                    ctx["has_many"] = serde_json::json!(true);
                }
            }
            let picker = field.admin.picker.as_deref().unwrap_or("drawer");
            if picker != "none" {
                ctx["picker"] = serde_json::json!(picker);
            }
        }
        FieldType::Text | FieldType::Number if field.has_many => {
            // Tag-style input: value is a JSON array like ["tag1","tag2"]
            let tags: Vec<String> = serde_json::from_str(&value).unwrap_or_default();
            ctx["has_many"] = serde_json::json!(true);
            ctx["tags"] = serde_json::json!(tags);
            // Store comma-separated for the hidden input
            ctx["value"] = serde_json::json!(tags.join(","));
        }
        FieldType::Richtext => {
            if !field.admin.features.is_empty() {
                ctx["features"] = serde_json::json!(field.admin.features);
            }
            let fmt = field.admin.richtext_format.as_deref().unwrap_or("html");
            ctx["richtext_format"] = serde_json::json!(fmt);
            // Store node names — full defs resolved in enrich_field_contexts
            if !field.admin.nodes.is_empty() {
                ctx["_node_names"] = serde_json::json!(field.admin.nodes);
            }
        }
        FieldType::Blocks => {
            let block_defs: Vec<_> = field.blocks.iter().map(|bd| {
                // Build sub-field contexts for each block type's <template> section
                let template_prefix = format!("{}[__INDEX__]", full_name);
                let block_fields: Vec<_> = bd.fields.iter().map(|sf| {
                    build_single_field_context(sf, &HashMap::new(), &HashMap::new(), &template_prefix, non_default_locale, depth + 1)
                }).collect();
                let mut def = serde_json::json!({
                    "block_type": bd.block_type,
                    "label": bd.label.as_ref().map(|ls| ls.resolve_default()).unwrap_or(&bd.block_type),
                    "fields": block_fields,
                });
                if let Some(ref lf) = bd.label_field {
                    def["label_field"] = serde_json::json!(lf);
                }
                if let Some(ref g) = bd.group {
                    def["group"] = serde_json::json!(g);
                }
                if let Some(ref url) = bd.image_url {
                    def["image_url"] = serde_json::json!(url);
                }
                def
            }).collect();
            ctx["block_definitions"] = serde_json::json!(block_defs);
            ctx["row_count"] = serde_json::json!(0);
            ctx["template_id"] = serde_json::json!(safe_template_id(&full_name));
            if let Some(ref lf) = field.admin.label_field {
                ctx["label_field"] = serde_json::json!(lf);
            }
            if let Some(max) = field.max_rows {
                ctx["max_rows"] = serde_json::json!(max);
            }
            if let Some(min) = field.min_rows {
                ctx["min_rows"] = serde_json::json!(min);
            }
            ctx["init_collapsed"] = serde_json::json!(field.admin.collapsed);
            if let Some(ref ls) = field.admin.labels_singular {
                ctx["add_label"] = serde_json::json!(ls.resolve_default());
            }
            if let Some(ref p) = field.admin.picker {
                ctx["picker"] = serde_json::json!(p);
            }
        }
        FieldType::Join => {
            if let Some(ref jc) = field.join {
                ctx["join_collection"] = serde_json::json!(jc.collection);
                ctx["join_on"] = serde_json::json!(jc.on);
            }
            ctx["readonly"] = serde_json::json!(true);
        }
        _ => {}
    }

    ctx
}

/// Apply type-specific extras to an already-built sub_ctx (for top-level group sub-fields
/// that use the `col_name` pattern but still need composite-type recursion).
pub fn apply_field_type_extras(
    sf: &crate::core::field::FieldDefinition,
    value: &str,
    sub_ctx: &mut serde_json::Value,
    values: &HashMap<String, String>,
    errors: &HashMap<String, String>,
    name_prefix: &str,
    non_default_locale: bool,
    depth: usize,
) {
    // Validation property context for sub-fields
    if let Some(ml) = sf.min_length {
        sub_ctx["min_length"] = serde_json::json!(ml);
    }
    if let Some(ml) = sf.max_length {
        sub_ctx["max_length"] = serde_json::json!(ml);
    }
    if let Some(v) = sf.min {
        sub_ctx["min"] = serde_json::json!(v);
        sub_ctx["has_min"] = serde_json::json!(true);
    }
    if let Some(v) = sf.max {
        sub_ctx["max"] = serde_json::json!(v);
        sub_ctx["has_max"] = serde_json::json!(true);
    }
    if sf.field_type == FieldType::Number {
        let step = sf.admin.step.as_deref().unwrap_or("any");
        sub_ctx["step"] = serde_json::json!(step);
    }
    if sf.field_type == FieldType::Textarea {
        let rows = sf.admin.rows.unwrap_or(8);
        sub_ctx["rows"] = serde_json::json!(rows);
    }
    if sf.field_type == FieldType::Date {
        if let Some(ref md) = sf.min_date {
            sub_ctx["min_date"] = serde_json::json!(md);
        }
        if let Some(ref md) = sf.max_date {
            sub_ctx["max_date"] = serde_json::json!(md);
        }
    }

    if depth >= MAX_FIELD_DEPTH { return; }
    match &sf.field_type {
        FieldType::Checkbox => {
            let checked = matches!(value, "1" | "true" | "on" | "yes");
            sub_ctx["checked"] = serde_json::json!(checked);
        }
        FieldType::Select | FieldType::Radio => {
            if sf.has_many {
                let selected_values: std::collections::HashSet<String> =
                    serde_json::from_str(value)
                        .unwrap_or_default();
                let options: Vec<_> = sf.options.iter().map(|opt| {
                    serde_json::json!({
                        "label": opt.label.resolve_default(),
                        "value": opt.value,
                        "selected": selected_values.contains(&opt.value),
                    })
                }).collect();
                sub_ctx["options"] = serde_json::json!(options);
                sub_ctx["has_many"] = serde_json::json!(true);
            } else {
                let options: Vec<_> = sf.options.iter().map(|opt| {
                    serde_json::json!({
                        "label": opt.label.resolve_default(),
                        "value": opt.value,
                        "selected": opt.value == value,
                    })
                }).collect();
                sub_ctx["options"] = serde_json::json!(options);
            }
        }
        FieldType::Date => {
            let appearance = sf.picker_appearance.as_deref().unwrap_or("dayOnly");
            sub_ctx["picker_appearance"] = serde_json::json!(appearance);
            match appearance {
                "dayOnly" => {
                    let date_val = if value.len() >= 10 { &value[..10] } else { value };
                    sub_ctx["date_only_value"] = serde_json::json!(date_val);
                }
                "dayAndTime" => {
                    let dt_val = if value.len() >= 16 { &value[..16] } else { value };
                    sub_ctx["datetime_local_value"] = serde_json::json!(dt_val);
                }
                _ => {}
            }
        }
        FieldType::Array => {
            let template_prefix = format!("{}[__INDEX__]", name_prefix);
            let sub_fields: Vec<_> = sf.fields.iter().map(|nested| {
                build_single_field_context(nested, &HashMap::new(), &HashMap::new(), &template_prefix, non_default_locale, depth + 1)
            }).collect();
            sub_ctx["sub_fields"] = serde_json::json!(sub_fields);
            sub_ctx["row_count"] = serde_json::json!(0);
            sub_ctx["template_id"] = serde_json::json!(safe_template_id(name_prefix));
            if let Some(ref lf) = sf.admin.label_field {
                sub_ctx["label_field"] = serde_json::json!(lf);
            }
            if let Some(max) = sf.max_rows {
                sub_ctx["max_rows"] = serde_json::json!(max);
            }
            if let Some(min) = sf.min_rows {
                sub_ctx["min_rows"] = serde_json::json!(min);
            }
            sub_ctx["init_collapsed"] = serde_json::json!(sf.admin.collapsed);
            if let Some(ref ls) = sf.admin.labels_singular {
                sub_ctx["add_label"] = serde_json::json!(ls.resolve_default());
            }
        }
        FieldType::Group => {
            let sub_fields: Vec<_> = sf.fields.iter().map(|nested| {
                build_single_field_context(nested, values, errors, name_prefix, non_default_locale, depth + 1)
            }).collect();
            sub_ctx["sub_fields"] = serde_json::json!(sub_fields);
            sub_ctx["collapsed"] = serde_json::json!(sf.admin.collapsed);
        }
        FieldType::Row => {
            let sub_fields: Vec<_> = sf.fields.iter().map(|nested| {
                build_single_field_context(nested, values, errors, name_prefix, non_default_locale, depth + 1)
            }).collect();
            sub_ctx["sub_fields"] = serde_json::json!(sub_fields);
        }
        FieldType::Collapsible => {
            let sub_fields: Vec<_> = sf.fields.iter().map(|nested| {
                build_single_field_context(nested, values, errors, name_prefix, non_default_locale, depth + 1)
            }).collect();
            sub_ctx["sub_fields"] = serde_json::json!(sub_fields);
            sub_ctx["collapsed"] = serde_json::json!(sf.admin.collapsed);
        }
        FieldType::Tabs => {
            let tabs_ctx: Vec<_> = sf.tabs.iter().map(|tab| {
                let tab_sub_fields: Vec<_> = tab.fields.iter().map(|nested| {
                    build_single_field_context(nested, values, errors, name_prefix, non_default_locale, depth + 1)
                }).collect();
                let error_count = count_errors_in_fields(&tab_sub_fields);
                let mut tab_ctx = serde_json::json!({
                    "label": &tab.label,
                    "sub_fields": tab_sub_fields,
                });
                if error_count > 0 {
                    tab_ctx["error_count"] = serde_json::json!(error_count);
                }
                if let Some(ref desc) = tab.description {
                    tab_ctx["description"] = serde_json::json!(desc);
                }
                tab_ctx
            }).collect();
            sub_ctx["tabs"] = serde_json::json!(tabs_ctx);
        }
        FieldType::Blocks => {
            let block_defs: Vec<_> = sf.blocks.iter().map(|bd| {
                let template_prefix = format!("{}[__INDEX__]", name_prefix);
                let block_fields: Vec<_> = bd.fields.iter().map(|nested| {
                    build_single_field_context(nested, &HashMap::new(), &HashMap::new(), &template_prefix, non_default_locale, depth + 1)
                }).collect();
                let mut def = serde_json::json!({
                    "block_type": bd.block_type,
                    "label": bd.label.as_ref().map(|ls| ls.resolve_default()).unwrap_or(&bd.block_type),
                    "fields": block_fields,
                });
                if let Some(ref lf) = bd.label_field {
                    def["label_field"] = serde_json::json!(lf);
                }
                if let Some(ref g) = bd.group {
                    def["group"] = serde_json::json!(g);
                }
                if let Some(ref url) = bd.image_url {
                    def["image_url"] = serde_json::json!(url);
                }
                def
            }).collect();
            sub_ctx["block_definitions"] = serde_json::json!(block_defs);
            sub_ctx["row_count"] = serde_json::json!(0);
            sub_ctx["template_id"] = serde_json::json!(safe_template_id(name_prefix));
            if let Some(max) = sf.max_rows {
                sub_ctx["max_rows"] = serde_json::json!(max);
            }
            if let Some(min) = sf.min_rows {
                sub_ctx["min_rows"] = serde_json::json!(min);
            }
            sub_ctx["init_collapsed"] = serde_json::json!(sf.admin.collapsed);
            if let Some(ref ls) = sf.admin.labels_singular {
                sub_ctx["add_label"] = serde_json::json!(ls.resolve_default());
            }
            if let Some(ref p) = sf.admin.picker {
                sub_ctx["picker"] = serde_json::json!(p);
            }
        }
        FieldType::Relationship => {
            if let Some(ref rc) = sf.relationship {
                sub_ctx["relationship_collection"] = serde_json::json!(rc.collection);
                sub_ctx["has_many"] = serde_json::json!(rc.has_many);
                if rc.is_polymorphic() {
                    sub_ctx["polymorphic"] = serde_json::json!(true);
                    sub_ctx["collections"] = serde_json::json!(rc.polymorphic);
                }
            }
            if let Some(ref p) = sf.admin.picker {
                sub_ctx["picker"] = serde_json::json!(p);
            }
        }
        FieldType::Upload => {
            if let Some(ref rc) = sf.relationship {
                sub_ctx["relationship_collection"] = serde_json::json!(rc.collection);
                if rc.has_many {
                    sub_ctx["has_many"] = serde_json::json!(true);
                }
            }
            let picker = sf.admin.picker.as_deref().unwrap_or("drawer");
            if picker != "none" {
                sub_ctx["picker"] = serde_json::json!(picker);
            }
        }
        FieldType::Code => {
            let lang = sf.admin.language.as_deref().unwrap_or("json");
            sub_ctx["language"] = serde_json::json!(lang);
        }
        FieldType::Text | FieldType::Number if sf.has_many => {
            let tags: Vec<String> = serde_json::from_str(value).unwrap_or_default();
            sub_ctx["has_many"] = serde_json::json!(true);
            sub_ctx["tags"] = serde_json::json!(tags);
            sub_ctx["value"] = serde_json::json!(tags.join(","));
        }
        _ => {}
    }
}

/// Build field context objects for template rendering.
///
/// `non_default_locale`: when true, non-localized fields are rendered readonly
/// (locked) because they are shared across all locales and should only be edited
/// from the default locale.
pub fn build_field_contexts(
    fields: &[crate::core::field::FieldDefinition],
    values: &HashMap<String, String>,
    errors: &HashMap<String, String>,
    filter_hidden: bool,
    non_default_locale: bool,
) -> Vec<serde_json::Value> {
    let iter: Box<dyn Iterator<Item = &crate::core::field::FieldDefinition>> = if filter_hidden {
        Box::new(fields.iter().filter(|field| !field.admin.hidden))
    } else {
        Box::new(fields.iter())
    };
    iter.map(|field| {
        build_single_field_context(field, values, errors, "", non_default_locale, 0)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::core::field::{FieldDefinition, SelectOption, LocalizedString, BlockDefinition};

    fn make_field(name: &str, ft: FieldType) -> FieldDefinition {
        FieldDefinition {
            name: name.to_string(),
            field_type: ft,
            ..Default::default()
        }
    }

    // --- build_field_contexts: array/block sub-field enrichment tests ---

    #[test]
    fn build_field_contexts_array_sub_fields_include_type_and_label() {
        let mut arr_field = make_field("items", FieldType::Array);
        arr_field.fields = vec![
            make_field("title", FieldType::Text),
            make_field("body", FieldType::Richtext),
        ];
        let fields = vec![arr_field];
        let values = HashMap::new();
        let errors = HashMap::new();
        let result = build_field_contexts(&fields, &values, &errors, false, false);
        assert_eq!(result.len(), 1);
        let sub_fields = result[0]["sub_fields"].as_array().unwrap();
        assert_eq!(sub_fields.len(), 2);
        assert_eq!(sub_fields[0]["field_type"], "text");
        assert_eq!(sub_fields[0]["label"], "Title");
        assert_eq!(sub_fields[1]["field_type"], "richtext");
        assert_eq!(sub_fields[1]["label"], "Body");
    }

    #[test]
    fn build_field_contexts_array_select_sub_field_includes_options() {
        let mut select_sf = make_field("status", FieldType::Select);
        select_sf.options = vec![
            SelectOption { label: LocalizedString::Plain("Draft".to_string()), value: "draft".to_string() },
            SelectOption { label: LocalizedString::Plain("Published".to_string()), value: "published".to_string() },
        ];
        let mut arr_field = make_field("items", FieldType::Array);
        arr_field.fields = vec![select_sf];
        let fields = vec![arr_field];
        let values = HashMap::new();
        let errors = HashMap::new();
        let result = build_field_contexts(&fields, &values, &errors, false, false);
        let sub_fields = result[0]["sub_fields"].as_array().unwrap();
        let opts = sub_fields[0]["options"].as_array().unwrap();
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0]["value"], "draft");
        assert_eq!(opts[1]["value"], "published");
    }

    #[test]
    fn build_field_contexts_blocks_sub_fields_include_type_and_label() {
        let mut blocks_field = make_field("content", FieldType::Blocks);
        blocks_field.blocks = vec![BlockDefinition {
            block_type: "rich".to_string(),
            label: Some(LocalizedString::Plain("Rich Text".to_string())),
            fields: vec![
                make_field("heading", FieldType::Text),
                make_field("body", FieldType::Richtext),
            ],
            ..Default::default()
        }];
        let fields = vec![blocks_field];
        let values = HashMap::new();
        let errors = HashMap::new();
        let result = build_field_contexts(&fields, &values, &errors, false, false);
        let block_defs = result[0]["block_definitions"].as_array().unwrap();
        assert_eq!(block_defs.len(), 1);
        let block_fields = block_defs[0]["fields"].as_array().unwrap();
        assert_eq!(block_fields.len(), 2);
        assert_eq!(block_fields[0]["field_type"], "text");
        assert_eq!(block_fields[0]["label"], "Heading");
        assert_eq!(block_fields[1]["field_type"], "richtext");
        assert_eq!(block_fields[1]["label"], "Body");
    }

    #[test]
    fn build_field_contexts_blocks_select_sub_field_includes_options() {
        let mut select_sf = make_field("align", FieldType::Select);
        select_sf.options = vec![
            SelectOption { label: LocalizedString::Plain("Left".to_string()), value: "left".to_string() },
            SelectOption { label: LocalizedString::Plain("Center".to_string()), value: "center".to_string() },
        ];
        let mut blocks_field = make_field("layout", FieldType::Blocks);
        blocks_field.blocks = vec![BlockDefinition {
            block_type: "section".to_string(),
            label: None,
            fields: vec![select_sf],
            ..Default::default()
        }];
        let fields = vec![blocks_field];
        let values = HashMap::new();
        let errors = HashMap::new();
        let result = build_field_contexts(&fields, &values, &errors, false, false);
        let block_defs = result[0]["block_definitions"].as_array().unwrap();
        let block_fields = block_defs[0]["fields"].as_array().unwrap();
        let opts = block_fields[0]["options"].as_array().unwrap();
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0]["value"], "left");
        assert_eq!(opts[1]["value"], "center");
    }

    // --- build_field_contexts: date field tests ---

    #[test]
    fn build_field_contexts_date_default_day_only() {
        let date_field = make_field("published_at", FieldType::Date);
        let fields = vec![date_field];
        let mut values = HashMap::new();
        values.insert("published_at".to_string(), "2026-01-15T12:00:00.000Z".to_string());
        let errors = HashMap::new();
        let result = build_field_contexts(&fields, &values, &errors, false, false);
        assert_eq!(result[0]["picker_appearance"], "dayOnly");
        assert_eq!(result[0]["date_only_value"], "2026-01-15");
    }

    #[test]
    fn build_field_contexts_date_day_and_time() {
        let mut date_field = make_field("event_at", FieldType::Date);
        date_field.picker_appearance = Some("dayAndTime".to_string());
        let fields = vec![date_field];
        let mut values = HashMap::new();
        values.insert("event_at".to_string(), "2026-01-15T09:30:00.000Z".to_string());
        let errors = HashMap::new();
        let result = build_field_contexts(&fields, &values, &errors, false, false);
        assert_eq!(result[0]["picker_appearance"], "dayAndTime");
        assert_eq!(result[0]["datetime_local_value"], "2026-01-15T09:30");
    }

    #[test]
    fn build_field_contexts_date_time_only() {
        let mut date_field = make_field("reminder", FieldType::Date);
        date_field.picker_appearance = Some("timeOnly".to_string());
        let fields = vec![date_field];
        let mut values = HashMap::new();
        values.insert("reminder".to_string(), "14:30".to_string());
        let errors = HashMap::new();
        let result = build_field_contexts(&fields, &values, &errors, false, false);
        assert_eq!(result[0]["picker_appearance"], "timeOnly");
        assert_eq!(result[0]["value"], "14:30");
    }

    #[test]
    fn build_field_contexts_date_month_only() {
        let mut date_field = make_field("birth_month", FieldType::Date);
        date_field.picker_appearance = Some("monthOnly".to_string());
        let fields = vec![date_field];
        let mut values = HashMap::new();
        values.insert("birth_month".to_string(), "2026-01".to_string());
        let errors = HashMap::new();
        let result = build_field_contexts(&fields, &values, &errors, false, false);
        assert_eq!(result[0]["picker_appearance"], "monthOnly");
        assert_eq!(result[0]["value"], "2026-01");
    }

    // --- safe_template_id tests ---

    #[test]
    fn safe_template_id_simple_name() {
        assert_eq!(safe_template_id("items"), "items");
    }

    #[test]
    fn safe_template_id_with_brackets() {
        assert_eq!(safe_template_id("content[0][items]"), "content-0-items");
    }

    #[test]
    fn safe_template_id_nested_index_placeholder() {
        assert_eq!(safe_template_id("content[__INDEX__][items]"), "content-__INDEX__-items");
    }

    // --- Recursive build_field_contexts tests (nested composites) ---

    #[test]
    fn build_field_contexts_array_has_template_id() {
        let mut arr_field = make_field("items", FieldType::Array);
        arr_field.fields = vec![make_field("title", FieldType::Text)];
        let fields = vec![arr_field];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["template_id"], "items");
    }

    #[test]
    fn build_field_contexts_blocks_has_template_id() {
        let mut blocks_field = make_field("content", FieldType::Blocks);
        blocks_field.blocks = vec![BlockDefinition {
            block_type: "text".to_string(),
            label: None,
            fields: vec![make_field("body", FieldType::Text)],
            ..Default::default()
        }];
        let fields = vec![blocks_field];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["template_id"], "content");
    }

    #[test]
    fn build_field_contexts_array_sub_fields_have_indexed_names() {
        let mut arr_field = make_field("slides", FieldType::Array);
        arr_field.fields = vec![
            make_field("title", FieldType::Text),
            make_field("body", FieldType::Textarea),
        ];
        let fields = vec![arr_field];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        let sub_fields = result[0]["sub_fields"].as_array().unwrap();
        // Sub-fields in the template context should have __INDEX__ placeholder names
        assert_eq!(sub_fields[0]["name"], "slides[__INDEX__][title]");
        assert_eq!(sub_fields[1]["name"], "slides[__INDEX__][body]");
    }

    #[test]
    fn build_field_contexts_nested_array_in_blocks() {
        // blocks field with a block that contains an array sub-field
        let mut inner_array = make_field("images", FieldType::Array);
        inner_array.fields = vec![
            make_field("url", FieldType::Text),
            make_field("caption", FieldType::Text),
        ];
        let mut blocks_field = make_field("content", FieldType::Blocks);
        blocks_field.blocks = vec![BlockDefinition {
            block_type: "gallery".to_string(),
            label: Some(LocalizedString::Plain("Gallery".to_string())),
            fields: vec![
                make_field("title", FieldType::Text),
                inner_array,
            ],
            ..Default::default()
        }];
        let fields = vec![blocks_field];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);

        let block_defs = result[0]["block_definitions"].as_array().unwrap();
        assert_eq!(block_defs.len(), 1);
        let block_fields = block_defs[0]["fields"].as_array().unwrap();
        assert_eq!(block_fields.len(), 2);

        // First field is simple text
        assert_eq!(block_fields[0]["field_type"], "text");
        assert_eq!(block_fields[0]["name"], "content[__INDEX__][title]");

        // Second field is a nested array
        assert_eq!(block_fields[1]["field_type"], "array");
        assert_eq!(block_fields[1]["name"], "content[__INDEX__][images]");

        // The nested array should have its own sub_fields with double __INDEX__
        let nested_sub_fields = block_fields[1]["sub_fields"].as_array().unwrap();
        assert_eq!(nested_sub_fields.len(), 2);
        assert_eq!(nested_sub_fields[0]["name"], "content[__INDEX__][images][__INDEX__][url]");
        assert_eq!(nested_sub_fields[1]["name"], "content[__INDEX__][images][__INDEX__][caption]");

        // Nested array should have template_id
        assert!(block_fields[1]["template_id"].as_str().is_some());
    }

    #[test]
    fn build_field_contexts_nested_blocks_in_array() {
        // array field with a blocks sub-field
        let mut inner_blocks = make_field("sections", FieldType::Blocks);
        inner_blocks.blocks = vec![BlockDefinition {
            block_type: "text".to_string(),
            label: None,
            fields: vec![make_field("body", FieldType::Richtext)],
            ..Default::default()
        }];
        let mut arr_field = make_field("pages", FieldType::Array);
        arr_field.fields = vec![
            make_field("title", FieldType::Text),
            inner_blocks,
        ];
        let fields = vec![arr_field];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);

        let sub_fields = result[0]["sub_fields"].as_array().unwrap();
        assert_eq!(sub_fields.len(), 2);
        assert_eq!(sub_fields[0]["field_type"], "text");
        assert_eq!(sub_fields[1]["field_type"], "blocks");

        // Nested blocks should have block_definitions
        let nested_block_defs = sub_fields[1]["block_definitions"].as_array().unwrap();
        assert_eq!(nested_block_defs.len(), 1);
        assert_eq!(nested_block_defs[0]["block_type"], "text");

        // The nested block's fields should have proper names
        let nested_block_fields = nested_block_defs[0]["fields"].as_array().unwrap();
        assert_eq!(nested_block_fields[0]["field_type"], "richtext");
        assert_eq!(nested_block_fields[0]["name"], "pages[__INDEX__][sections][__INDEX__][body]");
    }

    #[test]
    fn build_field_contexts_nested_group_in_array() {
        // array with a group sub-field
        let mut inner_group = make_field("meta", FieldType::Group);
        inner_group.fields = vec![
            make_field("author", FieldType::Text),
            make_field("date", FieldType::Date),
        ];
        let mut arr_field = make_field("entries", FieldType::Array);
        arr_field.fields = vec![inner_group];
        let fields = vec![arr_field];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);

        let sub_fields = result[0]["sub_fields"].as_array().unwrap();
        assert_eq!(sub_fields.len(), 1);
        assert_eq!(sub_fields[0]["field_type"], "group");

        // Group sub-fields inside array use bracketed naming
        let group_sub_fields = sub_fields[0]["sub_fields"].as_array().unwrap();
        assert_eq!(group_sub_fields.len(), 2);
        assert_eq!(group_sub_fields[0]["name"], "entries[__INDEX__][meta][author]");
        assert_eq!(group_sub_fields[1]["name"], "entries[__INDEX__][meta][date]");
    }

    #[test]
    fn build_field_contexts_nested_array_in_array() {
        // array containing an array sub-field
        let mut inner_array = make_field("tags", FieldType::Array);
        inner_array.fields = vec![make_field("name", FieldType::Text)];
        let mut outer_array = make_field("items", FieldType::Array);
        outer_array.fields = vec![
            make_field("title", FieldType::Text),
            inner_array,
        ];
        let fields = vec![outer_array];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);

        let sub_fields = result[0]["sub_fields"].as_array().unwrap();
        assert_eq!(sub_fields[1]["field_type"], "array");

        // Nested array sub_fields have double __INDEX__
        let nested_sub = sub_fields[1]["sub_fields"].as_array().unwrap();
        assert_eq!(nested_sub[0]["name"], "items[__INDEX__][tags][__INDEX__][name]");
    }

    // --- split_sidebar_fields tests ---

    #[test]
    fn split_sidebar_fields_separates_by_position() {
        let fields = vec![
            serde_json::json!({"name": "title", "field_type": "text"}),
            serde_json::json!({"name": "slug", "field_type": "text", "position": "sidebar"}),
            serde_json::json!({"name": "body", "field_type": "richtext"}),
            serde_json::json!({"name": "status", "field_type": "select", "position": "sidebar"}),
        ];
        let (main, sidebar) = super::super::split_sidebar_fields(fields);
        assert_eq!(main.len(), 2);
        assert_eq!(sidebar.len(), 2);
        assert_eq!(main[0]["name"], "title");
        assert_eq!(main[1]["name"], "body");
        assert_eq!(sidebar[0]["name"], "slug");
        assert_eq!(sidebar[1]["name"], "status");
    }

    #[test]
    fn split_sidebar_fields_no_sidebar() {
        let fields = vec![
            serde_json::json!({"name": "title", "field_type": "text"}),
            serde_json::json!({"name": "body", "field_type": "richtext"}),
        ];
        let (main, sidebar) = super::super::split_sidebar_fields(fields);
        assert_eq!(main.len(), 2);
        assert!(sidebar.is_empty());
    }

    #[test]
    fn split_sidebar_fields_all_sidebar() {
        let fields = vec![
            serde_json::json!({"name": "a", "position": "sidebar"}),
            serde_json::json!({"name": "b", "position": "sidebar"}),
        ];
        let (main, sidebar) = super::super::split_sidebar_fields(fields);
        assert!(main.is_empty());
        assert_eq!(sidebar.len(), 2);
    }

    #[test]
    fn split_sidebar_fields_empty() {
        let (main, sidebar) = super::super::split_sidebar_fields(vec![]);
        assert!(main.is_empty());
        assert!(sidebar.is_empty());
    }

    // --- build_field_contexts: filter_hidden tests ---

    #[test]
    fn build_field_contexts_filter_hidden_removes_hidden_fields() {
        let mut hidden_field = make_field("secret", FieldType::Text);
        hidden_field.admin.hidden = true;
        let fields = vec![
            make_field("title", FieldType::Text),
            hidden_field,
            make_field("body", FieldType::Textarea),
        ];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), true, false);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["name"], "title");
        assert_eq!(result[1]["name"], "body");
    }

    #[test]
    fn build_field_contexts_no_filter_includes_hidden_fields() {
        let mut hidden_field = make_field("secret", FieldType::Text);
        hidden_field.admin.hidden = true;
        let fields = vec![
            make_field("title", FieldType::Text),
            hidden_field,
        ];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result.len(), 2);
    }

    // --- build_field_contexts: relationship tests ---

    #[test]
    fn build_field_contexts_relationship_has_collection_info() {
        use crate::core::field::RelationshipConfig;
        let mut rel_field = make_field("author", FieldType::Relationship);
        rel_field.relationship = Some(RelationshipConfig {
            collection: "users".to_string(),
            has_many: false,
            max_depth: None,
            polymorphic: vec![],
        });
        let fields = vec![rel_field];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["relationship_collection"], "users");
        assert_eq!(result[0]["has_many"], false);
    }

    #[test]
    fn build_field_contexts_relationship_has_many() {
        use crate::core::field::RelationshipConfig;
        let mut rel_field = make_field("tags", FieldType::Relationship);
        rel_field.relationship = Some(RelationshipConfig {
            collection: "tags".to_string(),
            has_many: true,
            max_depth: None,
            polymorphic: vec![],
        });
        let fields = vec![rel_field];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["relationship_collection"], "tags");
        assert_eq!(result[0]["has_many"], true);
    }

    // --- build_field_contexts: checkbox tests ---

    #[test]
    fn build_field_contexts_checkbox_checked_values() {
        for val in &["1", "true", "on", "yes"] {
            let mut values = HashMap::new();
            values.insert("active".to_string(), val.to_string());
            let fields = vec![make_field("active", FieldType::Checkbox)];
            let result = build_field_contexts(&fields, &values, &HashMap::new(), false, false);
            assert_eq!(result[0]["checked"], true, "Checkbox should be checked for value '{}'", val);
        }
    }

    #[test]
    fn build_field_contexts_checkbox_unchecked_values() {
        for val in &["0", "false", "off", "no", ""] {
            let mut values = HashMap::new();
            values.insert("active".to_string(), val.to_string());
            let fields = vec![make_field("active", FieldType::Checkbox)];
            let result = build_field_contexts(&fields, &values, &HashMap::new(), false, false);
            assert_eq!(result[0]["checked"], false, "Checkbox should be unchecked for value '{}'", val);
        }
    }

    // --- build_field_contexts: upload field tests ---

    #[test]
    fn build_field_contexts_upload_has_collection() {
        use crate::core::field::RelationshipConfig;
        let mut upload_field = make_field("image", FieldType::Upload);
        upload_field.relationship = Some(RelationshipConfig {
            collection: "media".to_string(),
            has_many: false,
            max_depth: None,
            polymorphic: vec![],
        });
        let fields = vec![upload_field];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["relationship_collection"], "media");
        assert_eq!(result[0]["picker"], "drawer", "upload fields default to drawer picker");
    }

    // --- build_field_contexts: select tests ---

    #[test]
    fn build_field_contexts_select_marks_selected_option() {
        let mut sel = make_field("color", FieldType::Select);
        sel.options = vec![
            SelectOption { label: LocalizedString::Plain("Red".to_string()), value: "red".to_string() },
            SelectOption { label: LocalizedString::Plain("Blue".to_string()), value: "blue".to_string() },
        ];
        let mut values = HashMap::new();
        values.insert("color".to_string(), "blue".to_string());
        let fields = vec![sel];
        let result = build_field_contexts(&fields, &values, &HashMap::new(), false, false);
        let opts = result[0]["options"].as_array().unwrap();
        assert_eq!(opts[0]["selected"], false);
        assert_eq!(opts[1]["selected"], true);
    }

    // --- build_field_contexts: error propagation ---

    #[test]
    fn build_field_contexts_errors_attached_to_fields() {
        let fields = vec![make_field("title", FieldType::Text)];
        let mut errors = HashMap::new();
        errors.insert("title".to_string(), "Title is required".to_string());
        let result = build_field_contexts(&fields, &HashMap::new(), &errors, false, false);
        assert_eq!(result[0]["error"], "Title is required");
    }

    #[test]
    fn build_field_contexts_no_error_when_field_valid() {
        let fields = vec![make_field("title", FieldType::Text)];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert!(result[0].get("error").is_none());
    }

    // --- build_field_contexts: locale locking ---

    #[test]
    fn build_field_contexts_locale_locked_non_localized_field() {
        let fields = vec![make_field("slug", FieldType::Text)]; // not localized
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, true);
        assert_eq!(result[0]["locale_locked"], true);
        assert_eq!(result[0]["readonly"], true);
    }

    #[test]
    fn build_field_contexts_localized_field_not_locked() {
        let mut field = make_field("title", FieldType::Text);
        field.localized = true;
        let fields = vec![field];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, true);
        assert_eq!(result[0]["locale_locked"], false);
        assert_eq!(result[0]["readonly"], false);
    }

    // --- build_field_contexts: group field tests ---

    #[test]
    fn build_field_contexts_top_level_group_uses_double_underscore() {
        let mut group = make_field("seo", FieldType::Group);
        group.fields = vec![
            make_field("title", FieldType::Text),
            make_field("description", FieldType::Textarea),
        ];
        let fields = vec![group];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        let sub_fields = result[0]["sub_fields"].as_array().unwrap();
        assert_eq!(sub_fields[0]["name"], "seo__title");
        assert_eq!(sub_fields[1]["name"], "seo__description");
    }

    #[test]
    fn build_field_contexts_group_collapsed() {
        let mut group = make_field("meta", FieldType::Group);
        group.admin.collapsed = true;
        group.fields = vec![make_field("author", FieldType::Text)];
        let fields = vec![group];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["collapsed"], true);
    }

    #[test]
    fn build_field_contexts_group_sub_field_values() {
        let mut group = make_field("seo", FieldType::Group);
        group.fields = vec![make_field("title", FieldType::Text)];
        let mut values = HashMap::new();
        values.insert("seo__title".to_string(), "My SEO Title".to_string());
        let fields = vec![group];
        let result = build_field_contexts(&fields, &values, &HashMap::new(), false, false);
        let sub_fields = result[0]["sub_fields"].as_array().unwrap();
        assert_eq!(sub_fields[0]["value"], "My SEO Title");
    }

    // --- build_field_contexts: array with min/max rows and admin options ---

    #[test]
    fn build_field_contexts_array_with_min_max_rows() {
        let mut arr = make_field("items", FieldType::Array);
        arr.fields = vec![make_field("title", FieldType::Text)];
        arr.min_rows = Some(1);
        arr.max_rows = Some(5);
        let fields = vec![arr];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["min_rows"], 1);
        assert_eq!(result[0]["max_rows"], 5);
    }

    #[test]
    fn build_field_contexts_array_collapsed() {
        let mut arr = make_field("items", FieldType::Array);
        arr.fields = vec![make_field("title", FieldType::Text)];
        // collapsed defaults to true, verify it's set in template context
        let fields = vec![arr];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["init_collapsed"], true);

        // opt-out: collapsed = false
        let mut arr2 = make_field("items", FieldType::Array);
        arr2.fields = vec![make_field("title", FieldType::Text)];
        arr2.admin.collapsed = false;
        let fields2 = vec![arr2];
        let result2 = build_field_contexts(&fields2, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result2[0]["init_collapsed"], false);
    }

    #[test]
    fn build_field_contexts_array_labels_singular() {
        let mut arr = make_field("slides", FieldType::Array);
        arr.fields = vec![make_field("title", FieldType::Text)];
        arr.admin.labels_singular = Some(LocalizedString::Plain("Slide".to_string()));
        let fields = vec![arr];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["add_label"], "Slide");
    }

    #[test]
    fn build_field_contexts_array_label_field() {
        let mut arr = make_field("items", FieldType::Array);
        arr.fields = vec![make_field("title", FieldType::Text)];
        arr.admin.label_field = Some("title".to_string());
        let fields = vec![arr];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["label_field"], "title");
    }

    // --- build_field_contexts: blocks with min/max rows and admin options ---

    #[test]
    fn build_field_contexts_blocks_with_min_max_rows() {
        let mut blocks = make_field("content", FieldType::Blocks);
        blocks.blocks = vec![BlockDefinition {
            block_type: "text".to_string(),
            label: None,
            fields: vec![make_field("body", FieldType::Text)],
            ..Default::default()
        }];
        blocks.min_rows = Some(1);
        blocks.max_rows = Some(10);
        let fields = vec![blocks];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["min_rows"], 1);
        assert_eq!(result[0]["max_rows"], 10);
    }

    #[test]
    fn build_field_contexts_blocks_collapsed() {
        let mut blocks = make_field("content", FieldType::Blocks);
        blocks.blocks = vec![BlockDefinition {
            block_type: "text".to_string(),
            label: None,
            fields: vec![make_field("body", FieldType::Text)],
            ..Default::default()
        }];
        // collapsed defaults to true
        let fields = vec![blocks];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["init_collapsed"], true);
    }

    #[test]
    fn build_field_contexts_blocks_labels_singular() {
        let mut blocks = make_field("content", FieldType::Blocks);
        blocks.blocks = vec![BlockDefinition {
            block_type: "text".to_string(),
            label: None,
            fields: vec![make_field("body", FieldType::Text)],
            ..Default::default()
        }];
        blocks.admin.labels_singular = Some(LocalizedString::Plain("Block".to_string()));
        let fields = vec![blocks];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["add_label"], "Block");
    }

    #[test]
    fn build_field_contexts_blocks_block_label_field() {
        let mut blocks = make_field("content", FieldType::Blocks);
        blocks.blocks = vec![BlockDefinition {
            block_type: "text".to_string(),
            label: None,
            fields: vec![make_field("body", FieldType::Text)],
            label_field: Some("body".to_string()),
            ..Default::default()
        }];
        let fields = vec![blocks];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        let block_defs = result[0]["block_definitions"].as_array().unwrap();
        assert_eq!(block_defs[0]["label_field"], "body");
    }

    #[test]
    fn build_field_contexts_blocks_group_and_image_url() {
        let mut blocks = make_field("content", FieldType::Blocks);
        blocks.blocks = vec![
            BlockDefinition {
                block_type: "hero".to_string(),
                label: Some(LocalizedString::Plain("Hero".to_string())),
                group: Some("Layout".to_string()),
                image_url: Some("/static/blocks/hero.svg".to_string()),
                ..Default::default()
            },
            BlockDefinition {
                block_type: "text".to_string(),
                label: Some(LocalizedString::Plain("Text".to_string())),
                group: Some("Content".to_string()),
                ..Default::default()
            },
            BlockDefinition {
                block_type: "divider".to_string(),
                label: Some(LocalizedString::Plain("Divider".to_string())),
                ..Default::default()
            },
        ];
        let fields = vec![blocks];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        let block_defs = result[0]["block_definitions"].as_array().unwrap();

        assert_eq!(block_defs[0]["group"], "Layout");
        assert_eq!(block_defs[0]["image_url"], "/static/blocks/hero.svg");

        assert_eq!(block_defs[1]["group"], "Content");
        assert!(block_defs[1].get("image_url").is_none_or(|v| v.is_null()));

        assert!(block_defs[2].get("group").is_none_or(|v| v.is_null()));
        assert!(block_defs[2].get("image_url").is_none_or(|v| v.is_null()));
    }

    #[test]
    fn build_field_contexts_blocks_picker_card() {
        let mut blocks = make_field("content", FieldType::Blocks);
        blocks.admin.picker = Some("card".to_string());
        blocks.blocks = vec![BlockDefinition {
            block_type: "text".to_string(),
            fields: vec![make_field("body", FieldType::Text)],
            ..Default::default()
        }];
        let fields = vec![blocks];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["picker"], "card");
    }

    // --- has_many text/number inside composites regression tests ---

    #[test]
    fn has_many_text_in_group_gets_tags_context() {
        // Bug fix: has_many text inside a Group should produce tags/has_many context
        let mut group = make_field("meta", FieldType::Group);
        let mut tags = make_field("tags", FieldType::Text);
        tags.has_many = true;
        group.fields = vec![tags];
        let fields = vec![group];

        let mut values = HashMap::new();
        values.insert("meta__tags".to_string(), r#"["rust","lua"]"#.to_string());
        let result = build_field_contexts(&fields, &values, &HashMap::new(), false, false);

        let sub = result[0]["sub_fields"].as_array().unwrap();
        assert_eq!(sub[0]["has_many"], true);
        let tags_arr = sub[0]["tags"].as_array().unwrap();
        assert_eq!(tags_arr.len(), 2);
        assert_eq!(tags_arr[0], "rust");
        assert_eq!(tags_arr[1], "lua");
        assert_eq!(sub[0]["value"], "rust,lua");
    }

    // --- build_field_contexts: position field ---

    #[test]
    fn build_field_contexts_position_set() {
        let mut field = make_field("status", FieldType::Text);
        field.admin.position = Some("sidebar".to_string());
        let fields = vec![field];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["position"], "sidebar");
    }

    // --- build_field_contexts: label, placeholder, description ---

    #[test]
    fn build_field_contexts_custom_label_placeholder_description() {
        let mut field = make_field("title", FieldType::Text);
        field.admin.label = Some(LocalizedString::Plain("Custom Title".to_string()));
        field.admin.placeholder = Some(LocalizedString::Plain("Enter title here...".to_string()));
        field.admin.description = Some(LocalizedString::Plain("The main title".to_string()));
        let fields = vec![field];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["label"], "Custom Title");
        assert_eq!(result[0]["placeholder"], "Enter title here...");
        assert_eq!(result[0]["description"], "The main title");
    }

    #[test]
    fn build_field_contexts_readonly_field() {
        let mut field = make_field("slug", FieldType::Text);
        field.admin.readonly = true;
        let fields = vec![field];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["readonly"], true);
    }

    // --- build_field_contexts: date short values ---

    #[test]
    fn build_field_contexts_date_short_value_day_only() {
        let mut values = HashMap::new();
        values.insert("d".to_string(), "short".to_string()); // less than 10 chars
        let field = make_field("d", FieldType::Date);
        let fields = vec![field];
        let result = build_field_contexts(&fields, &values, &HashMap::new(), false, false);
        // Should use the short value as-is
        assert_eq!(result[0]["date_only_value"], "short");
    }

    #[test]
    fn build_field_contexts_date_short_value_day_and_time() {
        let mut field = make_field("d", FieldType::Date);
        field.picker_appearance = Some("dayAndTime".to_string());
        let mut values = HashMap::new();
        values.insert("d".to_string(), "short".to_string()); // less than 16 chars
        let fields = vec![field];
        let result = build_field_contexts(&fields, &values, &HashMap::new(), false, false);
        assert_eq!(result[0]["datetime_local_value"], "short");
    }

    // --- apply_field_type_extras tests ---

    #[test]
    fn apply_extras_checkbox_checked() {
        let sf = make_field("active", FieldType::Checkbox);
        let mut ctx = serde_json::json!({"name": "group__active"});
        apply_field_type_extras(&sf, "true", &mut ctx, &HashMap::new(), &HashMap::new(), "group__active", false, 0);
        assert_eq!(ctx["checked"], true);
    }

    #[test]
    fn apply_extras_checkbox_unchecked() {
        let sf = make_field("active", FieldType::Checkbox);
        let mut ctx = serde_json::json!({"name": "group__active"});
        apply_field_type_extras(&sf, "0", &mut ctx, &HashMap::new(), &HashMap::new(), "group__active", false, 0);
        assert_eq!(ctx["checked"], false);
    }

    #[test]
    fn apply_extras_select() {
        let mut sf = make_field("color", FieldType::Select);
        sf.options = vec![
            SelectOption { label: LocalizedString::Plain("Red".to_string()), value: "red".to_string() },
            SelectOption { label: LocalizedString::Plain("Green".to_string()), value: "green".to_string() },
        ];
        let mut ctx = serde_json::json!({"name": "group__color"});
        apply_field_type_extras(&sf, "green", &mut ctx, &HashMap::new(), &HashMap::new(), "group__color", false, 0);
        let opts = ctx["options"].as_array().unwrap();
        assert_eq!(opts[0]["selected"], false);
        assert_eq!(opts[1]["selected"], true);
    }

    #[test]
    fn apply_extras_date_day_only() {
        let sf = make_field("d", FieldType::Date);
        let mut ctx = serde_json::json!({"name": "group__d"});
        apply_field_type_extras(&sf, "2026-01-15T12:00:00Z", &mut ctx, &HashMap::new(), &HashMap::new(), "group__d", false, 0);
        assert_eq!(ctx["picker_appearance"], "dayOnly");
        assert_eq!(ctx["date_only_value"], "2026-01-15");
    }

    #[test]
    fn apply_extras_date_day_and_time() {
        let mut sf = make_field("d", FieldType::Date);
        sf.picker_appearance = Some("dayAndTime".to_string());
        let mut ctx = serde_json::json!({"name": "group__d"});
        apply_field_type_extras(&sf, "2026-01-15T09:30:00Z", &mut ctx, &HashMap::new(), &HashMap::new(), "group__d", false, 0);
        assert_eq!(ctx["picker_appearance"], "dayAndTime");
        assert_eq!(ctx["datetime_local_value"], "2026-01-15T09:30");
    }

    #[test]
    fn apply_extras_date_short_values() {
        let sf = make_field("d", FieldType::Date);
        let mut ctx = serde_json::json!({"name": "g__d"});
        apply_field_type_extras(&sf, "short", &mut ctx, &HashMap::new(), &HashMap::new(), "g__d", false, 0);
        assert_eq!(ctx["date_only_value"], "short");

        let mut sf2 = make_field("d2", FieldType::Date);
        sf2.picker_appearance = Some("dayAndTime".to_string());
        let mut ctx2 = serde_json::json!({"name": "g__d2"});
        apply_field_type_extras(&sf2, "short", &mut ctx2, &HashMap::new(), &HashMap::new(), "g__d2", false, 0);
        assert_eq!(ctx2["datetime_local_value"], "short");
    }

    #[test]
    fn apply_extras_relationship() {
        use crate::core::field::RelationshipConfig;
        let mut sf = make_field("author", FieldType::Relationship);
        sf.relationship = Some(RelationshipConfig {
            collection: "users".to_string(),
            has_many: true,
            max_depth: None,
            polymorphic: vec![],
        });
        let mut ctx = serde_json::json!({"name": "group__author"});
        apply_field_type_extras(&sf, "", &mut ctx, &HashMap::new(), &HashMap::new(), "group__author", false, 0);
        assert_eq!(ctx["relationship_collection"], "users");
        assert_eq!(ctx["has_many"], true);
    }

    #[test]
    fn apply_extras_upload() {
        use crate::core::field::RelationshipConfig;
        let mut sf = make_field("image", FieldType::Upload);
        sf.relationship = Some(RelationshipConfig {
            collection: "media".to_string(),
            has_many: false,
            max_depth: None,
            polymorphic: vec![],
        });
        let mut ctx = serde_json::json!({"name": "group__image"});
        apply_field_type_extras(&sf, "", &mut ctx, &HashMap::new(), &HashMap::new(), "group__image", false, 0);
        assert_eq!(ctx["relationship_collection"], "media");
        assert_eq!(ctx["picker"], "drawer");
    }

    #[test]
    fn apply_extras_array_in_group() {
        let mut arr = make_field("tags", FieldType::Array);
        arr.fields = vec![make_field("name", FieldType::Text)];
        arr.min_rows = Some(1);
        arr.max_rows = Some(3);
        arr.admin.collapsed = true;
        arr.admin.labels_singular = Some(LocalizedString::Plain("Tag".to_string()));
        arr.admin.label_field = Some("name".to_string());
        let mut ctx = serde_json::json!({"name": "group__tags"});
        apply_field_type_extras(&arr, "", &mut ctx, &HashMap::new(), &HashMap::new(), "group__tags", false, 0);
        assert!(ctx["sub_fields"].as_array().is_some());
        assert_eq!(ctx["row_count"], 0);
        assert_eq!(ctx["min_rows"], 1);
        assert_eq!(ctx["max_rows"], 3);
        assert_eq!(ctx["init_collapsed"], true);
        assert_eq!(ctx["add_label"], "Tag");
        assert_eq!(ctx["label_field"], "name");
    }

    #[test]
    fn apply_extras_group_in_group() {
        let mut inner = make_field("meta", FieldType::Group);
        inner.fields = vec![make_field("author", FieldType::Text)];
        inner.admin.collapsed = true;
        let mut ctx = serde_json::json!({"name": "outer__meta"});
        apply_field_type_extras(&inner, "", &mut ctx, &HashMap::new(), &HashMap::new(), "outer__meta", false, 0);
        assert!(ctx["sub_fields"].as_array().is_some());
        assert_eq!(ctx["collapsed"], true);
    }

    #[test]
    fn apply_extras_blocks_in_group() {
        let mut blk = make_field("sections", FieldType::Blocks);
        blk.blocks = vec![BlockDefinition {
            block_type: "text".to_string(),
            label: None,
            fields: vec![make_field("body", FieldType::Text)],
            label_field: Some("body".to_string()),
            ..Default::default()
        }];
        blk.min_rows = Some(0);
        blk.max_rows = Some(5);
        blk.admin.collapsed = true;
        blk.admin.labels_singular = Some(LocalizedString::Plain("Section".to_string()));
        let mut ctx = serde_json::json!({"name": "group__sections"});
        apply_field_type_extras(&blk, "", &mut ctx, &HashMap::new(), &HashMap::new(), "group__sections", false, 0);
        assert!(ctx["block_definitions"].as_array().is_some());
        assert_eq!(ctx["row_count"], 0);
        assert_eq!(ctx["min_rows"], 0);
        assert_eq!(ctx["max_rows"], 5);
        assert_eq!(ctx["init_collapsed"], true);
        assert_eq!(ctx["add_label"], "Section");
        let bd = ctx["block_definitions"].as_array().unwrap();
        assert_eq!(bd[0]["label_field"], "body");
    }

    #[test]
    fn apply_extras_max_depth_stops_recursion() {
        let mut arr = make_field("deep", FieldType::Array);
        arr.fields = vec![make_field("leaf", FieldType::Text)];
        let mut ctx = serde_json::json!({"name": "group__deep"});
        apply_field_type_extras(&arr, "", &mut ctx, &HashMap::new(), &HashMap::new(), "group__deep", false, MAX_FIELD_DEPTH);
        // At max depth, no sub_fields should be added
        assert!(ctx.get("sub_fields").is_none());
    }

    #[test]
    fn apply_extras_unknown_type_is_noop() {
        let sf = make_field("body", FieldType::Richtext);
        let mut ctx = serde_json::json!({"name": "group__body", "field_type": "richtext"});
        apply_field_type_extras(&sf, "hello", &mut ctx, &HashMap::new(), &HashMap::new(), "group__body", false, 0);
        // Should not add any extra fields
        assert!(ctx.get("options").is_none());
        assert!(ctx.get("checked").is_none());
    }

    // --- count_errors_in_fields tests ---

    #[test]
    fn count_errors_empty_fields() {
        assert_eq!(super::super::count_errors_in_fields(&[]), 0);
    }

    #[test]
    fn count_errors_no_errors() {
        let fields = vec![
            serde_json::json!({"name": "title", "value": "hello"}),
            serde_json::json!({"name": "body", "value": "world"}),
        ];
        assert_eq!(super::super::count_errors_in_fields(&fields), 0);
    }

    #[test]
    fn count_errors_direct_errors() {
        let fields = vec![
            serde_json::json!({"name": "title", "error": "Required"}),
            serde_json::json!({"name": "body", "value": "ok"}),
            serde_json::json!({"name": "email", "error": "Invalid email"}),
        ];
        assert_eq!(super::super::count_errors_in_fields(&fields), 2);
    }

    #[test]
    fn count_errors_nested_in_sub_fields() {
        let fields = vec![
            serde_json::json!({
                "name": "group1",
                "sub_fields": [
                    {"name": "nested1", "error": "Too short"},
                    {"name": "nested2", "value": "ok"},
                ]
            }),
        ];
        assert_eq!(super::super::count_errors_in_fields(&fields), 1);
    }

    #[test]
    fn count_errors_nested_in_tabs() {
        let fields = vec![
            serde_json::json!({
                "name": "settings",
                "tabs": [
                    {
                        "label": "General",
                        "sub_fields": [
                            {"name": "f1", "error": "Required"},
                            {"name": "f2", "error": "Too long"},
                        ]
                    },
                    {
                        "label": "Advanced",
                        "sub_fields": [
                            {"name": "f3", "value": "ok"},
                        ]
                    }
                ]
            }),
        ];
        assert_eq!(super::super::count_errors_in_fields(&fields), 2);
    }

    #[test]
    fn count_errors_nested_in_array_rows() {
        let fields = vec![
            serde_json::json!({
                "name": "items",
                "rows": [
                    {
                        "index": 0,
                        "sub_fields": [
                            {"name": "items[0][title]", "error": "Required"},
                        ]
                    },
                    {
                        "index": 1,
                        "sub_fields": [
                            {"name": "items[1][title]", "value": "ok"},
                        ]
                    }
                ]
            }),
        ];
        assert_eq!(super::super::count_errors_in_fields(&fields), 1);
    }

    #[test]
    fn count_errors_null_error_not_counted() {
        let fields = vec![
            serde_json::json!({"name": "title", "error": null}),
        ];
        assert_eq!(super::super::count_errors_in_fields(&fields), 0);
    }

    #[test]
    fn tabs_field_context_includes_error_count() {
        use crate::core::field::FieldTab;

        let mut tabs_field = make_field("settings", FieldType::Tabs);
        tabs_field.tabs = vec![
            FieldTab {
                label: "General".to_string(),
                description: None,
                fields: vec![
                    {
                        let mut f = make_field("title", FieldType::Text);
                        f.required = true;
                        f
                    },
                    make_field("slug", FieldType::Text),
                ],
            },
            FieldTab {
                label: "Advanced".to_string(),
                description: None,
                fields: vec![
                    make_field("meta", FieldType::Text),
                ],
            },
        ];

        let values = HashMap::new(); // empty values -> required field "title" has no value
        let mut errors = HashMap::new();
        errors.insert("title".to_string(), "Title is required".to_string());

        let result = build_field_contexts(&[tabs_field], &values, &errors, false, false);
        let tabs = result[0]["tabs"].as_array().expect("tabs should be an array");

        // First tab has 1 error (title is required)
        assert_eq!(tabs[0]["error_count"], 1);
        // Second tab has no errors
        assert!(tabs[1].get("error_count").is_none() || tabs[1]["error_count"].is_null());
    }

    // --- richtext_format context ---

    #[test]
    fn richtext_format_defaults_to_html() {
        let field = make_field("body", FieldType::Richtext);
        let fields = vec![field];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["richtext_format"], "html");
    }

    #[test]
    fn richtext_format_json() {
        let mut field = make_field("body", FieldType::Richtext);
        field.admin.richtext_format = Some("json".to_string());
        let fields = vec![field];
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result[0]["richtext_format"], "json");
    }

    #[test]
    fn max_depth_prevents_infinite_recursion() {
        // Build a deeply nested array structure
        fn make_nested_array(depth: usize) -> FieldDefinition {
            let mut field = FieldDefinition {
                name: format!("level{}", depth),
                field_type: FieldType::Array,
                ..Default::default()
            };
            if depth < 10 {
                field.fields = vec![make_nested_array(depth + 1)];
            } else {
                field.fields = vec![FieldDefinition {
                    name: "leaf".to_string(),
                    field_type: FieldType::Text,
                    ..Default::default()
                }];
            }
            field
        }
        let deep = make_nested_array(0);
        let fields = vec![deep];
        // This should not stack overflow -- MAX_FIELD_DEPTH caps recursion
        let result = build_field_contexts(&fields, &HashMap::new(), &HashMap::new(), false, false);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["field_type"], "array");
    }
}
