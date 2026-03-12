use crate::{admin::AdminState, hooks::lifecycle::DisplayConditionResult};

use axum::{Json, extract::State, response::IntoResponse};
use serde_json::{Map, Value, json};
use std::collections::HashMap;

/// POST /admin/collections/{slug}/evaluate-conditions
/// Evaluates server-only display conditions with current form data.
/// Returns JSON: { "field_name": true/false, ... }
pub async fn evaluate_conditions(
    State(state): State<AdminState>,
    Json(req): Json<EvaluateConditionsRequest>,
) -> impl IntoResponse {
    let form_data = json!(req.form_data);
    let mut results = Map::new();

    for (field_name, func_ref) in &req.conditions {
        let visible = match state
            .hook_runner
            .call_display_condition(func_ref, &form_data)
        {
            Some(DisplayConditionResult::Bool(b)) => b,
            Some(DisplayConditionResult::Table { visible, .. }) => visible,
            None => true, // error -> show
        };

        results.insert(field_name.clone(), json!(visible));
    }

    Json(Value::Object(results))
}

/// Request payload for evaluating field display conditions.
#[derive(serde::Deserialize)]
pub struct EvaluateConditionsRequest {
    /// The current form data.
    pub form_data: HashMap<String, serde_json::Value>,
    /// Map of field names to their condition function references.
    pub conditions: HashMap<String, String>,
}
