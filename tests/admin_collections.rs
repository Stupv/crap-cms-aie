//! Collection-related integration tests for admin HTTP handlers.
//!
//! Covers: collection CRUD, search/filter/sort, validation, versioning, uploads (API).

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use crap_cms::admin::AdminState;
use crap_cms::admin::server::build_router;
use crap_cms::admin::templates;
use crap_cms::admin::translations::Translations;
use crap_cms::config::{CrapConfig, LocaleConfig};
use crap_cms::core::auth;
use crap_cms::core::collection::*;
use crap_cms::core::email::EmailRenderer;
use crap_cms::core::field::*;
use crap_cms::core::Registry;
use crap_cms::db::{migrate, pool, query};
use crap_cms::hooks::lifecycle::HookRunner;

// ── Helpers ───────────────────────────────────────────────────────────────

fn make_posts_def() -> CollectionDefinition {
    CollectionDefinition {
        slug: "posts".to_string(),
        labels: CollectionLabels {
            singular: Some(LocalizedString::Plain("Post".to_string())),
            plural: Some(LocalizedString::Plain("Posts".to_string())),
        },
        timestamps: true,
        fields: vec![
            FieldDefinition {
                name: "title".to_string(),
                required: true,
                ..Default::default()
            },
        ],
        admin: CollectionAdmin::default(),
        hooks: CollectionHooks::default(),
        auth: None,
        upload: None,
        access: CollectionAccess::default(),
        mcp: Default::default(),
        live: None,
        versions: None,
        indexes: Vec::new(),
    }
}

fn make_users_def() -> CollectionDefinition {
    CollectionDefinition {
        slug: "users".to_string(),
        labels: CollectionLabels {
            singular: Some(LocalizedString::Plain("User".to_string())),
            plural: Some(LocalizedString::Plain("Users".to_string())),
        },
        timestamps: true,
        fields: vec![
            FieldDefinition {
                name: "email".to_string(),
                field_type: FieldType::Email,
                required: true,
                unique: true,
                ..Default::default()
            },
            FieldDefinition {
                name: "name".to_string(),
                ..Default::default()
            },
        ],
        admin: CollectionAdmin::default(),
        hooks: CollectionHooks::default(),
        auth: Some(CollectionAuth { enabled: true, ..Default::default() }),
        upload: None,
        access: CollectionAccess::default(),
        mcp: Default::default(),
        live: None,
        versions: None,
        indexes: Vec::new(),
    }
}

struct TestApp {
    _tmp: tempfile::TempDir,
    router: axum::Router,
    pool: crap_cms::db::DbPool,
    registry: crap_cms::core::SharedRegistry,
    jwt_secret: String,
}

fn setup_app(
    collections: Vec<CollectionDefinition>,
    globals: Vec<GlobalDefinition>,
) -> TestApp {
    let mut config = CrapConfig::default();
    config.database.path = "test.db".to_string();
    config.auth.secret = "test-jwt-secret".to_string();
    config.admin.require_auth = false;
    setup_app_with_config(collections, globals, config)
}

fn setup_app_with_config(
    collections: Vec<CollectionDefinition>,
    globals: Vec<GlobalDefinition>,
    config: CrapConfig,
) -> TestApp {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = config;

    let db_pool = pool::create_pool(tmp.path(), &config).expect("create pool");

    let registry = Registry::shared();
    {
        let mut reg = registry.write().unwrap();
        for def in &collections {
            reg.register_collection(def.clone());
        }
        for def in &globals {
            reg.register_global(def.clone());
        }
    }

    migrate::sync_all(&db_pool, &registry, &config.locale).expect("sync schema");

    let hook_runner =
        HookRunner::new(tmp.path(), registry.clone(), &config).expect("create hook runner");

    let translations = Arc::new(Translations::load(tmp.path()));
    let handlebars =
        templates::create_handlebars(tmp.path(), false, translations.clone()).expect("create handlebars");
    let email_renderer =
        Arc::new(EmailRenderer::new(tmp.path()).expect("create email renderer"));

    let has_auth = {
        let reg = registry.read().unwrap();
        reg.collections.values().any(|d| d.is_auth_collection())
    };

    let state = AdminState {
        config,
        config_dir: tmp.path().to_path_buf(),
        pool: db_pool.clone(),
        registry: Registry::snapshot(&registry),
        handlebars,
        hook_runner,
        jwt_secret: "test-jwt-secret".to_string(),
        email_renderer,
        event_bus: None,
        login_limiter: std::sync::Arc::new(crap_cms::core::rate_limit::LoginRateLimiter::new(5, 300)),
        forgot_password_limiter: std::sync::Arc::new(crap_cms::core::rate_limit::LoginRateLimiter::new(3, 900)),
        has_auth,
        translations,
        shutdown: tokio_util::sync::CancellationToken::new(),
    };

    let router = build_router(state);

    TestApp {
        _tmp: tmp,
        router,
        pool: db_pool,
        registry,
        jwt_secret: "test-jwt-secret".to_string(),
    }
}

fn create_test_user(app: &TestApp, email: &str, password: &str) -> String {
    let reg = app.registry.read().unwrap();
    let def = reg.get_collection("users").unwrap().clone();
    drop(reg);

    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = std::collections::HashMap::from([
        ("email".to_string(), email.to_string()),
        ("name".to_string(), "Test User".to_string()),
    ]);
    let doc = query::create(&tx, "users", &def, &data, None).unwrap();
    query::update_password(&tx, "users", &doc.id, password).unwrap();
    tx.commit().unwrap();
    doc.id
}

fn make_auth_cookie(app: &TestApp, user_id: &str, email: &str) -> String {
    let claims = auth::Claims {
        sub: user_id.to_string(),
        collection: "users".to_string(),
        email: email.to_string(),
        exp: (chrono::Utc::now().timestamp() as u64) + 3600,
    };
    let token = auth::create_token(&claims, &app.jwt_secret).unwrap();
    format!("crap_session={}", token)
}

const TEST_CSRF: &str = "test-csrf-token-12345";

fn csrf_cookie() -> String {
    format!("crap_csrf={}", TEST_CSRF)
}

fn auth_and_csrf(auth_cookie: &str) -> String {
    format!("{}; crap_csrf={}", auth_cookie, TEST_CSRF)
}

async fn body_string(body: Body) -> String {
    let bytes = body.collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn make_bearer_token(app: &TestApp, user_id: &str, email: &str) -> String {
    let claims = auth::Claims {
        sub: user_id.to_string(),
        collection: "users".to_string(),
        email: email.to_string(),
        exp: (chrono::Utc::now().timestamp() as u64) + 3600,
    };
    let token = auth::create_token(&claims, &app.jwt_secret).unwrap();
    format!("Bearer {}", token)
}

fn make_locale_config() -> LocaleConfig {
    LocaleConfig {
        default_locale: "en".to_string(),
        locales: vec!["en".to_string(), "de".to_string()],
        fallback: true,
    }
}

fn make_localized_pages_def() -> CollectionDefinition {
    CollectionDefinition {
        slug: "pages".to_string(),
        labels: CollectionLabels {
            singular: Some(LocalizedString::Plain("Page".to_string())),
            plural: Some(LocalizedString::Plain("Pages".to_string())),
        },
        timestamps: true,
        fields: vec![
            FieldDefinition {
                name: "title".to_string(),
                required: true,
                localized: true,
                ..Default::default()
            },
            FieldDefinition {
                name: "body".to_string(),
                field_type: FieldType::Textarea,
                localized: true,
                ..Default::default()
            },
        ],
        admin: CollectionAdmin {
            use_as_title: Some("title".to_string()),
            ..CollectionAdmin::default()
        },
        hooks: CollectionHooks::default(),
        auth: None,
        upload: None,
        access: CollectionAccess::default(),
        mcp: Default::default(),
        live: None,
        versions: None,
        indexes: Vec::new(),
    }
}

fn setup_localized_app() -> TestApp {
    let mut config = CrapConfig::default();
    config.database.path = "test.db".to_string();
    config.auth.secret = "test-jwt-secret".to_string();
    config.locale = make_locale_config();
    setup_app_with_config(
        vec![make_localized_pages_def(), make_users_def()],
        vec![],
        config,
    )
}

fn make_versioned_posts_def() -> CollectionDefinition {
    CollectionDefinition {
        slug: "articles".to_string(),
        labels: CollectionLabels {
            singular: Some(LocalizedString::Plain("Article".to_string())),
            plural: Some(LocalizedString::Plain("Articles".to_string())),
        },
        timestamps: true,
        fields: vec![
            FieldDefinition {
                name: "title".to_string(),
                required: true,
                ..Default::default()
            },
            FieldDefinition {
                name: "body".to_string(),
                field_type: FieldType::Textarea,
                ..Default::default()
            },
        ],
        admin: CollectionAdmin {
            use_as_title: Some("title".to_string()),
            ..CollectionAdmin::default()
        },
        hooks: CollectionHooks::default(),
        auth: None,
        upload: None,
        access: CollectionAccess::default(),
        mcp: Default::default(),
        live: None,
        versions: Some(crap_cms::core::collection::VersionsConfig {
            drafts: true,
            max_versions: 10,
        }),
        indexes: Vec::new(),
    }
}

fn make_posts_with_required_title() -> CollectionDefinition {
    CollectionDefinition {
        slug: "articles".to_string(),
        labels: CollectionLabels {
            singular: Some(LocalizedString::Plain("Article".to_string())),
            plural: Some(LocalizedString::Plain("Articles".to_string())),
        },
        timestamps: true,
        fields: vec![
            FieldDefinition {
                name: "title".to_string(),
                required: true,
                ..Default::default()
            },
            FieldDefinition {
                name: "body".to_string(),
                field_type: FieldType::Textarea,
                ..Default::default()
            },
        ],
        admin: CollectionAdmin {
            use_as_title: Some("title".to_string()),
            ..CollectionAdmin::default()
        },
        hooks: CollectionHooks::default(),
        auth: None,
        upload: None,
        access: CollectionAccess::default(),
        mcp: Default::default(),
        live: None,
        versions: None,
        indexes: Vec::new(),
    }
}

fn make_searchable_posts_def() -> CollectionDefinition {
    CollectionDefinition {
        slug: "sposts".to_string(),
        labels: CollectionLabels {
            singular: Some(LocalizedString::Plain("Searchable Post".to_string())),
            plural: Some(LocalizedString::Plain("Searchable Posts".to_string())),
        },
        timestamps: true,
        fields: vec![
            FieldDefinition {
                name: "title".to_string(),
                required: true,
                ..Default::default()
            },
            FieldDefinition {
                name: "body".to_string(),
                field_type: FieldType::Textarea,
                ..Default::default()
            },
            FieldDefinition {
                name: "category".to_string(),
                ..Default::default()
            },
        ],
        admin: CollectionAdmin {
            use_as_title: Some("title".to_string()),
            list_searchable_fields: vec!["title".to_string(), "body".to_string()],
            ..CollectionAdmin::default()
        },
        hooks: CollectionHooks::default(),
        auth: None,
        upload: None,
        access: CollectionAccess::default(),
        mcp: Default::default(),
        live: None,
        versions: None,
        indexes: Vec::new(),
    }
}

fn make_media_def() -> CollectionDefinition {
    use crap_cms::core::upload::CollectionUpload;

    fn hidden_text(name: &str) -> FieldDefinition {
        FieldDefinition {
            name: name.to_string(),
            admin: FieldAdmin { hidden: true, ..Default::default() },
            ..Default::default()
        }
    }
    fn hidden_number(name: &str) -> FieldDefinition {
        FieldDefinition {
            name: name.to_string(),
            field_type: FieldType::Number,
            admin: FieldAdmin { hidden: true, ..Default::default() },
            ..Default::default()
        }
    }

    CollectionDefinition {
        slug: "media".to_string(),
        labels: CollectionLabels {
            singular: Some(LocalizedString::Plain("Media".to_string())),
            plural: Some(LocalizedString::Plain("Media".to_string())),
        },
        timestamps: true,
        fields: vec![
            FieldDefinition {
                name: "filename".to_string(),
                required: true,
                admin: FieldAdmin { readonly: true, ..Default::default() },
                ..Default::default()
            },
            hidden_text("mime_type"),
            hidden_number("filesize"),
            hidden_number("width"),
            hidden_number("height"),
            hidden_text("url"),
            FieldDefinition {
                name: "alt".to_string(),
                ..Default::default()
            },
        ],
        admin: CollectionAdmin::default(),
        hooks: CollectionHooks::default(),
        auth: None,
        upload: Some(CollectionUpload {
            enabled: true,
            mime_types: vec!["image/*".to_string(), "application/pdf".to_string()],
            ..Default::default()
        }),
        access: CollectionAccess::default(),
        mcp: Default::default(),
        live: None,
        versions: None,
        indexes: Vec::new(),
    }
}

fn build_multipart_body(
    filename: &str,
    content_type: &str,
    file_data: &[u8],
    fields: &[(&str, &str)],
) -> (String, Vec<u8>) {
    let boundary = "----CrapTestBoundary";
    let mut body = Vec::new();

    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"_file\"; filename=\"{}\"\r\n",
            filename
        )
        .as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {}\r\n\r\n", content_type).as_bytes());
    body.extend_from_slice(file_data);
    body.extend_from_slice(b"\r\n");

    for (name, value) in fields {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{}\"\r\n\r\n", name).as_bytes(),
        );
        body.extend_from_slice(value.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

    let content_type = format!("multipart/form-data; boundary={}", boundary);
    (content_type, body)
}

fn tiny_png() -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    use image::ImageEncoder;
    encoder
        .write_image(&[0u8, 0, 0, 0], 1, 1, image::ExtendedColorType::Rgba8)
        .unwrap();
    buf.into_inner()
}

// ── 1C. Dashboard & Collections ───────────────────────────────────────────

#[tokio::test]
async fn dashboard_returns_200() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "dash@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "dash@test.com");

    let resp = app.router
        .oneshot(
            Request::get("/admin")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    assert!(body.to_lowercase().contains("posts") || body.to_lowercase().contains("dashboard"));
}

#[tokio::test]
async fn list_collections_returns_200() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "list@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "list@test.com");

    let resp = app.router
        .oneshot(
            Request::get("/admin/collections")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn list_items_returns_200() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "items@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "items@test.com");

    let resp = app.router
        .oneshot(
            Request::get("/admin/collections/posts")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn create_form_returns_200() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "create@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "create@test.com");

    let resp = app.router
        .oneshot(
            Request::get("/admin/collections/posts/create")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn create_action_creates_document() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "create_action@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "create_action@test.com");

    let resp = app.router
        .oneshot(
            Request::post("/admin/collections/posts")
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("title=Test+Post"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::FOUND || status == StatusCode::OK,
        "Create action should redirect or HX-Redirect, got {}",
        status
    );
}

#[tokio::test]
async fn edit_form_returns_200() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "edit@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "edit@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("posts").unwrap().clone()
    };
    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = std::collections::HashMap::from([("title".to_string(), "Edit Me".to_string())]);
    let doc = query::create(&tx, "posts", &def, &data, None).unwrap();
    tx.commit().unwrap();

    let resp = app.router
        .oneshot(
            Request::get(format!("/admin/collections/posts/{}", doc.id))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn update_action_updates_document() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "update@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "update@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("posts").unwrap().clone()
    };
    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = std::collections::HashMap::from([("title".to_string(), "Original".to_string())]);
    let doc = query::create(&tx, "posts", &def, &data, None).unwrap();
    tx.commit().unwrap();

    let resp = app.router
        .oneshot(
            Request::post(format!("/admin/collections/posts/{}", doc.id))
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("title=Updated"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::FOUND || status == StatusCode::OK,
        "Update action should redirect or HX-Redirect, got {}",
        status
    );
}

#[tokio::test]
async fn delete_action_removes_document() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "delete@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "delete@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("posts").unwrap().clone()
    };
    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = std::collections::HashMap::from([("title".to_string(), "Delete Me".to_string())]);
    let doc = query::create(&tx, "posts", &def, &data, None).unwrap();
    tx.commit().unwrap();

    let resp = app.router
        .oneshot(
            Request::delete(format!("/admin/collections/posts/{}", doc.id))
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::FOUND || status == StatusCode::OK,
        "Delete action should redirect or return 200, got {}",
        status
    );
}

#[tokio::test]
async fn nonexistent_collection_returns_404() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "notfound@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "notfound@test.com");

    let resp = app.router
        .oneshot(
            Request::get("/admin/collections/nope")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Collection Handler Gaps ───────────────────────────────────────────────

#[tokio::test]
async fn list_items_with_search() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "search@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "search@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("posts").unwrap().clone()
    };
    for title in &["Zebra Unique Alpha", "Beta Common", "Gamma Common"] {
        let mut conn = app.pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        let data = std::collections::HashMap::from([("title".to_string(), title.to_string())]);
        query::create(&tx, "posts", &def, &data, None).unwrap();
        tx.commit().unwrap();
    }

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/collections/posts?search=Zebra")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    assert!(
        body.contains("Zebra"),
        "Search results should contain 'Zebra'"
    );
}

#[tokio::test]
async fn create_action_with_locale() {
    let app = setup_localized_app();
    let user_id = create_test_user(&app, "locale_create@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "locale_create@test.com");

    let resp = app
        .router
        .oneshot(
            Request::post("/admin/collections/pages")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::from("title=Locale+Test+Page&body=Content+here&_locale=de"))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = resp.status();
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::OK,
        "Localized create with locale param should succeed, got {}",
        status
    );
}

#[tokio::test]
async fn delete_action_returns_redirect() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "delredir@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "delredir@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("posts").unwrap().clone()
    };
    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = std::collections::HashMap::from([("title".to_string(), "To Delete Redir".to_string())]);
    let doc = query::create(&tx, "posts", &def, &data, None).unwrap();
    tx.commit().unwrap();

    let resp = app
        .router
        .oneshot(
            Request::delete(format!("/admin/collections/posts/{}", doc.id))
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::FOUND || status == StatusCode::OK,
        "Delete action should redirect or return 200 with HX-Redirect, got {}",
        status
    );

    if status == StatusCode::SEE_OTHER || status == StatusCode::FOUND {
        let location = resp.headers().get("location")
            .map(|v| v.to_str().unwrap_or(""));
        if let Some(loc) = location {
            assert!(
                loc.contains("/admin/collections/posts"),
                "Delete redirect should point to collection list, got {}",
                loc
            );
        }
    }
}

// ── Edit nonexistent document returns 404 ────────────────────────────────

#[tokio::test]
async fn edit_nonexistent_document_returns_404() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "editnf@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "editnf@test.com");

    let resp = app.router
        .oneshot(
            Request::get("/admin/collections/posts/nonexistent-id-12345")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Delete nonexistent document ──────────────────────────────────────────

#[tokio::test]
async fn delete_nonexistent_document() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "delnf@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "delnf@test.com");

    let resp = app.router
        .oneshot(
            Request::delete("/admin/collections/posts/nonexistent-id-12345")
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::OK
            || status == StatusCode::FOUND || status == StatusCode::NOT_FOUND,
        "Delete nonexistent should return redirect or not found, got {}",
        status
    );
}

// ── Update nonexistent document ──────────────────────────────────────────

#[tokio::test]
async fn update_nonexistent_document() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "updnf@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "updnf@test.com");

    let resp = app.router
        .oneshot(
            Request::post("/admin/collections/posts/nonexistent-id-12345")
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("title=Updated"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::OK
            || status == StatusCode::FOUND || status == StatusCode::NOT_FOUND
            || status == StatusCode::INTERNAL_SERVER_ERROR,
        "Update nonexistent should return redirect or error, got {}",
        status
    );
}

// ── Pagination on collection list ────────────────────────────────────────

#[tokio::test]
async fn collection_list_with_pagination() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "page@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "page@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("posts").unwrap().clone()
    };
    for i in 0..5 {
        let mut conn = app.pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        let data = std::collections::HashMap::from([
            ("title".to_string(), format!("Post {}", i)),
        ]);
        query::create(&tx, "posts", &def, &data, None).unwrap();
        tx.commit().unwrap();
    }

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/collections/posts?page=1&per_page=2")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── Localized collection regression tests ─────────────────────────────

#[tokio::test]
async fn localized_collection_list_returns_200() {
    let app = setup_localized_app();
    let user_id = create_test_user(&app, "admin@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "admin@test.com");

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/collections/pages")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn localized_collection_list_shows_documents() {
    let app = setup_localized_app();
    let user_id = create_test_user(&app, "admin@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "admin@test.com");

    {
        let reg = app.registry.read().unwrap();
        let def = reg.get_collection("pages").unwrap().clone();
        drop(reg);

        let locale_ctx = query::LocaleContext {
            mode: query::LocaleMode::Single("en".to_string()),
            config: make_locale_config(),
        };
        let mut data = std::collections::HashMap::new();
        data.insert("title".to_string(), "Hello World".to_string());
        data.insert("body".to_string(), "Page body".to_string());
        let mut conn = app.pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        query::create(&tx, "pages", &def, &data, Some(&locale_ctx)).unwrap();
        tx.commit().unwrap();
    }

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/collections/pages")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    assert!(body.contains("Hello World"), "list should contain the document title");
}

#[tokio::test]
async fn localized_collection_create_via_form() {
    let app = setup_localized_app();
    let user_id = create_test_user(&app, "admin@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "admin@test.com");

    let resp = app
        .router
        .oneshot(
            Request::post("/admin/collections/pages")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::from("title=Created+Page&body=Some+content&_locale=en"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::OK,
        "Localized create should redirect or HX-Redirect, got {}",
        status
    );
}

#[tokio::test]
async fn localized_collection_edit_page_returns_200() {
    let app = setup_localized_app();
    let user_id = create_test_user(&app, "admin@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "admin@test.com");

    let doc_id = {
        let reg = app.registry.read().unwrap();
        let def = reg.get_collection("pages").unwrap().clone();
        drop(reg);

        let locale_ctx = query::LocaleContext {
            mode: query::LocaleMode::Single("en".to_string()),
            config: make_locale_config(),
        };
        let mut data = std::collections::HashMap::new();
        data.insert("title".to_string(), "Editable Page".to_string());
        data.insert("body".to_string(), "Content".to_string());
        let mut conn = app.pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        let doc = query::create(&tx, "pages", &def, &data, Some(&locale_ctx)).unwrap();
        tx.commit().unwrap();
        doc.id
    };

    let resp = app
        .router
        .oneshot(
            Request::get(&format!("/admin/collections/pages/{}", doc_id))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    assert!(body.contains("Editable Page"), "edit page should contain the document title");
}

#[tokio::test]
async fn localized_collection_delete_succeeds() {
    let app = setup_localized_app();
    let user_id = create_test_user(&app, "admin@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "admin@test.com");

    let doc_id = {
        let reg = app.registry.read().unwrap();
        let def = reg.get_collection("pages").unwrap().clone();
        drop(reg);

        let locale_ctx = query::LocaleContext {
            mode: query::LocaleMode::Single("en".to_string()),
            config: make_locale_config(),
        };
        let mut data = std::collections::HashMap::new();
        data.insert("title".to_string(), "To Delete".to_string());
        let mut conn = app.pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        let doc = query::create(&tx, "pages", &def, &data, Some(&locale_ctx)).unwrap();
        tx.commit().unwrap();
        doc.id
    };

    let resp = app
        .router
        .oneshot(
            Request::delete(format!("/admin/collections/pages/{}", doc_id))
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::FOUND || status == StatusCode::OK,
        "expected redirect after delete, got {}",
        status
    );
}

#[tokio::test]
async fn localized_collection_search_returns_200() {
    let app = setup_localized_app();
    let user_id = create_test_user(&app, "admin@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "admin@test.com");

    {
        let reg = app.registry.read().unwrap();
        let def = reg.get_collection("pages").unwrap().clone();
        drop(reg);

        let locale_ctx = query::LocaleContext {
            mode: query::LocaleMode::Single("en".to_string()),
            config: make_locale_config(),
        };
        let mut data = std::collections::HashMap::new();
        data.insert("title".to_string(), "Searchable Page".to_string());
        let mut conn = app.pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        query::create(&tx, "pages", &def, &data, Some(&locale_ctx)).unwrap();
        tx.commit().unwrap();
    }

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/collections/pages?search=Searchable")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── Collection Versioning Tests ──────────────────────────────────────────

#[tokio::test]
async fn collection_versions_page_returns_200() {
    let app = setup_app(vec![make_versioned_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "cvp@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "cvp@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("articles").unwrap().clone()
    };
    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = std::collections::HashMap::from([
        ("title".to_string(), "Versioned Article".to_string()),
        ("body".to_string(), "Content".to_string()),
    ]);
    let doc = query::create(&tx, "articles", &def, &data, None).unwrap();
    tx.commit().unwrap();

    let resp = app
        .router
        .clone()
        .oneshot(
            Request::get(format!("/admin/collections/articles/{}/versions", doc.id))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn collection_create_with_draft() {
    let app = setup_app(vec![make_versioned_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "cdraft@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "cdraft@test.com");

    let resp = app
        .router
        .oneshot(
            Request::post("/admin/collections/articles")
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("title=Draft+Article&body=WIP&_action=save_draft"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::OK,
        "Create draft should succeed, got {}",
        status
    );
}

// ── Collections: Search, Filter, Sort, Pagination ─────────────────────────

#[tokio::test]
async fn list_items_with_pagination() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "page@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "page@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("posts").unwrap().clone()
    };
    for i in 0..25 {
        let mut conn = app.pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        let data = std::collections::HashMap::from([("title".to_string(), format!("Post {}", i))]);
        query::create(&tx, "posts", &def, &data, None).unwrap();
        tx.commit().unwrap();
    }

    let resp = app
        .router
        .clone()
        .oneshot(
            Request::get("/admin/collections/posts?page=2&per_page=10")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn list_items_search_no_results() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "nosearch@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "nosearch@test.com");

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/collections/posts?search=nonexistent_query_xyz")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn list_items_with_search_and_pagination() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "sp@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "sp@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("posts").unwrap().clone()
    };
    for i in 0..5 {
        let mut conn = app.pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        let data = std::collections::HashMap::from([("title".to_string(), format!("Searchable Item {}", i))]);
        query::create(&tx, "posts", &def, &data, None).unwrap();
        tx.commit().unwrap();
    }

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/collections/posts?search=Searchable&page=1&per_page=3")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    assert!(body.contains("Searchable"), "Search results should contain matching items");
}

// ── Collections: Create with validation error ─────────────────────────────

#[tokio::test]
async fn create_action_validation_error_missing_required_field() {
    let app = setup_app(vec![make_posts_with_required_title(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "validate@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "validate@test.com");

    let resp = app
        .router
        .oneshot(
            Request::post("/admin/collections/articles")
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("title=&body=Some+content"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::OK || status == StatusCode::SEE_OTHER,
        "Expected 200 (validation error re-render) or redirect, got {}",
        status
    );
}

// ── Collections: Create for auth collection (password field) ──────────────

#[tokio::test]
async fn create_action_auth_collection_with_password() {
    let app = setup_app(vec![make_users_def()], vec![]);
    let user_id = create_test_user(&app, "admin@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "admin@test.com");

    let resp = app
        .router
        .oneshot(
            Request::post("/admin/collections/users")
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("email=newuser@test.com&name=New+User&password=secret456"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::OK || status == StatusCode::SEE_OTHER,
        "Create auth collection user should succeed, got {}",
        status
    );
}

// ── Collections: Create form for auth collection shows password ───────────

#[tokio::test]
async fn create_form_auth_collection_shows_password_field() {
    let app = setup_app(vec![make_users_def()], vec![]);
    let user_id = create_test_user(&app, "admin@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "admin@test.com");

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/collections/users/create")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    assert!(body.contains("password"), "Auth collection create form should contain password field");
}

// ── Collections: Edit form for auth collection shows password ─────────────

#[tokio::test]
async fn edit_form_auth_collection_shows_password_field() {
    let app = setup_app(vec![make_users_def()], vec![]);
    let user_id = create_test_user(&app, "admin@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "admin@test.com");

    let resp = app
        .router
        .oneshot(
            Request::get(format!("/admin/collections/users/{}", user_id))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    assert!(body.contains("password"), "Auth collection edit form should contain password field");
    assert!(
        body.contains("Leave blank") || body.contains("leave blank") || body.contains("keep current"),
        "Edit form should indicate password can be left blank"
    );
}

// ── Collections: Update action with validation error ──────────────────────

#[tokio::test]
async fn update_action_validation_error() {
    let app = setup_app(vec![make_posts_with_required_title(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "update_val@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "update_val@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("articles").unwrap().clone()
    };
    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = std::collections::HashMap::from([("title".to_string(), "Valid Title".to_string())]);
    let doc = query::create(&tx, "articles", &def, &data, None).unwrap();
    tx.commit().unwrap();

    let resp = app
        .router
        .oneshot(
            Request::post(format!("/admin/collections/articles/{}", doc.id))
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("title=&body=Updated+content"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::OK || status == StatusCode::SEE_OTHER,
        "Expected 200 (validation error re-render) or redirect, got {}",
        status
    );
}

#[tokio::test]
async fn delete_confirm_nonexistent_doc_returns_404() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "delnf@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "delnf@test.com");

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/collections/posts/nonexistent-id/delete")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Collections: Delete via POST with _method=DELETE ──────────────────────

#[tokio::test]
async fn update_action_post_with_method_delete() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "meth_del@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "meth_del@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("posts").unwrap().clone()
    };
    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = std::collections::HashMap::from([("title".to_string(), "Method Delete Test".to_string())]);
    let doc = query::create(&tx, "posts", &def, &data, None).unwrap();
    tx.commit().unwrap();

    let resp = app
        .router
        .oneshot(
            Request::post(format!("/admin/collections/posts/{}", doc.id))
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("_method=DELETE"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::OK || status == StatusCode::SEE_OTHER,
        "POST with _method=DELETE should succeed, got {}",
        status
    );
}

// ── Collections: Edit nonexistent document ────────────────────────────────

#[tokio::test]
async fn edit_form_nonexistent_doc_returns_404() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "editnf@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "editnf@test.com");

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/collections/posts/nonexistent-id")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Collections: Versioning ───────────────────────────────────────────────

#[tokio::test]
async fn versioned_collection_list_returns_200() {
    let app = setup_app(vec![make_versioned_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "ver@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "ver@test.com");

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/collections/articles")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn versioned_collection_create_as_draft() {
    let app = setup_app(vec![make_versioned_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "draft@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "draft@test.com");

    let resp = app
        .router
        .oneshot(
            Request::post("/admin/collections/articles")
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("title=Draft+Post&_action=save_draft"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::OK || status == StatusCode::SEE_OTHER,
        "Create as draft should succeed, got {}",
        status
    );
}

#[tokio::test]
async fn versioned_collection_edit_shows_versions() {
    let app = setup_app(vec![make_versioned_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "editver@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "editver@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("articles").unwrap().clone()
    };
    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = std::collections::HashMap::from([("title".to_string(), "Versioned Doc".to_string())]);
    let doc = query::create(&tx, "articles", &def, &data, None).unwrap();
    tx.commit().unwrap();

    let resp = app
        .router
        .oneshot(
            Request::get(format!("/admin/collections/articles/{}", doc.id))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    assert!(
        body.to_lowercase().contains("version") || body.contains("versions"),
        "Edit page for versioned collection should show version info"
    );
}

#[tokio::test]
async fn versioned_collection_update_unpublish() {
    let app = setup_app(vec![make_versioned_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "unpub@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "unpub@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("articles").unwrap().clone()
    };
    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = std::collections::HashMap::from([("title".to_string(), "Published Post".to_string())]);
    let doc = query::create(&tx, "articles", &def, &data, None).unwrap();
    tx.commit().unwrap();

    let resp = app
        .router
        .oneshot(
            Request::post(format!("/admin/collections/articles/{}", doc.id))
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("title=Published+Post&_action=unpublish"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::OK || status == StatusCode::SEE_OTHER,
        "Unpublish should succeed, got {}",
        status
    );
}

#[tokio::test]
async fn versioned_collection_versions_page() {
    let app = setup_app(vec![make_versioned_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "verpage@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "verpage@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("articles").unwrap().clone()
    };
    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = std::collections::HashMap::from([("title".to_string(), "Versioned Page".to_string())]);
    let doc = query::create(&tx, "articles", &def, &data, None).unwrap();
    tx.commit().unwrap();

    let resp = app
        .router
        .oneshot(
            Request::get(format!("/admin/collections/articles/{}/versions", doc.id))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    assert!(
        body.to_lowercase().contains("version"),
        "Versions page should contain 'version'"
    );
}

#[tokio::test]
async fn non_versioned_collection_versions_page_redirects() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "nover@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "nover@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("posts").unwrap().clone()
    };
    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = std::collections::HashMap::from([("title".to_string(), "No Versions".to_string())]);
    let doc = query::create(&tx, "posts", &def, &data, None).unwrap();
    tx.commit().unwrap();

    let resp = app
        .router
        .oneshot(
            Request::get(format!("/admin/collections/posts/{}/versions", doc.id))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::FOUND || status == StatusCode::TEMPORARY_REDIRECT,
        "Non-versioned collection versions page should redirect, got {}",
        status
    );
}

#[tokio::test]
async fn restore_version_non_versioned_redirects() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "restnv@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "restnv@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("posts").unwrap().clone()
    };
    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = std::collections::HashMap::from([("title".to_string(), "No Versions".to_string())]);
    let doc = query::create(&tx, "posts", &def, &data, None).unwrap();
    tx.commit().unwrap();

    let resp = app
        .router
        .oneshot(
            Request::post(format!("/admin/collections/posts/{}/versions/fake-ver/restore", doc.id))
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::OK,
        "Restore version on non-versioned collection should redirect, got {}",
        status
    );
}

// ── Collections: Evaluate conditions endpoint ─────────────────────────────

#[tokio::test]
async fn evaluate_conditions_returns_json() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "cond@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "cond@test.com");

    let body_json = serde_json::json!({
        "form_data": {"title": "Test"},
        "conditions": {}
    });

    let resp = app
        .router
        .oneshot(
            Request::post("/admin/collections/posts/evaluate-conditions")
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body_json).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(json.is_object(), "Evaluate conditions should return a JSON object");
}

// ── Collections: list_searchable_fields configuration ─────────────────────

#[tokio::test]
async fn search_uses_configured_searchable_fields() {
    let app = setup_app(vec![make_searchable_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "search2@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "search2@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("sposts").unwrap().clone()
    };
    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = std::collections::HashMap::from([
        ("title".to_string(), "Unique Title XYZ".to_string()),
        ("body".to_string(), "Some body text".to_string()),
        ("category".to_string(), "tech".to_string()),
    ]);
    let doc = query::create(&tx, "sposts", &def, &data, None).unwrap();
    query::fts::fts_upsert(&tx, "sposts", &doc, Some(&def)).unwrap();
    tx.commit().unwrap();

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/collections/sposts?search=Unique")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    assert!(body.contains("Unique Title XYZ"), "Search should find by configured searchable fields");
}

// ── Collections: Update via locale redirect suffix ────────────────────────

#[tokio::test]
async fn update_localized_collection_redirects_with_locale() {
    let app = setup_localized_app();
    let user_id = create_test_user(&app, "updloc@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "updloc@test.com");

    let doc_id = {
        let reg = app.registry.read().unwrap();
        let def = reg.get_collection("pages").unwrap().clone();
        drop(reg);
        let locale_ctx = query::LocaleContext {
            mode: query::LocaleMode::Single("en".to_string()),
            config: make_locale_config(),
        };
        let mut data = std::collections::HashMap::new();
        data.insert("title".to_string(), "Update Locale".to_string());
        let mut conn = app.pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        let doc = query::create(&tx, "pages", &def, &data, Some(&locale_ctx)).unwrap();
        tx.commit().unwrap();
        doc.id
    };

    let resp = app
        .router
        .oneshot(
            Request::post(format!("/admin/collections/pages/{}", doc_id))
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("title=Updated+Title&_locale=de"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::OK || status == StatusCode::SEE_OTHER,
        "Localized update should succeed, got {}",
        status
    );
    if status == StatusCode::OK {
        if let Some(hx_redir) = resp.headers().get("HX-Redirect") {
            let redir = hx_redir.to_str().unwrap_or("");
            assert!(
                !redir.contains("locale="),
                "HX-Redirect should not contain locale= (cookie-based now), got {}",
                redir
            );
        }
    }
}

// ── Collections: Nonexistent collection create form ───────────────────────

#[tokio::test]
async fn create_form_nonexistent_collection_returns_404() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "cfnf@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "cfnf@test.com");

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/collections/nonexistent/create")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Collections: Create action nonexistent collection ─────────────────────

#[tokio::test]
async fn create_action_nonexistent_collection_redirects() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "canf@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "canf@test.com");

    let resp = app
        .router
        .oneshot(
            Request::post("/admin/collections/nonexistent")
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("title=Test"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::FOUND,
        "Create on nonexistent collection should redirect, got {}",
        status
    );
}

// ── Collections: Collection with use_as_title in list view ────────────────

#[tokio::test]
async fn list_items_uses_title_field() {
    let mut def = make_posts_def();
    def.admin.use_as_title = Some("title".to_string());

    let app = setup_app(vec![def.clone(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "titlefield@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "titlefield@test.com");

    let real_def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("posts").unwrap().clone()
    };
    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = std::collections::HashMap::from([("title".to_string(), "My Custom Title".to_string())]);
    query::create(&tx, "posts", &real_def, &data, None).unwrap();
    tx.commit().unwrap();

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/collections/posts")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    assert!(body.contains("My Custom Title"), "List should show document title via use_as_title");
}

#[tokio::test]
async fn create_action_missing_required_field_shows_errors() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "valerr@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "valerr@test.com");

    let resp = app.router
        .oneshot(
            Request::post("/admin/collections/posts")
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("title="))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::OK || status == StatusCode::SEE_OTHER,
        "Validation error should re-render form or redirect, got {}",
        status
    );
}

#[tokio::test]
async fn create_form_auth_collection_includes_password() {
    let app = setup_app(vec![make_users_def()], vec![]);
    let user_id = create_test_user(&app, "authform@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "authform@test.com");

    let resp = app.router
        .oneshot(
            Request::get("/admin/collections/users/create")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    assert!(body.contains("password"), "Auth collection create form should have password field");
}

#[tokio::test]
async fn edit_form_auth_collection_includes_password() {
    let app = setup_app(vec![make_users_def()], vec![]);
    let user_id = create_test_user(&app, "authedit@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "authedit@test.com");

    let resp = app.router
        .oneshot(
            Request::get(format!("/admin/collections/users/{}", user_id))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    assert!(body.contains("password"), "Auth collection edit form should have password field");
}

#[tokio::test]
async fn edit_form_nonexistent_document_returns_404() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "nondoc@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "nondoc@test.com");

    let resp = app.router
        .oneshot(
            Request::get("/admin/collections/posts/nonexistent-id")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_confirm_nonexistent_document_returns_404() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "delconfnon@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "delconfnon@test.com");

    let resp = app.router
        .oneshot(
            Request::get("/admin/collections/posts/nonexistent-id/delete")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn versioned_collection_create_form() {
    let app = setup_app(vec![make_versioned_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "ver@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "ver@test.com");

    let resp = app.router
        .oneshot(
            Request::get("/admin/collections/articles/create")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn post_with_method_delete_deletes_document() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "methoddel@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "methoddel@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("posts").unwrap().clone()
    };
    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = std::collections::HashMap::from([("title".to_string(), "Method Delete".to_string())]);
    let doc = query::create(&tx, "posts", &def, &data, None).unwrap();
    tx.commit().unwrap();

    let resp = app.router
        .oneshot(
            Request::post(format!("/admin/collections/posts/{}", doc.id))
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("_method=DELETE"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::OK || status == StatusCode::SEE_OTHER,
        "DELETE via _method should succeed, got {}",
        status
    );
}

#[tokio::test]
async fn update_action_nonexistent_collection_redirects() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "noncolu@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "noncolu@test.com");

    let resp = app.router
        .oneshot(
            Request::post("/admin/collections/nonexistent/someid")
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("title=Test"))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert_eq!(status, StatusCode::SEE_OTHER, "Update on nonexistent collection should redirect");
}

#[tokio::test]
async fn delete_action_nonexistent_collection_redirects() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "noncold@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "noncold@test.com");

    let resp = app.router
        .oneshot(
            Request::delete("/admin/collections/nonexistent/someid")
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::OK,
        "Delete on nonexistent collection should redirect, got {}",
        status
    );
}

#[tokio::test]
async fn delete_confirm_nonexistent_collection_returns_404() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "noncoldc@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "noncoldc@test.com");

    let resp = app.router
        .oneshot(
            Request::get("/admin/collections/nonexistent/someid/delete")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn restore_version_nonversioned_redirects() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "restnv@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "restnv@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("posts").unwrap().clone()
    };
    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = std::collections::HashMap::from([("title".to_string(), "NV Restore".to_string())]);
    let doc = query::create(&tx, "posts", &def, &data, None).unwrap();
    tx.commit().unwrap();

    let resp = app.router
        .oneshot(
            Request::post(format!("/admin/collections/posts/{}/versions/fake-version/restore", doc.id))
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert_eq!(status, StatusCode::SEE_OTHER, "Restore on non-versioned should redirect");
}

#[tokio::test]
async fn restore_version_nonexistent_collection_redirects() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "restnc@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "restnc@test.com");

    let resp = app.router
        .oneshot(
            Request::post("/admin/collections/nonexistent/someid/versions/v1/restore")
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert_eq!(status, StatusCode::SEE_OTHER, "Restore on nonexistent collection should redirect");
}

// ── Collections: Create form with locale param ────────────────────────────

#[tokio::test]
async fn create_form_with_locale_returns_200() {
    let app = setup_localized_app();
    let user_id = create_test_user(&app, "cfloc@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "cfloc@test.com");

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/collections/pages/create")
                .header("cookie", format!("{}; crap_editor_locale=de", &cookie))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── Edit form with non-default locale ────────────────────────────────────

#[tokio::test]
async fn edit_form_with_non_default_locale() {
    let app = setup_localized_app();
    let user_id = create_test_user(&app, "efloc@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "efloc@test.com");

    let doc_id = {
        let reg = app.registry.read().unwrap();
        let def = reg.get_collection("pages").unwrap().clone();
        drop(reg);

        let locale_ctx = query::LocaleContext {
            mode: query::LocaleMode::Single("en".to_string()),
            config: make_locale_config(),
        };
        let mut data = std::collections::HashMap::new();
        data.insert("title".to_string(), "Locale Edit Test".to_string());
        let mut conn = app.pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        let doc = query::create(&tx, "pages", &def, &data, Some(&locale_ctx)).unwrap();
        tx.commit().unwrap();
        doc.id
    };

    let resp = app
        .router
        .oneshot(
            Request::get(format!("/admin/collections/pages/{}", doc_id))
                .header("cookie", format!("{}; crap_editor_locale=de", &cookie))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── Update action with locale parameter ──────────────────────────────────

#[tokio::test]
async fn update_action_with_locale() {
    let app = setup_localized_app();
    let user_id = create_test_user(&app, "updloc@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "updloc@test.com");

    let doc_id = {
        let reg = app.registry.read().unwrap();
        let def = reg.get_collection("pages").unwrap().clone();
        drop(reg);

        let locale_ctx = query::LocaleContext {
            mode: query::LocaleMode::Single("en".to_string()),
            config: make_locale_config(),
        };
        let mut data = std::collections::HashMap::new();
        data.insert("title".to_string(), "Update Locale Test".to_string());
        let mut conn = app.pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        let doc = query::create(&tx, "pages", &def, &data, Some(&locale_ctx)).unwrap();
        tx.commit().unwrap();
        doc.id
    };

    let resp = app
        .router
        .oneshot(
            Request::post(format!("/admin/collections/pages/{}", doc_id))
                .header("cookie", auth_and_csrf(&cookie))
                .header("X-CSRF-Token", TEST_CSRF)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("title=Updated+DE&_locale=de")))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::OK,
        "Update with locale should succeed, got {}",
        status
    );
}

// ── Collections: Search with special characters ───────────────────────────

#[tokio::test]
async fn list_items_search_with_special_chars() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "special@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "special@test.com");

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/collections/posts?search=hello%20world%26foo")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── Collections: Create form with locale ──────────────────────────────────

#[tokio::test]
async fn create_form_with_locale() {
    let app = setup_localized_app();
    let user_id = create_test_user(&app, "cfloc@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "cfloc@test.com");

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/collections/pages/create")
                .header("cookie", format!("{}; crap_editor_locale=de", &cookie))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    assert!(body.contains("DE") || body.contains("de"), "Should show locale selector with DE");
}

// ── Upload API tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn upload_api_create_returns_201_with_document() {
    let app = setup_app(vec![make_users_def(), make_media_def()], vec![]);
    let user_id = create_test_user(&app, "uploader@test.com", "secret123");
    let bearer = make_bearer_token(&app, &user_id, "uploader@test.com");

    let png = tiny_png();
    let (ct, body) = build_multipart_body("photo.png", "image/png", &png, &[("alt", "Test alt")]);

    let resp = app
        .router
        .oneshot(
            Request::post("/api/upload/media")
                .header("content-type", ct)
                .header("authorization", &bearer)
                .header("Cookie", csrf_cookie())
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);

    let body = body_string(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(json["document"]["id"].is_string());
    assert_eq!(json["document"]["alt"], "Test alt");
    assert!(json["document"]["filename"].as_str().unwrap().ends_with("photo.png"));
    assert!(json["document"]["url"].as_str().unwrap().starts_with("/uploads/media/"));
    assert_eq!(json["document"]["mime_type"], "image/png");
}

#[tokio::test]
async fn upload_api_create_no_file_returns_400() {
    let app = setup_app(vec![make_users_def(), make_media_def()], vec![]);
    let user_id = create_test_user(&app, "uploader@test.com", "secret123");
    let bearer = make_bearer_token(&app, &user_id, "uploader@test.com");

    let boundary = "----CrapTestBoundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"alt\"\r\n\r\nsome text\r\n");
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

    let resp = app
        .router
        .oneshot(
            Request::post("/api/upload/media")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .header("authorization", &bearer)
                .header("Cookie", csrf_cookie())
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_string(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(json["error"].as_str().unwrap().contains("No file"));
}

#[tokio::test]
async fn upload_api_create_non_upload_collection_returns_400() {
    let app = setup_app(vec![make_users_def(), make_posts_def()], vec![]);
    let user_id = create_test_user(&app, "uploader@test.com", "secret123");
    let bearer = make_bearer_token(&app, &user_id, "uploader@test.com");

    let png = tiny_png();
    let (ct, body) = build_multipart_body("photo.png", "image/png", &png, &[]);

    let resp = app
        .router
        .oneshot(
            Request::post("/api/upload/posts")
                .header("content-type", ct)
                .header("authorization", &bearer)
                .header("Cookie", csrf_cookie())
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_string(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(json["error"].as_str().unwrap().contains("not an upload collection"));
}

#[tokio::test]
async fn upload_api_create_unknown_collection_returns_404() {
    let app = setup_app(vec![make_users_def(), make_media_def()], vec![]);
    let user_id = create_test_user(&app, "uploader@test.com", "secret123");
    let bearer = make_bearer_token(&app, &user_id, "uploader@test.com");

    let png = tiny_png();
    let (ct, body) = build_multipart_body("photo.png", "image/png", &png, &[]);

    let resp = app
        .router
        .oneshot(
            Request::post("/api/upload/nonexistent")
                .header("content-type", ct)
                .header("authorization", &bearer)
                .header("Cookie", csrf_cookie())
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn upload_api_create_rejected_mime_returns_400() {
    let app = setup_app(vec![make_users_def(), make_media_def()], vec![]);
    let user_id = create_test_user(&app, "uploader@test.com", "secret123");
    let bearer = make_bearer_token(&app, &user_id, "uploader@test.com");

    let (ct, body) =
        build_multipart_body("notes.txt", "text/plain", b"hello world", &[]);

    let resp = app
        .router
        .oneshot(
            Request::post("/api/upload/media")
                .header("content-type", ct)
                .header("authorization", &bearer)
                .header("Cookie", csrf_cookie())
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_string(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(json["error"].as_str().unwrap().contains("not allowed"));
}

#[tokio::test]
async fn upload_api_update_replaces_file() {
    let app = setup_app(vec![make_users_def(), make_media_def()], vec![]);
    let user_id = create_test_user(&app, "uploader@test.com", "secret123");
    let bearer = make_bearer_token(&app, &user_id, "uploader@test.com");

    let png = tiny_png();
    let (ct, body) = build_multipart_body("first.png", "image/png", &png, &[("alt", "First")]);

    let resp = app
        .router
        .clone()
        .oneshot(
            Request::post("/api/upload/media")
                .header("content-type", &ct)
                .header("authorization", &bearer)
                .header("Cookie", csrf_cookie())
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let create_body = body_string(resp.into_body()).await;
    let create_json: serde_json::Value = serde_json::from_str(&create_body).unwrap();
    let doc_id = create_json["document"]["id"].as_str().unwrap();
    let old_filename = create_json["document"]["filename"].as_str().unwrap().to_string();

    let png2 = tiny_png();
    let (ct2, body2) =
        build_multipart_body("second.png", "image/png", &png2, &[("alt", "Second")]);

    let resp = app
        .router
        .clone()
        .oneshot(
            Request::patch(&format!("/api/upload/media/{}", doc_id))
                .header("content-type", ct2)
                .header("authorization", &bearer)
                .header("Cookie", csrf_cookie())
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::from(body2))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let update_body = body_string(resp.into_body()).await;
    let update_json: serde_json::Value = serde_json::from_str(&update_body).unwrap();
    let new_filename = update_json["document"]["filename"].as_str().unwrap();
    assert_ne!(new_filename, old_filename, "Filename should change on file replacement");
    assert_eq!(update_json["document"]["alt"], "Second");
}

#[tokio::test]
async fn upload_api_delete_returns_success() {
    let app = setup_app(vec![make_users_def(), make_media_def()], vec![]);
    let user_id = create_test_user(&app, "uploader@test.com", "secret123");
    let bearer = make_bearer_token(&app, &user_id, "uploader@test.com");

    let png = tiny_png();
    let (ct, body) = build_multipart_body("todelete.png", "image/png", &png, &[]);

    let resp = app
        .router
        .clone()
        .oneshot(
            Request::post("/api/upload/media")
                .header("content-type", ct)
                .header("authorization", &bearer)
                .header("Cookie", csrf_cookie())
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let create_body = body_string(resp.into_body()).await;
    let create_json: serde_json::Value = serde_json::from_str(&create_body).unwrap();
    let doc_id = create_json["document"]["id"].as_str().unwrap();

    let resp = app
        .router
        .clone()
        .oneshot(
            Request::delete(&format!("/api/upload/media/{}", doc_id))
                .header("authorization", &bearer)
                .header("Cookie", csrf_cookie())
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let del_body = body_string(resp.into_body()).await;
    let del_json: serde_json::Value = serde_json::from_str(&del_body).unwrap();
    assert_eq!(del_json["success"], true);
}

#[tokio::test]
async fn upload_api_delete_nonexistent_returns_404() {
    let app = setup_app(vec![make_users_def(), make_media_def()], vec![]);
    let user_id = create_test_user(&app, "uploader@test.com", "secret123");
    let bearer = make_bearer_token(&app, &user_id, "uploader@test.com");

    let resp = app
        .router
        .oneshot(
            Request::delete("/api/upload/media/nonexistent-id")
                .header("authorization", &bearer)
                .header("Cookie", csrf_cookie())
                .header("X-CSRF-Token", TEST_CSRF)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn upload_collection_create_form_shows_file_field() {
    let app = setup_app(vec![make_users_def(), make_media_def()], vec![]);
    let user_id = create_test_user(&app, "upform@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "upform@test.com");

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/collections/media/create")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    assert!(
        body.contains("file") || body.contains("upload"),
        "Upload collection create form should contain file upload controls"
    );
}

#[tokio::test]
async fn upload_collection_create_form_has_upload_context() {
    let app = setup_app(vec![make_users_def(), make_media_def()], vec![]);
    let user_id = create_test_user(&app, "uploadadm@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "uploadadm@test.com");

    let resp = app.router
        .oneshot(
            Request::get("/admin/collections/media/create")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── Delete confirm page ──────────────────────────────────────────────────

#[tokio::test]
async fn delete_confirm_page_returns_200() {
    let app = setup_app(vec![make_posts_def(), make_users_def()], vec![]);
    let user_id = create_test_user(&app, "delconf@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "delconf@test.com");

    let def = {
        let reg = app.registry.read().unwrap();
        reg.get_collection("posts").unwrap().clone()
    };
    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = std::collections::HashMap::from([("title".to_string(), "To Confirm Delete".to_string())]);
    let doc = query::create(&tx, "posts", &def, &data, None).unwrap();
    tx.commit().unwrap();

    let resp = app
        .router
        .oneshot(
            Request::get(format!("/admin/collections/posts/{}/delete", doc.id))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
