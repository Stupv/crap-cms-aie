use std::collections::HashMap;
use std::path::PathBuf;

use crap_cms::config::CrapConfig;
use crap_cms::core::Document;
use crap_cms::db::{migrate, ops, pool, query};
use crap_cms::hooks;
use crap_cms::hooks::lifecycle::HookRunner;

fn setup() -> (tempfile::TempDir, crap_cms::db::DbPool, crap_cms::core::SharedRegistry, HookRunner) {
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("example");
    let config = CrapConfig::default();
    let registry = hooks::init_lua(&config_dir, &config).unwrap();

    let tmp = tempfile::tempdir().unwrap();
    let db_pool = pool::create_pool(tmp.path(), &config).unwrap();

    // Sync schema so tables exist
    migrate::sync_all(&db_pool, &registry, &config.locale).unwrap();

    let runner = HookRunner::new(&config_dir, registry.clone(), &config).unwrap();
    (tmp, db_pool, registry, runner)
}

fn make_user_doc(id: &str, role: &str) -> Document {
    let mut doc = Document::new(id.to_string());
    doc.fields.insert("role".into(), serde_json::json!(role));
    doc.fields.insert("email".into(), serde_json::json!(format!("{}@test.com", role)));
    doc.fields.insert("name".into(), serde_json::json!(role.to_uppercase()));
    doc
}

// ── 1. Lua Parsing ──────────────────────────────────────────────────────────

#[test]
fn access_config_parsed_from_lua() {
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("example");
    let config = CrapConfig::default();
    let registry = hooks::init_lua(&config_dir, &config).unwrap();
    let reg = registry.read().unwrap();
    let posts = reg.get_collection("posts").expect("posts collection not found");

    assert_eq!(posts.access.read.as_deref(), Some("access.published_or_author"));
    assert_eq!(posts.access.create.as_deref(), Some("access.authenticated"));
    assert_eq!(posts.access.update.as_deref(), Some("access.author_or_editor"));
    assert_eq!(posts.access.delete.as_deref(), Some("access.author_or_admin"));
}

#[test]
fn field_access_parsed_from_lua() {
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("example");
    let config = CrapConfig::default();
    let registry = hooks::init_lua(&config_dir, &config).unwrap();
    let reg = registry.read().unwrap();
    let posts = reg.get_collection("posts").expect("posts collection not found");

    // New posts definition has no field-level access controls
    for field in &posts.fields {
        assert!(field.access.read.is_none(), "field {} has unexpected read access", field.name);
        assert!(field.access.create.is_none(), "field {} has unexpected create access", field.name);
        assert!(field.access.update.is_none(), "field {} has unexpected update access", field.name);
    }
}

// ── 2. Collection-Level check_access ────────────────────────────────────────

#[test]
fn no_access_ref_allows() {
    let (_tmp, pool, _registry, runner) = setup();
    let conn = pool.get().unwrap();

    let result = runner.check_access(None, None, None, None, &conn).unwrap();
    assert!(matches!(result, query::AccessResult::Allowed));
}

#[test]
fn anyone_allows_anonymous() {
    let (_tmp, pool, _registry, runner) = setup();
    let conn = pool.get().unwrap();

    let result = runner.check_access(
        Some("access.anyone"), None, None, None, &conn,
    ).unwrap();
    assert!(matches!(result, query::AccessResult::Allowed));
}

#[test]
fn authenticated_denies_anonymous() {
    let (_tmp, pool, _registry, runner) = setup();
    let conn = pool.get().unwrap();

    let result = runner.check_access(
        Some("access.authenticated"), None, None, None, &conn,
    ).unwrap();
    assert!(matches!(result, query::AccessResult::Denied));
}

#[test]
fn authenticated_allows_user() {
    let (_tmp, pool, _registry, runner) = setup();
    let conn = pool.get().unwrap();
    let editor = make_user_doc("editor-1", "editor");

    let result = runner.check_access(
        Some("access.authenticated"), Some(&editor), None, None, &conn,
    ).unwrap();
    assert!(matches!(result, query::AccessResult::Allowed));
}

#[test]
fn admin_only_denies_editor() {
    let (_tmp, pool, _registry, runner) = setup();
    let conn = pool.get().unwrap();
    let editor = make_user_doc("editor-1", "editor");

    let result = runner.check_access(
        Some("access.admin_only"), Some(&editor), None, None, &conn,
    ).unwrap();
    assert!(matches!(result, query::AccessResult::Denied));
}

#[test]
fn admin_only_allows_admin() {
    let (_tmp, pool, _registry, runner) = setup();
    let conn = pool.get().unwrap();
    let admin = make_user_doc("admin-1", "admin");

    let result = runner.check_access(
        Some("access.admin_only"), Some(&admin), None, None, &conn,
    ).unwrap();
    assert!(matches!(result, query::AccessResult::Allowed));
}

#[test]
fn published_or_author_constrains_anonymous() {
    let (_tmp, pool, _registry, runner) = setup();
    let conn = pool.get().unwrap();

    let result = runner.check_access(
        Some("access.published_or_author"), None, None, None, &conn,
    ).unwrap();

    match result {
        query::AccessResult::Constrained(clauses) => {
            assert_eq!(clauses.len(), 1);
            match &clauses[0] {
                query::FilterClause::Single(f) => {
                    assert_eq!(f.field, "_status");
                    match &f.op {
                        query::FilterOp::Equals(val) => assert_eq!(val, "published"),
                        other => panic!("Expected Equals op, got {:?}", other),
                    }
                }
                other => panic!("Expected Single clause, got {:?}", other),
            }
        }
        other => panic!("Expected Constrained, got {:?}", other),
    }
}

#[test]
fn published_or_author_allows_admin() {
    let (_tmp, pool, _registry, runner) = setup();
    let conn = pool.get().unwrap();
    let admin = make_user_doc("admin-1", "admin");

    let result = runner.check_access(
        Some("access.published_or_author"), Some(&admin), None, None, &conn,
    ).unwrap();
    assert!(matches!(result, query::AccessResult::Allowed));
}

// ── 3. Field-Level Access ───────────────────────────────────────────────────

#[test]
fn field_write_no_field_access_allows_all() {
    let (_tmp, pool, registry, runner) = setup();
    let conn = pool.get().unwrap();
    let editor = make_user_doc("editor-1", "editor");

    let reg = registry.read().unwrap();
    let posts = reg.get_collection("posts").unwrap();

    let denied = runner.check_field_write_access(
        &posts.fields, Some(&editor), "update", &conn,
    );
    // No field-level access controls in posts definition
    assert!(denied.is_empty(),
        "Expected no denied fields, got: {:?}", denied);
}

#[test]
fn field_read_no_config_allows_all() {
    let (_tmp, pool, registry, runner) = setup();
    let conn = pool.get().unwrap();

    let reg = registry.read().unwrap();
    let posts = reg.get_collection("posts").unwrap();

    // No field has read access configured, so nothing should be denied
    let denied = runner.check_field_read_access(&posts.fields, None, &conn);
    assert!(denied.is_empty(),
        "Expected no denied fields for read, got: {:?}", denied);
}

// ── 4. End-to-End with DB ───────────────────────────────────────────────────

#[test]
fn constrained_find_filters_results() {
    let (_tmp, pool, registry, _runner) = setup();

    let reg = registry.read().unwrap();
    let posts = reg.get_collection("posts").unwrap().clone();
    drop(reg);

    // Create posts with different _status values via the versioning system
    let post_data = vec![
        ("Alpha Post", "alpha-post", "draft"),
        ("Beta Post", "beta-post", "published"),
        ("Gamma Post", "gamma-post", "draft"),
        ("Delta Post", "delta-post", "published"),
    ];
    for (title, slug, status) in &post_data {
        let mut data = HashMap::new();
        data.insert("title".to_string(), title.to_string());
        data.insert("slug".to_string(), slug.to_string());
        data.insert("excerpt".to_string(), "Test excerpt".to_string());

        let mut conn = pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        let doc = query::create(&tx, "posts", &posts, &data, None).unwrap();
        query::set_document_status(&tx, "posts", &doc.id, status).unwrap();
        tx.commit().unwrap();
    }

    // Simulate a Constrained access result (like published_or_author returns
    // for anonymous users), filtering to only published posts via _status.
    let constraint_filters = vec![
        query::FilterClause::Single(query::Filter {
            field: "_status".to_string(),
            op: query::FilterOp::Equals("published".to_string()),
        }),
    ];

    let find_query = query::FindQuery {
        filters: constraint_filters,
        ..Default::default()
    };

    let docs = ops::find_documents(&pool, "posts", &posts, &find_query, None).unwrap();

    // Should only see published posts
    assert_eq!(docs.len(), 2, "Expected 2 published posts, got {}", docs.len());
}

#[test]
fn access_check_plus_db_query_end_to_end() {
    let (_tmp, pool, registry, runner) = setup();

    let reg = registry.read().unwrap();
    let posts = reg.get_collection("posts").unwrap().clone();
    drop(reg);

    // Create some posts
    for (i, slug) in ["e2e-post-1", "e2e-post-2"].iter().enumerate() {
        let mut data = HashMap::new();
        data.insert("title".to_string(), format!("E2E Post {}", i + 1));
        data.insert("slug".to_string(), slug.to_string());
        data.insert("excerpt".to_string(), "Test excerpt".to_string());

        let mut conn = pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        let doc = query::create(&tx, "posts", &posts, &data, None).unwrap();
        query::set_document_status(&tx, "posts", &doc.id, "published").unwrap();
        tx.commit().unwrap();
    }

    // Verify published_or_author for anonymous returns constraint (not fully open)
    let conn = pool.get().unwrap();
    let result = runner.check_access(
        posts.access.read.as_deref(), None, None, None, &conn,
    ).unwrap();
    assert!(matches!(result, query::AccessResult::Constrained(_)));

    // Verify admin gets full access
    let admin = make_user_doc("admin-1", "admin");
    let result = runner.check_access(
        posts.access.read.as_deref(), Some(&admin), None, None, &conn,
    ).unwrap();
    assert!(matches!(result, query::AccessResult::Allowed));

    let all_docs = ops::find_documents(&pool, "posts", &posts, &query::FindQuery::default(), None).unwrap();
    assert_eq!(all_docs.len(), 2);

    // Verify author_or_admin denies anonymous delete
    let result = runner.check_access(
        posts.access.delete.as_deref(), None, None, None, &conn,
    ).unwrap();
    assert!(matches!(result, query::AccessResult::Denied));
}
