//! Shared service layer for collection/global CRUD operations.
//!
//! These synchronous functions encapsulate the transaction lifecycle (open tx → run hooks →
//! DB operation → commit) shared between admin handlers and the gRPC service. They are meant
//! to be called from within `spawn_blocking`.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context as _, Result};

use crate::config::{EmailConfig, ServerConfig};
use crate::core::collection::{CollectionHooks, GlobalDefinition};
use crate::core::document::Document;
use crate::core::email::EmailRenderer;
use crate::core::CollectionDefinition;
use crate::db::query::{self, LocaleContext};
use crate::db::DbPool;
use crate::hooks::lifecycle::{self, HookContext, HookEvent, HookRunner};

/// Create a document within a single transaction: before-hooks → insert → join data → password.
#[allow(clippy::too_many_arguments)]
pub fn create_document(
    pool: &DbPool,
    runner: &HookRunner,
    slug: &str,
    def: &CollectionDefinition,
    data: HashMap<String, String>,
    join_data: &HashMap<String, serde_json::Value>,
    password: Option<&str>,
    locale_ctx: Option<&LocaleContext>,
    locale: Option<String>,
    user: Option<&Document>,
) -> Result<Document> {
    let mut conn = pool.get().context("DB connection")?;
    let tx = conn.transaction().context("Start transaction")?;

    let hook_ctx = HookContext {
        collection: slug.to_string(),
        operation: "create".to_string(),
        data: data.iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect(),
        locale,
    };
    let final_ctx = runner.run_before_write(
        &def.hooks, &def.fields, hook_ctx, &tx, slug, None, user,
    )?;
    let final_data = lifecycle::hook_ctx_to_string_map(&final_ctx);
    let doc = query::create(&tx, slug, def, &final_data, locale_ctx)?;

    query::save_join_table_data(&tx, slug, def, &doc.id, join_data)?;

    if let Some(pw) = password {
        if !pw.is_empty() {
            query::update_password(&tx, slug, &doc.id, pw)?;
        }
    }

    tx.commit().context("Commit transaction")?;
    Ok(doc)
}

/// Update a document within a single transaction: before-hooks → update → join data → password.
#[allow(clippy::too_many_arguments)]
pub fn update_document(
    pool: &DbPool,
    runner: &HookRunner,
    slug: &str,
    id: &str,
    def: &CollectionDefinition,
    data: HashMap<String, String>,
    join_data: &HashMap<String, serde_json::Value>,
    password: Option<&str>,
    locale_ctx: Option<&LocaleContext>,
    locale: Option<String>,
    user: Option<&Document>,
) -> Result<Document> {
    let mut conn = pool.get().context("DB connection")?;
    let tx = conn.transaction().context("Start transaction")?;

    let hook_ctx = HookContext {
        collection: slug.to_string(),
        operation: "update".to_string(),
        data: data.iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect(),
        locale,
    };
    let final_ctx = runner.run_before_write(
        &def.hooks, &def.fields, hook_ctx, &tx, slug, Some(id), user,
    )?;
    let final_data = lifecycle::hook_ctx_to_string_map(&final_ctx);
    let doc = query::update(&tx, slug, def, id, &final_data, locale_ctx)?;

    query::save_join_table_data(&tx, slug, def, &doc.id, join_data)?;

    if let Some(pw) = password {
        if !pw.is_empty() {
            query::update_password(&tx, slug, &doc.id, pw)?;
        }
    }

    tx.commit().context("Commit transaction")?;
    Ok(doc)
}

/// Delete a document within a single transaction: before-hooks → delete.
pub fn delete_document(
    pool: &DbPool,
    runner: &HookRunner,
    slug: &str,
    id: &str,
    hooks: &CollectionHooks,
    user: Option<&Document>,
) -> Result<()> {
    let mut conn = pool.get().context("DB connection")?;
    let tx = conn.transaction().context("Start transaction")?;

    let hook_ctx = HookContext {
        collection: slug.to_string(),
        operation: "delete".to_string(),
        data: [("id".to_string(), serde_json::Value::String(id.to_string()))].into(),
        locale: None,
    };
    runner.run_hooks_with_conn(hooks, HookEvent::BeforeDelete, hook_ctx, &tx, user)?;
    query::delete(&tx, slug, id)?;

    tx.commit().context("Commit transaction")?;
    Ok(())
}

/// Update a global document within a single transaction: before-hooks → update.
#[allow(clippy::too_many_arguments)]
pub fn update_global_document(
    pool: &DbPool,
    runner: &HookRunner,
    slug: &str,
    def: &GlobalDefinition,
    data: HashMap<String, String>,
    locale_ctx: Option<&LocaleContext>,
    locale: Option<String>,
    user: Option<&Document>,
) -> Result<Document> {
    let mut conn = pool.get().context("DB connection")?;
    let tx = conn.transaction().context("Start transaction")?;

    let hook_ctx = HookContext {
        collection: slug.to_string(),
        operation: "update".to_string(),
        data: data.iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect(),
        locale,
    };
    let global_table = format!("_global_{}", slug);
    let final_ctx = runner.run_before_write(
        &def.hooks, &def.fields, hook_ctx, &tx, &global_table, Some("default"), user,
    )?;
    let final_data = lifecycle::hook_ctx_to_string_map(&final_ctx);
    let doc = query::update_global(&tx, slug, def, &final_data, locale_ctx)?;

    tx.commit().context("Commit transaction")?;
    Ok(doc)
}

/// Fire-and-forget: generate a verification token and send the verification email.
/// Spawns its own `spawn_blocking` task internally.
pub fn send_verification_email(
    pool: DbPool,
    email_config: EmailConfig,
    email_renderer: Arc<EmailRenderer>,
    server_config: ServerConfig,
    slug: String,
    user_id: String,
    user_email: String,
) {
    tokio::task::spawn_blocking(move || {
        if !crate::core::email::is_configured(&email_config) {
            tracing::warn!("Email not configured — skipping verification email for {}", user_email);
            return;
        }

        let token = nanoid::nanoid!(32);

        let conn = match pool.get() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("DB connection for verification token: {}", e);
                return;
            }
        };
        if let Err(e) = query::set_verification_token(&conn, &slug, &user_id, &token) {
            tracing::error!("Failed to set verification token: {}", e);
            return;
        }

        let verify_url = format!(
            "http://{}:{}/admin/verify-email?token={}",
            server_config.host, server_config.admin_port, token
        );
        let data = serde_json::json!({ "verify_url": verify_url });
        let html = match email_renderer.render("verify_email", &data) {
            Ok(h) => h,
            Err(e) => {
                tracing::error!("Failed to render verify email template: {}", e);
                return;
            }
        };

        if let Err(e) = crate::core::email::send_email(
            &email_config, &user_email, "Verify your email", &html, None,
        ) {
            tracing::error!("Failed to send verification email: {}", e);
        }
    });
}
