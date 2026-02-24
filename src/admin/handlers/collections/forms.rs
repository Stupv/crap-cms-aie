//! Form parsing helpers: multipart, array fields, upload metadata.

use axum::extract::{FromRequest, Multipart};
use std::collections::HashMap;

use crate::admin::AdminState;
use crate::core::field::FieldType;
use crate::core::upload::{UploadedFile, ProcessedUpload};

/// Extract join table data from form submission for has-many relationships and array fields.
/// Returns a map suitable for `query::save_join_table_data`.
pub(super) fn extract_join_data_from_form(
    form: &HashMap<String, String>,
    field_defs: &[crate::core::field::FieldDefinition],
) -> HashMap<String, serde_json::Value> {
    let mut join_data = HashMap::new();

    for field in field_defs {
        match field.field_type {
            FieldType::Relationship => {
                if let Some(ref rc) = field.relationship {
                    if rc.has_many {
                        // Has-many: comma-separated IDs in form value
                        if let Some(val) = form.get(&field.name) {
                            join_data.insert(field.name.clone(), serde_json::Value::String(val.clone()));
                        } else {
                            // Empty selection — clear all
                            join_data.insert(field.name.clone(), serde_json::Value::String(String::new()));
                        }
                    }
                }
            }
            FieldType::Array => {
                let rows = parse_array_form_data(form, &field.name);
                let json_rows: Vec<serde_json::Value> = rows.into_iter()
                    .map(|row| {
                        let obj: serde_json::Map<String, serde_json::Value> = row.into_iter()
                            .map(|(k, v)| (k, serde_json::Value::String(v)))
                            .collect();
                        serde_json::Value::Object(obj)
                    })
                    .collect();
                join_data.insert(field.name.clone(), serde_json::Value::Array(json_rows));
            }
            FieldType::Blocks => {
                // Same form data pattern as arrays: name[idx][key]
                // _block_type comes as name[idx][_block_type]
                let rows = parse_array_form_data(form, &field.name);
                let json_rows: Vec<serde_json::Value> = rows.into_iter()
                    .map(|row| {
                        let obj: serde_json::Map<String, serde_json::Value> = row.into_iter()
                            .map(|(k, v)| (k, serde_json::Value::String(v)))
                            .collect();
                        serde_json::Value::Object(obj)
                    })
                    .collect();
                join_data.insert(field.name.clone(), serde_json::Value::Array(json_rows));
            }
            _ => {}
        }
    }

    join_data
}

/// Parse array sub-field data from flat form keys.
/// Converts keys like `slides[0][title]`, `slides[1][caption]` into
/// a Vec of row hashmaps.
fn parse_array_form_data(form: &HashMap<String, String>, field_name: &str) -> Vec<HashMap<String, String>> {
    let prefix = format!("{}[", field_name);
    let mut rows: std::collections::BTreeMap<usize, HashMap<String, String>> = std::collections::BTreeMap::new();

    for (key, value) in form {
        if let Some(rest) = key.strip_prefix(&prefix) {
            // rest looks like "0][title]"
            if let Some(bracket_pos) = rest.find(']') {
                if let Ok(idx) = rest[..bracket_pos].parse::<usize>() {
                    // After "]" we expect "[fieldname]"
                    let after = &rest[bracket_pos + 1..];
                    if let Some(field_key) = after.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                        rows.entry(idx).or_default().insert(field_key.to_string(), value.clone());
                    }
                }
            }
        }
    }

    rows.into_values().collect()
}

/// Parse a multipart form request, extracting form fields and an optional file upload.
pub(super) async fn parse_multipart_form(
    request: axum::extract::Request,
    state: &AdminState,
) -> Result<(HashMap<String, String>, Option<UploadedFile>), anyhow::Error> {
    let mut multipart = Multipart::from_request(request, state).await
        .map_err(|e| anyhow::anyhow!("Failed to parse multipart: {}", e))?;

    let mut form_data = HashMap::new();
    let mut file: Option<UploadedFile> = None;

    while let Some(field) = multipart.next_field().await
        .map_err(|e| anyhow::anyhow!("Failed to read multipart field: {}", e))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "_file" && field.file_name().is_some() {
            let filename = field.file_name().unwrap_or("").to_string();
            let content_type = field.content_type()
                .unwrap_or("application/octet-stream").to_string();
            let data = field.bytes().await
                .map_err(|e| anyhow::anyhow!("Failed to read file data: {}", e))?;
            if !data.is_empty() {
                file = Some(UploadedFile {
                    filename,
                    content_type,
                    data: data.to_vec(),
                });
            }
        } else {
            let text = field.text().await.unwrap_or_default();
            form_data.insert(name, text);
        }
    }

    Ok((form_data, file))
}

/// Inject upload metadata fields into form data from a processed upload.
/// Writes per-size typed fields ({name}_url, {name}_width, {name}_height, {name}_webp_url, etc.)
pub(super) fn inject_upload_metadata(form_data: &mut HashMap<String, String>, processed: &ProcessedUpload) {
    form_data.insert("filename".into(), processed.filename.clone());
    form_data.insert("mime_type".into(), processed.mime_type.clone());
    form_data.insert("filesize".into(), processed.filesize.to_string());
    if let Some(w) = processed.width {
        form_data.insert("width".into(), w.to_string());
    }
    if let Some(h) = processed.height {
        form_data.insert("height".into(), h.to_string());
    }
    form_data.insert("url".into(), processed.url.clone());

    // Per-size typed fields
    for (name, size) in &processed.sizes {
        form_data.insert(format!("{}_url", name), size.url.clone());
        form_data.insert(format!("{}_width", name), size.width.to_string());
        form_data.insert(format!("{}_height", name), size.height.to_string());
        for (fmt, result) in &size.formats {
            form_data.insert(format!("{}_{}_url", name, fmt), result.url.clone());
        }
    }
}

