//! Localization, drafts, versions, complex globals, has-many relationships,
//! bulk operations, count, FTS search, and jobs RPC tests.
//!
//! Uses ContentService directly (no network) via ContentApi trait.

use std::collections::BTreeMap;
use std::sync::Arc;

use prost_types::{value::Kind, ListValue, Struct, Value};
use tonic::Request;

use crap_cms::api::content;
use crap_cms::api::content::content_api_server::ContentApi;
use crap_cms::api::service::ContentService;
use crap_cms::config::*;
use crap_cms::core::collection::*;
use crap_cms::core::email::EmailRenderer;
use crap_cms::core::field::*;
use crap_cms::core::Registry;
use crap_cms::db::{migrate, pool};
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
            FieldDefinition {
                name: "status".to_string(),
                field_type: FieldType::Select,
                default_value: Some(serde_json::json!("draft")),
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

/// Build a prost Struct from key-value string pairs.
fn make_struct(pairs: &[(&str, &str)]) -> Struct {
    let mut fields = BTreeMap::new();
    for (k, v) in pairs {
        fields.insert(
            k.to_string(),
            Value {
                kind: Some(Kind::StringValue(v.to_string())),
            },
        );
    }
    Struct { fields }
}

/// Extract a string field from a proto Document's fields struct.
fn get_proto_field(doc: &content::Document, field: &str) -> Option<String> {
    doc.fields.as_ref().and_then(|s| {
        s.fields.get(field).and_then(|v| match &v.kind {
            Some(Kind::StringValue(s)) => Some(s.clone()),
            _ => None,
        })
    })
}

fn str_val(s: &str) -> Value {
    Value { kind: Some(Kind::StringValue(s.to_string())) }
}

fn struct_val(pairs: &[(&str, Value)]) -> Value {
    let mut fields = BTreeMap::new();
    for (k, v) in pairs {
        fields.insert(k.to_string(), v.clone());
    }
    Value {
        kind: Some(Kind::StructValue(Struct { fields })),
    }
}

fn list_val(items: Vec<Value>) -> Value {
    Value {
        kind: Some(Kind::ListValue(ListValue { values: items })),
    }
}

struct TestSetup {
    _tmp: tempfile::TempDir,
    service: ContentService,
    #[allow(dead_code)]
    pool: crap_cms::db::DbPool,
}

fn setup_service(
    collections: Vec<CollectionDefinition>,
    globals: Vec<GlobalDefinition>,
) -> TestSetup {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut config = CrapConfig::default();
    config.database.path = "test.db".to_string();
    config.auth.secret = "test-jwt-secret".to_string();

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

    let email_renderer =
        Arc::new(EmailRenderer::new(tmp.path()).expect("create email renderer"));

    let service = ContentService::new(
        db_pool.clone(),
        Registry::snapshot(&registry),
        hook_runner,
        config.auth.secret.clone(),
        &config.depth,
        &config.pagination,
        config.email.clone(),
        email_renderer,
        config.server.clone(),
        None, // no event bus
        config.locale.clone(),
        tmp.path().to_path_buf(),
        std::sync::Arc::new(crap_cms::core::rate_limit::LoginRateLimiter::new(5, 300)),
        config.auth.reset_token_expiry,
        config.auth.password_policy.clone(),
        std::sync::Arc::new(crap_cms::core::rate_limit::LoginRateLimiter::new(3, 900)),
    );

    TestSetup { _tmp: tmp, service, pool: db_pool }
}

fn setup_service_with_locale(
    collections: Vec<CollectionDefinition>,
    globals: Vec<GlobalDefinition>,
    locales: Vec<&str>,
) -> TestSetup {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut config = CrapConfig::default();
    config.database.path = "test.db".to_string();
    config.auth.secret = "test-jwt-secret".to_string();
    config.locale.locales = locales.iter().map(|s| s.to_string()).collect();
    config.locale.default_locale = locales.first().unwrap_or(&"en").to_string();
    config.locale.fallback = true;

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

    let email_renderer =
        Arc::new(EmailRenderer::new(tmp.path()).expect("create email renderer"));

    let service = ContentService::new(
        db_pool.clone(),
        Registry::snapshot(&registry),
        hook_runner,
        config.auth.secret.clone(),
        &config.depth,
        &config.pagination,
        config.email.clone(),
        email_renderer,
        config.server.clone(),
        None,
        config.locale.clone(),
        tmp.path().to_path_buf(),
        std::sync::Arc::new(crap_cms::core::rate_limit::LoginRateLimiter::new(5, 300)),
        config.auth.reset_token_expiry,
        config.auth.password_policy.clone(),
        std::sync::Arc::new(crap_cms::core::rate_limit::LoginRateLimiter::new(3, 900)),
    );

    TestSetup { _tmp: tmp, service, pool: db_pool }
}

fn make_localized_posts_def() -> CollectionDefinition {
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
                localized: true,
                ..Default::default()
            },
            FieldDefinition {
                name: "body".to_string(),
                field_type: FieldType::Textarea,
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

fn make_versioned_posts_def() -> CollectionDefinition {
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
            FieldDefinition {
                name: "body".to_string(),
                field_type: FieldType::Textarea,
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
        versions: Some(VersionsConfig {
            drafts: true,
            max_versions: 10,
        }),
        indexes: Vec::new(),
    }
}

fn make_tags_def() -> CollectionDefinition {
    CollectionDefinition {
        slug: "tags".to_string(),
        labels: CollectionLabels {
            singular: Some(LocalizedString::Plain("Tag".to_string())),
            plural: Some(LocalizedString::Plain("Tags".to_string())),
        },
        timestamps: true,
        fields: vec![FieldDefinition {
            name: "name".to_string(),
            required: true,
            ..Default::default()
        }],
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

fn make_posts_with_has_many() -> CollectionDefinition {
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
            FieldDefinition {
                name: "tags".to_string(),
                field_type: FieldType::Relationship,
                relationship: Some(RelationshipConfig {
                    collection: "tags".to_string(),
                    has_many: true,
                    max_depth: None,
                    polymorphic: vec![],
                }),
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

fn make_complex_global_def() -> GlobalDefinition {
    GlobalDefinition {
        slug: "site_config".to_string(),
        labels: CollectionLabels {
            singular: Some(LocalizedString::Plain("Site Config".to_string())),
            plural: None,
        },
        fields: vec![
            FieldDefinition {
                name: "site_name".to_string(),
                ..Default::default()
            },
            FieldDefinition {
                name: "seo".to_string(),
                field_type: FieldType::Group,
                fields: vec![
                    FieldDefinition {
                        name: "meta_title".to_string(),
                        ..Default::default()
                    },
                    FieldDefinition {
                        name: "meta_description".to_string(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            FieldDefinition {
                name: "nav_items".to_string(),
                field_type: FieldType::Array,
                fields: vec![
                    FieldDefinition {
                        name: "label".to_string(),
                        ..Default::default()
                    },
                    FieldDefinition {
                        name: "url".to_string(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            FieldDefinition {
                name: "sections".to_string(),
                field_type: FieldType::Blocks,
                blocks: vec![BlockDefinition {
                    block_type: "hero".to_string(),
                    fields: vec![FieldDefinition {
                        name: "heading".to_string(),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            },
        ],
        hooks: CollectionHooks::default(),
        access: CollectionAccess::default(),
        mcp: Default::default(),
        live: None,
        versions: None,
    }
}

// ── Group 6: Localization (gRPC) ──────────────────────────────────────────

#[tokio::test]
async fn create_and_find_with_locale() {
    let ts = setup_service_with_locale(
        vec![make_localized_posts_def()],
        vec![],
        vec!["en", "de"],
    );

    // Create with locale=en
    ts.service
        .create(Request::new(content::CreateRequest {
            collection: "posts".to_string(),
            data: Some(make_struct(&[("title", "Hello"), ("body", "English body")])),
            locale: Some("en".to_string()),
            draft: None,
        }))
        .await
        .unwrap();

    // Find with locale=en should return the English title
    let resp = ts
        .service
        .find(Request::new(content::FindRequest {
            collection: "posts".to_string(),
            locale: Some("en".to_string()),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.pagination.as_ref().unwrap().total_docs, 1);
    assert_eq!(
        get_proto_field(&resp.documents[0], "title").as_deref(),
        Some("Hello"),
        "Should return English title"
    );
}

#[tokio::test]
async fn create_and_find_with_locale_fallback() {
    let ts = setup_service_with_locale(
        vec![make_localized_posts_def()],
        vec![],
        vec!["en", "de"],
    );

    // Create with locale=en only
    ts.service
        .create(Request::new(content::CreateRequest {
            collection: "posts".to_string(),
            data: Some(make_struct(&[("title", "English Only")])),
            locale: Some("en".to_string()),
            draft: None,
        }))
        .await
        .unwrap();

    // Find with locale=de should fallback to en value
    let resp = ts
        .service
        .find(Request::new(content::FindRequest {
            collection: "posts".to_string(),
            locale: Some("de".to_string()),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.pagination.as_ref().unwrap().total_docs, 1);
    assert_eq!(
        get_proto_field(&resp.documents[0], "title").as_deref(),
        Some("English Only"),
        "Fallback should return default locale value"
    );
}

#[tokio::test]
async fn create_and_find_with_locale_all() {
    let ts = setup_service_with_locale(
        vec![make_localized_posts_def()],
        vec![],
        vec!["en", "de"],
    );

    // Create English version
    let doc = ts
        .service
        .create(Request::new(content::CreateRequest {
            collection: "posts".to_string(),
            data: Some(make_struct(&[("title", "English Title")])),
            locale: Some("en".to_string()),
            draft: None,
        }))
        .await
        .unwrap()
        .into_inner()
        .document
        .unwrap();

    // Update with German version
    ts.service
        .update(Request::new(content::UpdateRequest {
            collection: "posts".to_string(),
            id: doc.id.clone(),
            data: Some(make_struct(&[("title", "Deutscher Titel")])),
            locale: Some("de".to_string()),
            draft: None,
            unpublish: None,
        }))
        .await
        .unwrap();

    // Find with locale=all should return nested locale objects
    let resp = ts
        .service
        .find(Request::new(content::FindRequest {
            collection: "posts".to_string(),
            locale: Some("all".to_string()),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.pagination.as_ref().unwrap().total_docs, 1);
    let fields = resp.documents[0].fields.as_ref().unwrap();
    let title_val = fields.fields.get("title");
    assert!(title_val.is_some(), "title field should be present");

    // When locale=all, title should be a struct with en/de keys
    match &title_val.unwrap().kind {
        Some(Kind::StructValue(s)) => {
            assert!(
                s.fields.contains_key("en"),
                "locale=all should have 'en' key"
            );
            assert!(
                s.fields.contains_key("de"),
                "locale=all should have 'de' key"
            );
        }
        other => {
            // Some implementations may return it differently
            panic!(
                "Expected struct with locale keys for locale=all, got: {:?}",
                other
            );
        }
    }
}

// ── Group 7: Drafts (gRPC) ───────────────────────────────────────────────

#[tokio::test]
async fn create_draft_and_find() {
    let ts = setup_service(vec![make_versioned_posts_def()], vec![]);

    // Create a draft
    let doc = ts
        .service
        .create(Request::new(content::CreateRequest {
            collection: "posts".to_string(),
            data: Some(make_struct(&[("title", "Draft Post"), ("body", "WIP")])),
            locale: None,
            draft: Some(true),
        }))
        .await
        .unwrap()
        .into_inner()
        .document
        .unwrap();
    assert!(!doc.id.is_empty());

    // Find with draft=true should return the draft
    let resp = ts
        .service
        .find(Request::new(content::FindRequest {
            collection: "posts".to_string(),
            draft: Some(true),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(resp.pagination.as_ref().unwrap().total_docs, 1, "draft=true should find the draft");

    // Find without draft flag should NOT return drafts
    let resp = ts
        .service
        .find(Request::new(content::FindRequest {
            collection: "posts".to_string(),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(resp.pagination.as_ref().unwrap().total_docs, 0, "default find should not return drafts");
}

#[tokio::test]
async fn draft_skips_required_validation() {
    let ts = setup_service(vec![make_versioned_posts_def()], vec![]);

    // Create a draft without required 'title' field — should succeed
    let resp = ts
        .service
        .create(Request::new(content::CreateRequest {
            collection: "posts".to_string(),
            data: Some(make_struct(&[("body", "Just a body")])),
            locale: None,
            draft: Some(true),
        }))
        .await;
    assert!(
        resp.is_ok(),
        "Draft should skip required validation: {:?}",
        resp.err()
    );
}

#[tokio::test]
async fn publish_draft() {
    let ts = setup_service(vec![make_versioned_posts_def()], vec![]);

    // Create a draft
    let doc = ts
        .service
        .create(Request::new(content::CreateRequest {
            collection: "posts".to_string(),
            data: Some(make_struct(&[
                ("title", "Draft to Publish"),
                ("body", "Content"),
            ])),
            locale: None,
            draft: Some(true),
        }))
        .await
        .unwrap()
        .into_inner()
        .document
        .unwrap();

    // Publish it by updating with draft=false
    ts.service
        .update(Request::new(content::UpdateRequest {
            collection: "posts".to_string(),
            id: doc.id.clone(),
            data: Some(make_struct(&[("title", "Draft to Publish")])),
            locale: None,
            draft: Some(false),
            unpublish: None,
        }))
        .await
        .unwrap();

    // Now find without draft flag should return it
    let resp = ts
        .service
        .find(Request::new(content::FindRequest {
            collection: "posts".to_string(),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(resp.pagination.as_ref().unwrap().total_docs, 1, "Published post should be findable");
}

// ── Group 8: Complex Globals (gRPC) ──────────────────────────────────────

#[tokio::test]
async fn update_global_with_nested_fields() {
    let ts = setup_service(vec![], vec![make_complex_global_def()]);

    // Build complex nested data
    let mut data_fields = BTreeMap::new();
    data_fields.insert("site_name".to_string(), str_val("My Site"));
    data_fields.insert(
        "seo".to_string(),
        struct_val(&[
            ("meta_title", str_val("Site Title")),
            ("meta_description", str_val("Site Description")),
        ]),
    );
    data_fields.insert(
        "nav_items".to_string(),
        list_val(vec![
            struct_val(&[("label", str_val("Home")), ("url", str_val("/"))]),
            struct_val(&[("label", str_val("About")), ("url", str_val("/about"))]),
        ]),
    );
    data_fields.insert(
        "sections".to_string(),
        list_val(vec![struct_val(&[
            ("_block_type", str_val("hero")),
            ("heading", str_val("Welcome!")),
        ])]),
    );

    ts.service
        .update_global(Request::new(content::UpdateGlobalRequest {
            slug: "site_config".to_string(),
            data: Some(Struct {
                fields: data_fields,
            }),
            locale: None,
        }))
        .await
        .unwrap();

    // Read back and verify
    let doc = ts
        .service
        .get_global(Request::new(content::GetGlobalRequest {
            slug: "site_config".to_string(),
            locale: None,
        }))
        .await
        .unwrap()
        .into_inner()
        .document
        .unwrap();

    let fields = doc.fields.as_ref().unwrap();
    assert_eq!(
        get_proto_field(&doc, "site_name").as_deref(),
        Some("My Site")
    );

    // Verify seo group
    let seo = fields.fields.get("seo");
    assert!(seo.is_some(), "seo group should exist");
    if let Some(Kind::StructValue(s)) = seo.unwrap().kind.as_ref() {
        assert!(
            s.fields.contains_key("meta_title"),
            "seo should have meta_title"
        );
    }

    // Verify nav_items array
    let nav = fields.fields.get("nav_items");
    assert!(nav.is_some(), "nav_items should exist");
    if let Some(Kind::ListValue(l)) = nav.unwrap().kind.as_ref() {
        assert_eq!(l.values.len(), 2, "Should have 2 nav items");
    }

    // Verify blocks
    let sections = fields.fields.get("sections");
    assert!(sections.is_some(), "sections should exist");
    if let Some(Kind::ListValue(l)) = sections.unwrap().kind.as_ref() {
        assert_eq!(l.values.len(), 1, "Should have 1 section block");
    }
}

// ── Group 9: Has-Many Relationship Filters (gRPC) ────────────────────────

#[tokio::test]
async fn find_with_has_many_relationship_filter() {
    let ts = setup_service(
        vec![make_tags_def(), make_posts_with_has_many()],
        vec![],
    );

    // Create tags
    let tag_rust = ts
        .service
        .create(Request::new(content::CreateRequest {
            collection: "tags".to_string(),
            data: Some(make_struct(&[("name", "rust")])),
            locale: None,
            draft: None,
        }))
        .await
        .unwrap()
        .into_inner()
        .document
        .unwrap();

    let tag_web = ts
        .service
        .create(Request::new(content::CreateRequest {
            collection: "tags".to_string(),
            data: Some(make_struct(&[("name", "web")])),
            locale: None,
            draft: None,
        }))
        .await
        .unwrap()
        .into_inner()
        .document
        .unwrap();

    // Create posts with tags (has-many: pass as comma-separated or list)
    let mut post1_fields = BTreeMap::new();
    post1_fields.insert("title".to_string(), str_val("Rust Post"));
    post1_fields.insert(
        "tags".to_string(),
        list_val(vec![str_val(&tag_rust.id)]),
    );
    ts.service
        .create(Request::new(content::CreateRequest {
            collection: "posts".to_string(),
            data: Some(Struct {
                fields: post1_fields,
            }),
            locale: None,
            draft: None,
        }))
        .await
        .unwrap();

    let mut post2_fields = BTreeMap::new();
    post2_fields.insert("title".to_string(), str_val("Web Post"));
    post2_fields.insert(
        "tags".to_string(),
        list_val(vec![str_val(&tag_web.id)]),
    );
    ts.service
        .create(Request::new(content::CreateRequest {
            collection: "posts".to_string(),
            data: Some(Struct {
                fields: post2_fields,
            }),
            locale: None,
            draft: None,
        }))
        .await
        .unwrap();

    let mut post3_fields = BTreeMap::new();
    post3_fields.insert("title".to_string(), str_val("Both Post"));
    post3_fields.insert(
        "tags".to_string(),
        list_val(vec![str_val(&tag_rust.id), str_val(&tag_web.id)]),
    );
    ts.service
        .create(Request::new(content::CreateRequest {
            collection: "posts".to_string(),
            data: Some(Struct {
                fields: post3_fields,
            }),
            locale: None,
            draft: None,
        }))
        .await
        .unwrap();

    // Filter posts by tags.id containing the rust tag ID
    let resp = ts
        .service
        .find(Request::new(content::FindRequest {
            collection: "posts".to_string(),
            r#where: Some(format!(r#"{{"tags.id": "{}"}}"#, tag_rust.id)),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        resp.pagination.as_ref().unwrap().total_docs, 2,
        "Should find 2 posts with rust tag (Rust Post + Both Post)"
    );
}

// ── Group 10: Bulk Operations (gRPC) ─────────────────────────────────────

#[tokio::test]
async fn update_many_with_filter() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    for (title, status) in &[
        ("A", "draft"),
        ("B", "draft"),
        ("C", "published"),
    ] {
        ts.service
            .create(Request::new(content::CreateRequest {
                collection: "posts".to_string(),
                data: Some(make_struct(&[("title", title), ("status", status)])),
                locale: None,
                draft: None,
            }))
            .await
            .unwrap();
    }

    // Update all drafts to published
    let resp = ts
        .service
        .update_many(Request::new(content::UpdateManyRequest {
            collection: "posts".to_string(),
            r#where: Some(r#"{"status": "draft"}"#.to_string()),
            data: Some(make_struct(&[("status", "published")])),
            locale: None,
            draft: None,
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(resp.modified, 2, "Should update 2 draft posts");

    // Verify all are now published
    let count_resp = ts
        .service
        .count(Request::new(content::CountRequest {
            collection: "posts".to_string(),
            r#where: Some(r#"{"status": "published"}"#.to_string()),
            locale: None,
            draft: None,
            search: None,
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(count_resp.count, 3, "All 3 should be published");
}

#[tokio::test]
async fn delete_many_with_where() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    for (title, status) in &[
        ("A", "draft"),
        ("B", "draft"),
        ("C", "published"),
        ("D", "published"),
    ] {
        ts.service
            .create(Request::new(content::CreateRequest {
                collection: "posts".to_string(),
                data: Some(make_struct(&[("title", title), ("status", status)])),
                locale: None,
                draft: None,
            }))
            .await
            .unwrap();
    }

    // Delete all drafts
    let resp = ts
        .service
        .delete_many(Request::new(content::DeleteManyRequest {
            collection: "posts".to_string(),
            r#where: Some(r#"{"status": "draft"}"#.to_string()),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(resp.deleted, 2, "Should delete 2 draft posts");

    // Verify only published remain
    let count_resp = ts
        .service
        .count(Request::new(content::CountRequest {
            collection: "posts".to_string(),
            r#where: None,
            locale: None,
            draft: None,
            search: None,
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(count_resp.count, 2, "2 published posts should remain");
}

// ── Group 11: Versions (gRPC) ────────────────────────────────────────────

#[tokio::test]
async fn list_and_restore_versions() {
    let ts = setup_service(vec![make_versioned_posts_def()], vec![]);

    // Create a published post
    let doc = ts
        .service
        .create(Request::new(content::CreateRequest {
            collection: "posts".to_string(),
            data: Some(make_struct(&[
                ("title", "Version 1"),
                ("body", "First version"),
            ])),
            locale: None,
            draft: None,
        }))
        .await
        .unwrap()
        .into_inner()
        .document
        .unwrap();

    // Update to version 2
    ts.service
        .update(Request::new(content::UpdateRequest {
            collection: "posts".to_string(),
            id: doc.id.clone(),
            data: Some(make_struct(&[
                ("title", "Version 2"),
                ("body", "Second version"),
            ])),
            locale: None,
            draft: None,
            unpublish: None,
        }))
        .await
        .unwrap();

    // Update to version 3
    ts.service
        .update(Request::new(content::UpdateRequest {
            collection: "posts".to_string(),
            id: doc.id.clone(),
            data: Some(make_struct(&[
                ("title", "Version 3"),
                ("body", "Third version"),
            ])),
            locale: None,
            draft: None,
            unpublish: None,
        }))
        .await
        .unwrap();

    // List versions
    let versions_resp = ts
        .service
        .list_versions(Request::new(content::ListVersionsRequest {
            collection: "posts".to_string(),
            id: doc.id.clone(),
            limit: None,
        }))
        .await
        .unwrap()
        .into_inner();

    assert!(
        versions_resp.versions.len() >= 2,
        "Should have at least 2 versions, got {}",
        versions_resp.versions.len()
    );

    // The latest version should be at the top
    let latest = &versions_resp.versions[0];
    assert!(latest.latest, "First version in list should be latest");

    // Restore an earlier version (not the latest)
    let earlier = versions_resp
        .versions
        .iter()
        .find(|v| !v.latest)
        .expect("Should have a non-latest version");

    let restored = ts
        .service
        .restore_version(Request::new(content::RestoreVersionRequest {
            collection: "posts".to_string(),
            document_id: doc.id.clone(),
            version_id: earlier.id.clone(),
        }))
        .await
        .unwrap()
        .into_inner()
        .document
        .unwrap();

    // The restored document should have an earlier version's title
    let restored_title = get_proto_field(&restored, "title");
    assert!(
        restored_title.is_some(),
        "Restored doc should have a title"
    );
    assert_ne!(
        restored_title.as_deref(),
        Some("Version 3"),
        "Restored should not be version 3 anymore"
    );
}

// ── Access Control / CRUD Gaps ────────────────────────────────────────────

#[tokio::test]
async fn create_returns_document_with_fields() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    // Create a document
    let doc = ts
        .service
        .create(Request::new(content::CreateRequest {
            collection: "posts".to_string(),
            data: Some(make_struct(&[("title", "Field Check"), ("status", "draft")])),
            locale: None,
            draft: None,
        }))
        .await
        .unwrap()
        .into_inner()
        .document
        .unwrap();

    // Verify the create response has all fields
    assert!(!doc.id.is_empty(), "Document should have an ID");
    assert_eq!(doc.collection, "posts");
    assert_eq!(get_proto_field(&doc, "title").as_deref(), Some("Field Check"));
    assert_eq!(get_proto_field(&doc, "status").as_deref(), Some("draft"));

    // Also fetch via FindByID with depth=0 to verify persistence
    let found = ts
        .service
        .find_by_id(Request::new(content::FindByIdRequest {
            collection: "posts".to_string(),
            id: doc.id.clone(),
            depth: Some(0),
            locale: None,
            select: vec![],
            draft: None,
        }))
        .await
        .unwrap()
        .into_inner()
        .document
        .expect("Document should be found");

    assert_eq!(found.id, doc.id);
    assert_eq!(get_proto_field(&found, "title").as_deref(), Some("Field Check"));
    assert_eq!(get_proto_field(&found, "status").as_deref(), Some("draft"));
}

#[tokio::test]
async fn find_with_pagination() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    // Create 5 documents with ordered titles
    for i in 0..5 {
        ts.service
            .create(Request::new(content::CreateRequest {
                collection: "posts".to_string(),
                data: Some(make_struct(&[("title", &format!("Page {}", i))])),
                locale: None,
                draft: None,
            }))
            .await
            .unwrap();
    }

    // Find with limit=2, page=2 — should return the 3rd and 4th documents
    let resp = ts
        .service
        .find(Request::new(content::FindRequest {
            collection: "posts".to_string(),
            limit: Some(2),
            page: Some(2),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.documents.len(), 2, "Should return exactly 2 documents");
    assert_eq!(resp.pagination.as_ref().unwrap().total_docs, 5, "Total count should still be 5 regardless of pagination");

    // Verify we can get the remaining page (page 3)
    let resp2 = ts
        .service
        .find(Request::new(content::FindRequest {
            collection: "posts".to_string(),
            limit: Some(2),
            page: Some(3),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp2.documents.len(), 1, "Last page should have 1 document");
    assert_eq!(resp2.pagination.as_ref().unwrap().total_docs, 5, "Total count should still be 5");
}

// ══════════════════════════════════════════════════════════════════════════════
// Count RPC Tests
// ══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn count_empty_collection() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    let resp = ts
        .service
        .count(Request::new(content::CountRequest {
            collection: "posts".to_string(),
            ..Default::default()
        }))
        .await
        .expect("Count failed");

    assert_eq!(resp.into_inner().count, 0);
}

#[tokio::test]
async fn count_with_documents() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    for title in &["A", "B", "C"] {
        ts.service
            .create(Request::new(content::CreateRequest {
                collection: "posts".to_string(),
                data: Some(make_struct(&[("title", title)])),
                locale: None,
                draft: None,
            }))
            .await
            .unwrap();
    }

    let resp = ts
        .service
        .count(Request::new(content::CountRequest {
            collection: "posts".to_string(),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.count, 3);
}

#[tokio::test]
async fn count_with_where() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    for (title, status) in &[("A", "draft"), ("B", "published"), ("C", "published")] {
        ts.service
            .create(Request::new(content::CreateRequest {
                collection: "posts".to_string(),
                data: Some(make_struct(&[("title", title), ("status", status)])),
                locale: None,
                draft: None,
            }))
            .await
            .unwrap();
    }

    let resp = ts
        .service
        .count(Request::new(content::CountRequest {
            collection: "posts".to_string(),
            r#where: Some(r#"{"status": "published"}"#.to_string()),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.count, 2);
}

#[tokio::test]
async fn count_with_where_json() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    for (title, status) in &[("A", "draft"), ("B", "published")] {
        ts.service
            .create(Request::new(content::CreateRequest {
                collection: "posts".to_string(),
                data: Some(make_struct(&[("title", title), ("status", status)])),
                locale: None,
                draft: None,
            }))
            .await
            .unwrap();
    }

    let resp = ts
        .service
        .count(Request::new(content::CountRequest {
            collection: "posts".to_string(),
            r#where: Some(r#"{"status": "draft"}"#.to_string()),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.count, 1);
}

#[tokio::test]
async fn count_nonexistent_collection() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    let err = ts
        .service
        .count(Request::new(content::CountRequest {
            collection: "nonexistent".to_string(),
            ..Default::default()
        }))
        .await
        .unwrap_err();

    assert_eq!(err.code(), tonic::Code::NotFound);
}

// ══════════════════════════════════════════════════════════════════════════════
// UpdateMany / DeleteMany Tests
// ══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn update_many_basic() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    for title in &["X", "Y", "Z"] {
        ts.service
            .create(Request::new(content::CreateRequest {
                collection: "posts".to_string(),
                data: Some(make_struct(&[("title", title), ("status", "draft")])),
                locale: None,
                draft: None,
            }))
            .await
            .unwrap();
    }

    let resp = ts
        .service
        .update_many(Request::new(content::UpdateManyRequest {
            collection: "posts".to_string(),
            r#where: Some(r#"{"status": "draft"}"#.to_string()),
            data: Some(make_struct(&[("status", "published")])),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.modified, 3);
}

#[tokio::test]
async fn update_many_with_where_partial() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    for (title, status) in &[("A", "draft"), ("B", "published"), ("C", "draft")] {
        ts.service
            .create(Request::new(content::CreateRequest {
                collection: "posts".to_string(),
                data: Some(make_struct(&[("title", title), ("status", status)])),
                locale: None,
                draft: None,
            }))
            .await
            .unwrap();
    }

    let resp = ts
        .service
        .update_many(Request::new(content::UpdateManyRequest {
            collection: "posts".to_string(),
            r#where: Some(r#"{"status": "draft"}"#.to_string()),
            data: Some(make_struct(&[("status", "published")])),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.modified, 2);
}

#[tokio::test]
async fn update_many_no_matches() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    let resp = ts
        .service
        .update_many(Request::new(content::UpdateManyRequest {
            collection: "posts".to_string(),
            r#where: Some(r#"{"status": "nonexistent"}"#.to_string()),
            data: Some(make_struct(&[("status", "published")])),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.modified, 0);
}

#[tokio::test]
async fn delete_many_basic() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    for title in &["A", "B", "C"] {
        ts.service
            .create(Request::new(content::CreateRequest {
                collection: "posts".to_string(),
                data: Some(make_struct(&[("title", title), ("status", "draft")])),
                locale: None,
                draft: None,
            }))
            .await
            .unwrap();
    }

    let resp = ts
        .service
        .delete_many(Request::new(content::DeleteManyRequest {
            collection: "posts".to_string(),
            r#where: Some(r#"{"status": "draft"}"#.to_string()),
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.deleted, 3);

    // Verify all deleted
    let count = ts
        .service
        .count(Request::new(content::CountRequest {
            collection: "posts".to_string(),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner()
        .count;
    assert_eq!(count, 0);
}

#[tokio::test]
async fn delete_many_with_where_partial() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    for (title, status) in &[("A", "draft"), ("B", "published"), ("C", "draft")] {
        ts.service
            .create(Request::new(content::CreateRequest {
                collection: "posts".to_string(),
                data: Some(make_struct(&[("title", title), ("status", status)])),
                locale: None,
                draft: None,
            }))
            .await
            .unwrap();
    }

    let resp = ts
        .service
        .delete_many(Request::new(content::DeleteManyRequest {
            collection: "posts".to_string(),
            r#where: Some(r#"{"status": "draft"}"#.to_string()),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.deleted, 2);
}

#[tokio::test]
async fn delete_many_no_matches() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    let resp = ts
        .service
        .delete_many(Request::new(content::DeleteManyRequest {
            collection: "posts".to_string(),
            r#where: Some(r#"{"status": "nonexistent"}"#.to_string()),
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.deleted, 0);
}

// ══════════════════════════════════════════════════════════════════════════════
// Versioning RPCs (collection without versions)
// ══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn list_versions_no_versioning() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    let doc = ts
        .service
        .create(Request::new(content::CreateRequest {
            collection: "posts".to_string(),
            data: Some(make_struct(&[("title", "V Test")])),
            locale: None,
            draft: None,
        }))
        .await
        .unwrap()
        .into_inner()
        .document
        .unwrap();

    let err = ts
        .service
        .list_versions(Request::new(content::ListVersionsRequest {
            collection: "posts".to_string(),
            id: doc.id,
            limit: None,
        }))
        .await
        .unwrap_err();

    assert_eq!(err.code(), tonic::Code::FailedPrecondition);
    assert!(err.message().contains("versioning"));
}

#[tokio::test]
async fn restore_version_no_versioning() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    let err = ts
        .service
        .restore_version(Request::new(content::RestoreVersionRequest {
            collection: "posts".to_string(),
            document_id: "some-id".to_string(),
            version_id: "some-version".to_string(),
        }))
        .await
        .unwrap_err();

    assert_eq!(err.code(), tonic::Code::FailedPrecondition);
}

// ══════════════════════════════════════════════════════════════════════════════
// Job RPCs (unauthenticated)
// ══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn list_jobs_unauthenticated() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    let err = ts
        .service
        .list_jobs(Request::new(content::ListJobsRequest {}))
        .await
        .unwrap_err();

    assert_eq!(err.code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn trigger_job_unauthenticated() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    let err = ts
        .service
        .trigger_job(Request::new(content::TriggerJobRequest {
            slug: "cleanup".to_string(),
            data_json: None,
        }))
        .await
        .unwrap_err();

    assert_eq!(err.code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn get_job_run_unauthenticated() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    let err = ts
        .service
        .get_job_run(Request::new(content::GetJobRunRequest {
            id: "some-id".to_string(),
        }))
        .await
        .unwrap_err();

    assert_eq!(err.code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn list_job_runs_unauthenticated() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    let err = ts
        .service
        .list_job_runs(Request::new(content::ListJobRunsRequest {
            slug: None,
            status: None,
            limit: None,
            offset: None,
        }))
        .await
        .unwrap_err();

    assert_eq!(err.code(), tonic::Code::Unauthenticated);
}

// ══════════════════════════════════════════════════════════════════════════════
// FindByID with select fields
// ══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn find_by_id_with_select() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    let doc = ts
        .service
        .create(Request::new(content::CreateRequest {
            collection: "posts".to_string(),
            data: Some(make_struct(&[("title", "Select Me"), ("status", "published")])),
            locale: None,
            draft: None,
        }))
        .await
        .unwrap()
        .into_inner()
        .document
        .unwrap();

    let found = ts
        .service
        .find_by_id(Request::new(content::FindByIdRequest {
            collection: "posts".to_string(),
            id: doc.id.clone(),
            depth: Some(0),
            locale: None,
            select: vec!["title".to_string()],
            draft: None,
        }))
        .await
        .unwrap()
        .into_inner()
        .document
        .unwrap();

    assert!(get_proto_field(&found, "title").is_some());
    // status should be stripped by select
    assert!(get_proto_field(&found, "status").is_none());
}

// ══════════════════════════════════════════════════════════════════════════════
// Update global nonexistent
// ══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn update_global_nonexistent() {
    let ts = setup_service(vec![], vec![]);

    let err = ts
        .service
        .update_global(Request::new(content::UpdateGlobalRequest {
            slug: "nonexistent".to_string(),
            data: Some(make_struct(&[("key", "value")])),
            locale: None,
        }))
        .await
        .unwrap_err();

    assert_eq!(err.code(), tonic::Code::NotFound);
}

// ══════════════════════════════════════════════════════════════════════════════
// Describe collection with auth and upload flags
// ══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn describe_auth_collection() {
    let ts = setup_service(vec![make_users_def()], vec![]);

    let resp = ts
        .service
        .describe_collection(Request::new(content::DescribeCollectionRequest {
            slug: "users".to_string(),
            is_global: false,
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.slug, "users");
    assert!(resp.auth);
    assert!(resp.timestamps);
    assert!(!resp.upload);
    assert!(!resp.drafts);
}

// ══════════════════════════════════════════════════════════════════════════════
// FindByID not found
// ══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn find_by_id_not_found() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    let resp = ts
        .service
        .find_by_id(Request::new(content::FindByIdRequest {
            collection: "posts".to_string(),
            id: "nonexistent-id".to_string(),
            depth: Some(0),
            locale: None,
            select: vec![],
            draft: None,
        }))
        .await
        .unwrap()
        .into_inner();

    assert!(resp.document.is_none());
}

// ══════════════════════════════════════════════════════════════════════════════
// Delete nonexistent collection
// ══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn delete_nonexistent_collection() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    let err = ts
        .service
        .delete(Request::new(content::DeleteRequest {
            collection: "nonexistent".to_string(),
            id: "some-id".to_string(),
        }))
        .await
        .unwrap_err();

    assert_eq!(err.code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn update_nonexistent_collection() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    let err = ts
        .service
        .update(Request::new(content::UpdateRequest {
            collection: "nonexistent".to_string(),
            id: "some-id".to_string(),
            data: Some(make_struct(&[("title", "Test")])),
            locale: None,
            draft: None,
            unpublish: None,
        }))
        .await
        .unwrap_err();

    assert_eq!(err.code(), tonic::Code::NotFound);
}

// ── FTS Search Tests ──────────────────────────────────────────────────────

#[tokio::test]
async fn find_with_search() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    // Create posts with distinct titles
    for title in &["Rust Programming Guide", "Python Tutorial", "Advanced Rust Patterns"] {
        ts.service
            .create(Request::new(content::CreateRequest {
                collection: "posts".to_string(),
                data: Some(make_struct(&[("title", title)])),
                locale: None,
                draft: None,
            }))
            .await
            .unwrap();
    }

    // Search for "Rust"
    let resp = ts
        .service
        .find(Request::new(content::FindRequest {
            collection: "posts".to_string(),
            search: Some("Rust".to_string()),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.pagination.as_ref().unwrap().total_docs, 2);
    assert_eq!(resp.documents.len(), 2);

    // All results should contain "Rust" in the title
    for doc in &resp.documents {
        let title = get_proto_field(doc, "title").unwrap();
        assert!(title.contains("Rust"), "Expected Rust in title, got: {}", title);
    }
}

#[tokio::test]
async fn find_with_search_no_results() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    ts.service
        .create(Request::new(content::CreateRequest {
            collection: "posts".to_string(),
            data: Some(make_struct(&[("title", "Hello World")])),
            locale: None,
            draft: None,
        }))
        .await
        .unwrap();

    let resp = ts
        .service
        .find(Request::new(content::FindRequest {
            collection: "posts".to_string(),
            search: Some("nonexistent_xyz".to_string()),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.pagination.as_ref().unwrap().total_docs, 0);
    assert!(resp.documents.is_empty());
}

#[tokio::test]
async fn find_with_search_and_where() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    // Create posts with different statuses
    for (title, status) in &[
        ("Rust Basics", "published"),
        ("Rust Advanced", "draft"),
        ("Python Basics", "published"),
    ] {
        ts.service
            .create(Request::new(content::CreateRequest {
                collection: "posts".to_string(),
                data: Some(make_struct(&[("title", title), ("status", status)])),
                locale: None,
                draft: None,
            }))
            .await
            .unwrap();
    }

    // Search for "Rust" + filter by status=published
    let resp = ts
        .service
        .find(Request::new(content::FindRequest {
            collection: "posts".to_string(),
            search: Some("Rust".to_string()),
            r#where: Some(r#"{"status": "published"}"#.to_string()),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    // Only "Rust Basics" should match (Rust + published)
    assert_eq!(resp.pagination.as_ref().unwrap().total_docs, 1);
    assert_eq!(
        get_proto_field(&resp.documents[0], "title").as_deref(),
        Some("Rust Basics")
    );
}

#[tokio::test]
async fn count_with_search() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    for title in &["Rust Guide", "Rust Tutorial", "Python Guide"] {
        ts.service
            .create(Request::new(content::CreateRequest {
                collection: "posts".to_string(),
                data: Some(make_struct(&[("title", title)])),
                locale: None,
                draft: None,
            }))
            .await
            .unwrap();
    }

    let resp = ts
        .service
        .count(Request::new(content::CountRequest {
            collection: "posts".to_string(),
            search: Some("Rust".to_string()),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.count, 2);
}

#[tokio::test]
async fn count_with_search_and_where() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    for (title, status) in &[
        ("Rust A", "published"),
        ("Rust B", "draft"),
        ("Python A", "published"),
    ] {
        ts.service
            .create(Request::new(content::CreateRequest {
                collection: "posts".to_string(),
                data: Some(make_struct(&[("title", title), ("status", status)])),
                locale: None,
                draft: None,
            }))
            .await
            .unwrap();
    }

    let resp = ts
        .service
        .count(Request::new(content::CountRequest {
            collection: "posts".to_string(),
            search: Some("Rust".to_string()),
            r#where: Some(r#"{"status": "published"}"#.to_string()),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.count, 1);
}

#[tokio::test]
async fn find_with_search_empty_string_returns_all() {
    let ts = setup_service(vec![make_posts_def()], vec![]);

    for title in &["A", "B"] {
        ts.service
            .create(Request::new(content::CreateRequest {
                collection: "posts".to_string(),
                data: Some(make_struct(&[("title", title)])),
                locale: None,
                draft: None,
            }))
            .await
            .unwrap();
    }

    // Empty search string should return all documents
    let resp = ts
        .service
        .find(Request::new(content::FindRequest {
            collection: "posts".to_string(),
            search: Some("".to_string()),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.pagination.as_ref().unwrap().total_docs, 2);
}
