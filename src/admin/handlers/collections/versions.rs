//! Collection version history handlers.

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Extension,
};
use std::collections::HashMap;

use crate::admin::AdminState;
use crate::admin::context::{ContextBuilder, PageType, Breadcrumb};
use crate::core::auth::{AuthUser, Claims};
use crate::db::{ops, query};
use crate::db::query::{AccessResult, LocaleContext};

use super::{
    PaginationParams,
    check_access_or_forbid, extract_editor_locale,
    version_to_json,
    forbidden, redirect_response, htmx_redirect,
    render_or_error, not_found, server_error,
};

/// POST /admin/collections/{slug}/{id}/versions/{version_id}/restore — restore a version
pub async fn restore_version(
    State(state): State<AdminState>,
    Path((slug, id, version_id)): Path<(String, String, String)>,
    auth_user: Option<Extension<AuthUser>>,
) -> impl IntoResponse {
    let def = match state.registry.get_collection(&slug) {
        Some(d) => d.clone(),
        None => return redirect_response("/admin/collections"),
    };

    if !def.has_versions() {
        return redirect_response(&format!("/admin/collections/{}/{}", slug, id));
    }

    // Check update access
    match check_access_or_forbid(
        &state, def.access.update.as_deref(), &auth_user, Some(&id), None,
    ) {
        Ok(AccessResult::Denied) => return forbidden(&state, "You don't have permission to update this item").into_response(),
        Err(resp) => return resp,
        _ => {}
    }

    let pool = state.pool.clone();
    let slug_owned = slug.clone();
    let id_owned = id.clone();
    let def_owned = def.clone();
    let locale_config = state.config.locale.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e| anyhow::anyhow!("DB connection: {}", e))?;
        let tx = conn.transaction().map_err(|e| anyhow::anyhow!("Start transaction: {}", e))?;
        let version = query::find_version_by_id(&tx, &slug_owned, &version_id)?
            .ok_or_else(|| anyhow::anyhow!("Version not found"))?;
        let doc = query::restore_version(&tx, &slug_owned, &def_owned, &id_owned, &version.snapshot, "published", &locale_config)?;
        tx.commit().map_err(|e| anyhow::anyhow!("Commit: {}", e))?;
        Ok::<_, anyhow::Error>(doc)
    }).await;

    match result {
        Ok(Ok(_)) => htmx_redirect(&format!("/admin/collections/{}/{}", slug, id)),
        Ok(Err(e)) => {
            tracing::error!("Restore version error: {}", e);
            htmx_redirect(&format!("/admin/collections/{}/{}", slug, id))
        }
        Err(e) => {
            tracing::error!("Restore version task error: {}", e);
            htmx_redirect(&format!("/admin/collections/{}/{}", slug, id))
        }
    }
}

/// GET /admin/collections/{slug}/{id}/versions — dedicated version history page
pub async fn list_versions_page(
    State(state): State<AdminState>,
    Path((slug, id)): Path<(String, String)>,
    Query(params): Query<PaginationParams>,
    headers: axum::http::HeaderMap,
    claims: Option<Extension<Claims>>,
    auth_user: Option<Extension<AuthUser>>,
) -> impl IntoResponse {
    let def = match state.registry.get_collection(&slug) {
        Some(d) => d.clone(),
        None => return not_found(&state, &format!("Collection '{}' not found", slug)).into_response(),
    };

    if !def.has_versions() {
        return redirect_response(&format!("/admin/collections/{}/{}", slug, id)).into_response();
    }

    // Check read access
    match check_access_or_forbid(
        &state, def.access.read.as_deref(), &auth_user, Some(&id), None,
    ) {
        Ok(AccessResult::Denied) => return forbidden(&state, "You don't have permission to view this item").into_response(),
        Err(resp) => return resp,
        _ => {}
    }

    // Build locale context so localized column names resolve correctly
    let locale_ctx = LocaleContext::from_locale_string(None, &state.config.locale);

    // Fetch the document for breadcrumb title
    let document = match ops::find_document_by_id(&state.pool, &slug, &def, &id, locale_ctx.as_ref()) {
        Ok(Some(doc)) => doc,
        Ok(None) => return not_found(&state, &format!("Document '{}' not found", id)).into_response(),
        Err(e) => { tracing::error!("Document versions query error: {}", e); return server_error(&state, "An internal error occurred.").into_response(); }
    };

    let doc_title = def.title_field()
        .and_then(|f| document.get_str(f))
        .map(|s| s.to_string())
        .unwrap_or_else(|| document.id.clone());

    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page
        .unwrap_or(state.config.pagination.default_limit)
        .min(state.config.pagination.max_limit);
    let offset = (page - 1) * per_page;

    let conn = match state.pool.get() {
        Ok(c) => c,
        Err(_) => return server_error(&state, "Database error").into_response(),
    };

    let total = query::count_versions(&conn, &slug, &id).unwrap_or(0);
    let versions: Vec<serde_json::Value> = query::list_versions(&conn, &slug, &id, Some(per_page), Some(offset))
        .unwrap_or_default()
        .into_iter()
        .map(version_to_json)
        .collect();

    let editor_locale = extract_editor_locale(&headers, &state.config.locale);
    let claims_ref = claims.as_ref().map(|Extension(c)| c);
    let data = ContextBuilder::new(&state, claims_ref)
        .locale_from_auth(&auth_user)
        .editor_locale(editor_locale.as_deref(), &state.config.locale)
        .page(PageType::CollectionVersions, format!("Version History — {}", doc_title))
        .set("page_title", serde_json::json!(format!("Version History — {}", doc_title)))
        .collection_def(&def)
        .document_stub(&id)
        .set("doc_title", serde_json::json!(doc_title))
        .set("versions", serde_json::json!(versions))
        .set("restore_url_prefix", serde_json::json!(format!("/admin/collections/{}/{}", slug, id)))
        .pagination(
            page, per_page, total,
            format!("/admin/collections/{}/{}/versions?page={}", slug, id, page - 1),
            format!("/admin/collections/{}/{}/versions?page={}", slug, id, page + 1),
        )
        .breadcrumbs(vec![
            Breadcrumb::link("Collections", "/admin/collections"),
            Breadcrumb::link(def.display_name(), format!("/admin/collections/{}", slug)),
            Breadcrumb::link(doc_title.clone(), format!("/admin/collections/{}/{}", slug, id)),
            Breadcrumb::current("Version History"),
        ])
        .build();

    let data = state.hook_runner.run_before_render(data);

    render_or_error(&state, "collections/versions", &data).into_response()
}

/// POST /admin/collections/{slug}/evaluate-conditions
/// Evaluates server-only display conditions with current form data.
/// Returns JSON: { "field_name": true/false, ... }
pub async fn evaluate_conditions(
    State(state): State<AdminState>,
    Path(_slug): Path<String>,
    axum::Json(req): axum::Json<EvaluateConditionsRequest>,
) -> impl IntoResponse {
    use crate::hooks::lifecycle::DisplayConditionResult;

    let form_data = serde_json::json!(req.form_data);
    let mut results = serde_json::Map::new();
    for (field_name, func_ref) in &req.conditions {
        let visible = match state.hook_runner.call_display_condition(func_ref, &form_data) {
            Some(DisplayConditionResult::Bool(b)) => b,
            Some(DisplayConditionResult::Table { visible, .. }) => visible,
            None => true, // error → show
        };
        results.insert(field_name.clone(), serde_json::json!(visible));
    }
    axum::Json(serde_json::Value::Object(results))
}

#[derive(serde::Deserialize)]
pub struct EvaluateConditionsRequest {
    pub form_data: HashMap<String, serde_json::Value>,
    pub conditions: HashMap<String, String>,
}
