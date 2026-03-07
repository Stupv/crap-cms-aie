use std::collections::{HashMap, HashSet};

use crap_cms::config::{CrapConfig, LocaleConfig};
use crap_cms::core::collection::{
    CollectionAccess, CollectionAdmin, CollectionDefinition, CollectionHooks,
    CollectionLabels, GlobalDefinition,
};
use crap_cms::core::field::{
    BlockDefinition, FieldDefinition, FieldType,
    LocalizedString, RelationshipConfig,
};
use crap_cms::core::Registry;
use crap_cms::db::{migrate, pool, query};

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

fn create_test_pool() -> (tempfile::TempDir, crap_cms::db::DbPool) {
    let tmp = tempfile::tempdir().expect("Failed to create temp dir");
    let mut config = CrapConfig::default();
    config.database.path = "test.db".to_string();
    let db_pool = pool::create_pool(tmp.path(), &config).expect("Failed to create pool");
    (tmp, db_pool)
}

fn make_field(name: &str, field_type: FieldType) -> FieldDefinition {
    FieldDefinition {
        name: name.to_string(),
        field_type,
        ..Default::default()
    }
}

// ── 1C. Join Tables ──────────────────────────────────────────────────────────

fn make_articles_with_join_tables() -> CollectionDefinition {
    CollectionDefinition {
        slug: "articles".to_string(),
        labels: CollectionLabels::default(),
        timestamps: true,
        fields: vec![
            make_field("title", FieldType::Text),
            // has-many relationship
            FieldDefinition {
                name: "tags".to_string(),
                field_type: FieldType::Relationship,
                relationship: Some(RelationshipConfig {
                    collection: "tags".to_string(),
                    has_many: true,
                    max_depth: None,
                    polymorphic: vec![],
                }),
                ..make_field("tags", FieldType::Relationship)
            },
            // array field with sub-fields
            FieldDefinition {
                name: "links".to_string(),
                field_type: FieldType::Array,
                fields: vec![
                    make_field("url", FieldType::Text),
                    make_field("label", FieldType::Text),
                ],
                ..make_field("links", FieldType::Array)
            },
            // blocks field
            FieldDefinition {
                name: "content".to_string(),
                field_type: FieldType::Blocks,
                blocks: vec![
                    BlockDefinition {
                        block_type: "paragraph".to_string(),
                        fields: vec![make_field("text", FieldType::Textarea)],
                        ..Default::default()
                    },
                    BlockDefinition {
                        block_type: "image".to_string(),
                        fields: vec![make_field("url", FieldType::Text)],
                        ..Default::default()
                    },
                ],
                ..make_field("content", FieldType::Blocks)
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

fn setup_articles() -> (tempfile::TempDir, crap_cms::db::DbPool, CollectionDefinition) {
    let (_tmp, pool) = create_test_pool();
    let registry = Registry::shared();
    let def = make_articles_with_join_tables();
    let tags_def = CollectionDefinition {
        slug: "tags".to_string(),
        labels: CollectionLabels::default(),
        timestamps: true,
        fields: vec![make_field("name", FieldType::Text)],
        admin: CollectionAdmin::default(),
        hooks: CollectionHooks::default(),
        auth: None,
        upload: None,
        access: CollectionAccess::default(),
        mcp: Default::default(),
        live: None,
            versions: None,
            indexes: Vec::new(),
    };
    {
        let mut reg = registry.write().unwrap();
        reg.register_collection(def.clone());
        reg.register_collection(tags_def);
    }
    migrate::sync_all(&pool, &registry, &CrapConfig::default().locale).expect("Sync failed");
    (_tmp, pool, def)
}

#[test]
fn set_and_find_related_ids() {
    let (_tmp, pool, def) = setup_articles();

    // Create an article
    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");
    let mut data = HashMap::new();
    data.insert("title".to_string(), "Test Article".to_string());
    let doc = query::create(&tx, "articles", &def, &data, None).expect("Create failed");

    let ids = vec!["tag-1".to_string(), "tag-2".to_string(), "tag-3".to_string()];
    query::set_related_ids(&tx, "articles", "tags", &doc.id, &ids, None)
        .expect("Set related ids failed");
    tx.commit().expect("Commit");

    let conn = pool.get().expect("DB connection");
    let found = query::find_related_ids(&conn, "articles", "tags", &doc.id, None)
        .expect("Find related ids failed");
    assert_eq!(found, ids);
}

#[test]
fn set_related_ids_replaces_existing() {
    let (_tmp, pool, def) = setup_articles();
    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");
    let mut data = HashMap::new();
    data.insert("title".to_string(), "Test".to_string());
    let doc = query::create(&tx, "articles", &def, &data, None).expect("Create failed");

    // First set
    query::set_related_ids(&tx, "articles", "tags", &doc.id, &["a".to_string(), "b".to_string()], None)
        .expect("Set failed");

    // Replace
    query::set_related_ids(&tx, "articles", "tags", &doc.id, &["c".to_string(), "d".to_string()], None)
        .expect("Set failed");
    tx.commit().expect("Commit");

    let conn = pool.get().expect("DB connection");
    let found = query::find_related_ids(&conn, "articles", "tags", &doc.id, None)
        .expect("Find failed");
    assert_eq!(found, vec!["c".to_string(), "d".to_string()]);
}

#[test]
fn find_related_ids_empty() {
    let (_tmp, pool, def) = setup_articles();
    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");
    let mut data = HashMap::new();
    data.insert("title".to_string(), "Test".to_string());
    let doc = query::create(&tx, "articles", &def, &data, None).expect("Create failed");
    tx.commit().expect("Commit");

    let conn = pool.get().expect("DB connection");
    let found = query::find_related_ids(&conn, "articles", "tags", &doc.id, None)
        .expect("Find failed");
    assert!(found.is_empty());
}

#[test]
fn set_and_find_array_rows() {
    let (_tmp, pool, def) = setup_articles();
    let links_field = def.fields.iter().find(|f| f.name == "links").unwrap();
    let sub_fields = &links_field.fields;

    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");
    let mut data = HashMap::new();
    data.insert("title".to_string(), "Test".to_string());
    let doc = query::create(&tx, "articles", &def, &data, None).expect("Create failed");

    let rows = vec![
        {
            let mut m = HashMap::new();
            m.insert("url".to_string(), "https://example.com".to_string());
            m.insert("label".to_string(), "Example".to_string());
            m
        },
        {
            let mut m = HashMap::new();
            m.insert("url".to_string(), "https://rust-lang.org".to_string());
            m.insert("label".to_string(), "Rust".to_string());
            m
        },
    ];
    query::set_array_rows(&tx, "articles", "links", &doc.id, &rows, sub_fields, None)
        .expect("Set array rows failed");
    tx.commit().expect("Commit");

    let conn = pool.get().expect("DB connection");
    let found = query::find_array_rows(&conn, "articles", "links", &doc.id, sub_fields, None)
        .expect("Find array rows failed");
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].get("url").unwrap().as_str().unwrap(), "https://example.com");
    assert_eq!(found[0].get("label").unwrap().as_str().unwrap(), "Example");
    assert_eq!(found[1].get("url").unwrap().as_str().unwrap(), "https://rust-lang.org");
    // Each row should have an id
    assert!(found[0].get("id").is_some());
}

#[test]
fn set_array_rows_replaces_existing() {
    let (_tmp, pool, def) = setup_articles();
    let links_field = def.fields.iter().find(|f| f.name == "links").unwrap();
    let sub_fields = &links_field.fields;

    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");
    let mut data = HashMap::new();
    data.insert("title".to_string(), "Test".to_string());
    let doc = query::create(&tx, "articles", &def, &data, None).expect("Create failed");

    let rows1 = vec![{
        let mut m = HashMap::new();
        m.insert("url".to_string(), "https://old.com".to_string());
        m.insert("label".to_string(), "Old".to_string());
        m
    }];
    query::set_array_rows(&tx, "articles", "links", &doc.id, &rows1, sub_fields, None)
        .expect("Set failed");

    let rows2 = vec![{
        let mut m = HashMap::new();
        m.insert("url".to_string(), "https://new.com".to_string());
        m.insert("label".to_string(), "New".to_string());
        m
    }];
    query::set_array_rows(&tx, "articles", "links", &doc.id, &rows2, sub_fields, None)
        .expect("Set failed");
    tx.commit().expect("Commit");

    let conn = pool.get().expect("DB connection");
    let found = query::find_array_rows(&conn, "articles", "links", &doc.id, sub_fields, None)
        .expect("Find failed");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].get("url").unwrap().as_str().unwrap(), "https://new.com");
}

#[test]
fn find_array_rows_empty() {
    let (_tmp, pool, def) = setup_articles();
    let links_field = def.fields.iter().find(|f| f.name == "links").unwrap();
    let sub_fields = &links_field.fields;

    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");
    let mut data = HashMap::new();
    data.insert("title".to_string(), "Test".to_string());
    let doc = query::create(&tx, "articles", &def, &data, None).expect("Create failed");
    tx.commit().expect("Commit");

    let conn = pool.get().expect("DB connection");
    let found = query::find_array_rows(&conn, "articles", "links", &doc.id, sub_fields, None)
        .expect("Find failed");
    assert!(found.is_empty());
}

#[test]
fn set_and_find_block_rows() {
    let (_tmp, pool, def) = setup_articles();
    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");
    let mut data = HashMap::new();
    data.insert("title".to_string(), "Test".to_string());
    let doc = query::create(&tx, "articles", &def, &data, None).expect("Create failed");

    let blocks = vec![
        serde_json::json!({"_block_type": "paragraph", "text": "Hello world"}),
        serde_json::json!({"_block_type": "image", "url": "/img/test.png"}),
    ];
    query::set_block_rows(&tx, "articles", "content", &doc.id, &blocks, None)
        .expect("Set block rows failed");
    tx.commit().expect("Commit");

    let conn = pool.get().expect("DB connection");
    let found = query::find_block_rows(&conn, "articles", "content", &doc.id, None)
        .expect("Find block rows failed");
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].get("_block_type").unwrap().as_str().unwrap(), "paragraph");
    assert_eq!(found[0].get("text").unwrap().as_str().unwrap(), "Hello world");
    assert_eq!(found[1].get("_block_type").unwrap().as_str().unwrap(), "image");
    assert_eq!(found[1].get("url").unwrap().as_str().unwrap(), "/img/test.png");
    // Each block should have an id
    assert!(found[0].get("id").is_some());
}

#[test]
fn set_block_rows_replaces_existing() {
    let (_tmp, pool, def) = setup_articles();
    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");
    let mut data = HashMap::new();
    data.insert("title".to_string(), "Test".to_string());
    let doc = query::create(&tx, "articles", &def, &data, None).expect("Create failed");

    let blocks1 = vec![serde_json::json!({"_block_type": "paragraph", "text": "Old"})];
    query::set_block_rows(&tx, "articles", "content", &doc.id, &blocks1, None)
        .expect("Set failed");

    let blocks2 = vec![serde_json::json!({"_block_type": "image", "url": "/new.png"})];
    query::set_block_rows(&tx, "articles", "content", &doc.id, &blocks2, None)
        .expect("Set failed");
    tx.commit().expect("Commit");

    let conn = pool.get().expect("DB connection");
    let found = query::find_block_rows(&conn, "articles", "content", &doc.id, None)
        .expect("Find failed");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].get("_block_type").unwrap().as_str().unwrap(), "image");
}

#[test]
fn find_block_rows_empty() {
    let (_tmp, pool, def) = setup_articles();
    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");
    let mut data = HashMap::new();
    data.insert("title".to_string(), "Test".to_string());
    let doc = query::create(&tx, "articles", &def, &data, None).expect("Create failed");
    tx.commit().expect("Commit");

    let conn = pool.get().expect("DB connection");
    let found = query::find_block_rows(&conn, "articles", "content", &doc.id, None)
        .expect("Find failed");
    assert!(found.is_empty());
}

#[test]
fn hydrate_document_populates_join_data() {
    let (_tmp, pool, def) = setup_articles();
    let links_field = def.fields.iter().find(|f| f.name == "links").unwrap();

    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");
    let mut data = HashMap::new();
    data.insert("title".to_string(), "Test".to_string());
    let doc = query::create(&tx, "articles", &def, &data, None).expect("Create failed");

    // Set up join table data
    query::set_related_ids(&tx, "articles", "tags", &doc.id, &["t1".to_string(), "t2".to_string()], None)
        .expect("Set related failed");
    let rows = vec![{
        let mut m = HashMap::new();
        m.insert("url".to_string(), "https://example.com".to_string());
        m.insert("label".to_string(), "Ex".to_string());
        m
    }];
    query::set_array_rows(&tx, "articles", "links", &doc.id, &rows, &links_field.fields, None)
        .expect("Set array failed");
    let blocks = vec![serde_json::json!({"_block_type": "paragraph", "text": "Hi"})];
    query::set_block_rows(&tx, "articles", "content", &doc.id, &blocks, None)
        .expect("Set blocks failed");
    tx.commit().expect("Commit");

    // Hydrate
    let conn = pool.get().expect("DB connection");
    let mut doc = query::find_by_id(&conn, "articles", &def, &doc.id, None)
        .expect("Find failed").expect("Not found");
    query::hydrate_document(&conn, "articles", &def.fields, &mut doc, None, None)
        .expect("Hydrate failed");

    // Verify tags (has-many relationship)
    let tags = doc.get("tags").expect("tags should exist");
    assert!(tags.is_array());
    let tags_arr = tags.as_array().unwrap();
    assert_eq!(tags_arr.len(), 2);
    assert_eq!(tags_arr[0].as_str().unwrap(), "t1");

    // Verify links (array)
    let links = doc.get("links").expect("links should exist");
    assert!(links.is_array());
    let links_arr = links.as_array().unwrap();
    assert_eq!(links_arr.len(), 1);
    assert_eq!(links_arr[0].get("url").unwrap().as_str().unwrap(), "https://example.com");

    // Verify content (blocks)
    let content = doc.get("content").expect("content should exist");
    assert!(content.is_array());
    let blocks_arr = content.as_array().unwrap();
    assert_eq!(blocks_arr.len(), 1);
    assert_eq!(blocks_arr[0].get("_block_type").unwrap().as_str().unwrap(), "paragraph");
}

#[test]
fn save_join_table_data_from_hashmap() {
    let (_tmp, pool, def) = setup_articles();
    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");
    let mut data = HashMap::new();
    data.insert("title".to_string(), "Test".to_string());
    let doc = query::create(&tx, "articles", &def, &data, None).expect("Create failed");

    // Prepare join table data as JSON values
    let mut jt_data: HashMap<String, serde_json::Value> = HashMap::new();
    jt_data.insert("tags".to_string(), serde_json::json!(["tag-a", "tag-b"]));
    jt_data.insert("links".to_string(), serde_json::json!([
        {"url": "https://a.com", "label": "A"},
        {"url": "https://b.com", "label": "B"},
    ]));
    jt_data.insert("content".to_string(), serde_json::json!([
        {"_block_type": "paragraph", "text": "Content block"},
    ]));

    query::save_join_table_data(&tx, "articles", &def.fields, &doc.id, &jt_data, None)
        .expect("Save join table data failed");
    tx.commit().expect("Commit");

    // Verify
    let conn = pool.get().expect("DB connection");
    let tags = query::find_related_ids(&conn, "articles", "tags", &doc.id, None)
        .expect("Find tags failed");
    assert_eq!(tags, vec!["tag-a", "tag-b"]);

    let links_field = def.fields.iter().find(|f| f.name == "links").unwrap();
    let links = query::find_array_rows(&conn, "articles", "links", &doc.id, &links_field.fields, None)
        .expect("Find links failed");
    assert_eq!(links.len(), 2);

    let blocks = query::find_block_rows(&conn, "articles", "content", &doc.id, None)
        .expect("Find blocks failed");
    assert_eq!(blocks.len(), 1);
}

#[test]
fn save_join_table_data_partial_update() {
    let (_tmp, pool, def) = setup_articles();
    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");
    let mut data = HashMap::new();
    data.insert("title".to_string(), "Test".to_string());
    let doc = query::create(&tx, "articles", &def, &data, None).expect("Create failed");

    // First: set tags and links
    let mut jt_data: HashMap<String, serde_json::Value> = HashMap::new();
    jt_data.insert("tags".to_string(), serde_json::json!(["tag-1", "tag-2"]));
    jt_data.insert("links".to_string(), serde_json::json!([{"url": "https://a.com", "label": "A"}]));
    query::save_join_table_data(&tx, "articles", &def.fields, &doc.id, &jt_data, None)
        .expect("Save failed");

    // Second: only update tags (links should be unchanged)
    let mut jt_data2: HashMap<String, serde_json::Value> = HashMap::new();
    jt_data2.insert("tags".to_string(), serde_json::json!(["tag-3"]));
    query::save_join_table_data(&tx, "articles", &def.fields, &doc.id, &jt_data2, None)
        .expect("Save failed");
    tx.commit().expect("Commit");

    let conn = pool.get().expect("DB connection");
    let tags = query::find_related_ids(&conn, "articles", "tags", &doc.id, None)
        .expect("Find tags failed");
    assert_eq!(tags, vec!["tag-3"]);

    let links_field = def.fields.iter().find(|f| f.name == "links").unwrap();
    let links = query::find_array_rows(&conn, "articles", "links", &doc.id, &links_field.fields, None)
        .expect("Find links failed");
    // Links should be unchanged (not in the second update)
    assert_eq!(links.len(), 1);
}

// ── 1D. Relationship Population / Depth ───────────────────────────────────────

fn make_categories_def() -> CollectionDefinition {
    CollectionDefinition {
        slug: "categories".to_string(),
        labels: CollectionLabels::default(),
        timestamps: true,
        fields: vec![
            make_field("name", FieldType::Text),
            // Self-referencing parent (for circular ref test)
            FieldDefinition {
                name: "parent".to_string(),
                field_type: FieldType::Relationship,
                relationship: Some(RelationshipConfig {
                    collection: "categories".to_string(),
                    has_many: false,
                    max_depth: None,
                    polymorphic: vec![],
                }),
                ..make_field("parent", FieldType::Relationship)
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

fn make_posts_with_category() -> CollectionDefinition {
    CollectionDefinition {
        slug: "posts_v2".to_string(),
        labels: CollectionLabels::default(),
        timestamps: true,
        fields: vec![
            make_field("title", FieldType::Text),
            // has-one relationship to categories
            FieldDefinition {
                name: "category".to_string(),
                field_type: FieldType::Relationship,
                relationship: Some(RelationshipConfig {
                    collection: "categories".to_string(),
                    has_many: false,
                    max_depth: None,
                    polymorphic: vec![],
                }),
                ..make_field("category", FieldType::Relationship)
            },
            // has-many relationship to categories
            FieldDefinition {
                name: "secondary_categories".to_string(),
                field_type: FieldType::Relationship,
                relationship: Some(RelationshipConfig {
                    collection: "categories".to_string(),
                    has_many: true,
                    max_depth: None,
                    polymorphic: vec![],
                }),
                ..make_field("secondary_categories", FieldType::Relationship)
            },
            // field with max_depth cap
            FieldDefinition {
                name: "limited_cat".to_string(),
                field_type: FieldType::Relationship,
                relationship: Some(RelationshipConfig {
                    collection: "categories".to_string(),
                    has_many: false,
                    max_depth: Some(0),
                    polymorphic: vec![],
                }),
                ..make_field("limited_cat", FieldType::Relationship)
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

fn setup_posts_categories() -> (
    tempfile::TempDir,
    crap_cms::db::DbPool,
    crap_cms::core::SharedRegistry,
    CollectionDefinition,
    CollectionDefinition,
) {
    let (_tmp, pool) = create_test_pool();
    let shared_registry = Registry::shared();
    let cats_def = make_categories_def();
    let posts_def = make_posts_with_category();
    {
        let mut reg = shared_registry.write().unwrap();
        reg.register_collection(cats_def.clone());
        reg.register_collection(posts_def.clone());
    }
    migrate::sync_all(&pool, &shared_registry, &CrapConfig::default().locale).expect("Sync failed");

    (_tmp, pool, shared_registry, posts_def, cats_def)
}

#[test]
fn populate_depth_0_leaves_ids() {
    let (_tmp, pool, registry, posts_def, cats_def) = setup_posts_categories();

    // Create a category
    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");
    let mut cat_data = HashMap::new();
    cat_data.insert("name".to_string(), "Tech".to_string());
    let cat = query::create(&tx, "categories", &cats_def, &cat_data, None).expect("Create cat failed");

    let mut post_data = HashMap::new();
    post_data.insert("title".to_string(), "My Post".to_string());
    post_data.insert("category".to_string(), cat.id.clone());
    let mut post = query::create(&tx, "posts_v2", &posts_def, &post_data, None).expect("Create post failed");
    tx.commit().expect("Commit");

    // Populate at depth 0 — should be a no-op
    let conn = pool.get().expect("DB connection");
    let mut visited = HashSet::new();
    query::populate_relationships(
        &query::PopulateContext { conn: &conn, registry: &registry.read().unwrap(), collection_slug: "posts_v2", def: &posts_def },
        &mut post, &mut visited,
        &query::PopulateOpts { depth: 0, select: None, locale_ctx: None },
    ).expect("Populate failed");

    // category should still be an ID string
    assert_eq!(post.get_str("category"), Some(cat.id.as_str()));
}

#[test]
fn populate_depth_1_hydrates_has_one() {
    let (_tmp, pool, registry, posts_def, cats_def) = setup_posts_categories();

    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");
    let mut cat_data = HashMap::new();
    cat_data.insert("name".to_string(), "Tech".to_string());
    let cat = query::create(&tx, "categories", &cats_def, &cat_data, None).expect("Create cat");

    let mut post_data = HashMap::new();
    post_data.insert("title".to_string(), "My Post".to_string());
    post_data.insert("category".to_string(), cat.id.clone());
    let mut post = query::create(&tx, "posts_v2", &posts_def, &post_data, None).expect("Create post");
    tx.commit().expect("Commit");

    let conn = pool.get().expect("DB connection");
    let mut visited = HashSet::new();
    query::populate_relationships(
        &query::PopulateContext { conn: &conn, registry: &registry.read().unwrap(), collection_slug: "posts_v2", def: &posts_def },
        &mut post, &mut visited,
        &query::PopulateOpts { depth: 1, select: None, locale_ctx: None },
    ).expect("Populate failed");

    // category should be a full document object
    let cat_val = post.get("category").expect("category should exist");
    assert!(cat_val.is_object(), "category should be an object, got: {:?}", cat_val);
    assert_eq!(cat_val.get("name").unwrap().as_str().unwrap(), "Tech");
    assert_eq!(cat_val.get("id").unwrap().as_str().unwrap(), cat.id);
}

#[test]
fn populate_depth_1_hydrates_has_many() {
    let (_tmp, pool, registry, posts_def, cats_def) = setup_posts_categories();

    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");

    let mut cat1_data = HashMap::new();
    cat1_data.insert("name".to_string(), "Tech".to_string());
    let cat1 = query::create(&tx, "categories", &cats_def, &cat1_data, None).expect("Create cat1");

    let mut cat2_data = HashMap::new();
    cat2_data.insert("name".to_string(), "Science".to_string());
    let cat2 = query::create(&tx, "categories", &cats_def, &cat2_data, None).expect("Create cat2");

    let mut post_data = HashMap::new();
    post_data.insert("title".to_string(), "Multi-cat Post".to_string());
    let mut post = query::create(&tx, "posts_v2", &posts_def, &post_data, None).expect("Create post");

    query::set_related_ids(&tx, "posts_v2", "secondary_categories", &post.id, &[cat1.id.clone(), cat2.id.clone()], None)
        .expect("Set related failed");
    tx.commit().expect("Commit");

    let conn = pool.get().expect("DB connection");
    query::hydrate_document(&conn, "posts_v2", &posts_def.fields, &mut post, None, None)
        .expect("Hydrate failed");

    let mut visited = HashSet::new();
    query::populate_relationships(
        &query::PopulateContext { conn: &conn, registry: &registry.read().unwrap(), collection_slug: "posts_v2", def: &posts_def },
        &mut post, &mut visited,
        &query::PopulateOpts { depth: 1, select: None, locale_ctx: None },
    ).expect("Populate failed");

    let sec_cats = post.get("secondary_categories").expect("should exist");
    assert!(sec_cats.is_array());
    let arr = sec_cats.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    // Should be full objects, not IDs
    assert!(arr[0].is_object());
    assert!(arr[0].get("name").is_some());
}

#[test]
fn populate_circular_ref_stops() {
    let (_tmp, pool, registry, _posts_def, cats_def) = setup_posts_categories();

    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");

    // Create cat A → parent B → parent A (circular)
    let mut a_data = HashMap::new();
    a_data.insert("name".to_string(), "A".to_string());
    let cat_a = query::create(&tx, "categories", &cats_def, &a_data, None).expect("Create A");

    let mut b_data = HashMap::new();
    b_data.insert("name".to_string(), "B".to_string());
    b_data.insert("parent".to_string(), cat_a.id.clone());
    let cat_b = query::create(&tx, "categories", &cats_def, &b_data, None).expect("Create B");

    // Update A to point to B
    let mut update = HashMap::new();
    update.insert("parent".to_string(), cat_b.id.clone());
    let mut cat_a = query::update(&tx, "categories", &cats_def, &cat_a.id, &update, None).expect("Update A");
    tx.commit().expect("Commit");

    // Populate at depth 10 — should not infinite loop
    let conn = pool.get().expect("DB connection");
    let mut visited = HashSet::new();
    query::populate_relationships(
        &query::PopulateContext { conn: &conn, registry: &registry.read().unwrap(), collection_slug: "categories", def: &cats_def },
        &mut cat_a, &mut visited,
        &query::PopulateOpts { depth: 10, select: None, locale_ctx: None },
    ).expect("Populate should not loop");
    // Should complete without panic
}

#[test]
fn populate_missing_related_doc() {
    let (_tmp, pool, registry, posts_def, _cats_def) = setup_posts_categories();

    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");
    let mut post_data = HashMap::new();
    post_data.insert("title".to_string(), "Orphaned".to_string());
    post_data.insert("category".to_string(), "nonexistent-cat-id".to_string());
    let mut post = query::create(&tx, "posts_v2", &posts_def, &post_data, None).expect("Create post");
    tx.commit().expect("Commit");

    let conn = pool.get().expect("DB connection");
    let mut visited = HashSet::new();
    query::populate_relationships(
        &query::PopulateContext { conn: &conn, registry: &registry.read().unwrap(), collection_slug: "posts_v2", def: &posts_def },
        &mut post, &mut visited,
        &query::PopulateOpts { depth: 1, select: None, locale_ctx: None },
    ).expect("Populate should handle missing");

    // Category should remain as a string ID (not populated)
    assert_eq!(post.get_str("category"), Some("nonexistent-cat-id"));
}

#[test]
fn populate_respects_field_max_depth() {
    let (_tmp, pool, registry, posts_def, cats_def) = setup_posts_categories();

    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");

    let mut cat_data = HashMap::new();
    cat_data.insert("name".to_string(), "Tech".to_string());
    let cat = query::create(&tx, "categories", &cats_def, &cat_data, None).expect("Create cat");

    let mut post_data = HashMap::new();
    post_data.insert("title".to_string(), "Post".to_string());
    post_data.insert("limited_cat".to_string(), cat.id.clone());
    let mut post = query::create(&tx, "posts_v2", &posts_def, &post_data, None).expect("Create post");
    tx.commit().expect("Commit");

    let conn = pool.get().expect("DB connection");
    let mut visited = HashSet::new();
    // Even with depth=5, the limited_cat field has max_depth=0, so it shouldn't populate
    query::populate_relationships(
        &query::PopulateContext { conn: &conn, registry: &registry.read().unwrap(), collection_slug: "posts_v2", def: &posts_def },
        &mut post, &mut visited,
        &query::PopulateOpts { depth: 5, select: None, locale_ctx: None },
    ).expect("Populate failed");

    // limited_cat should remain as string ID (max_depth=0 prevents population)
    assert_eq!(post.get_str("limited_cat"), Some(cat.id.as_str()));
}

// Regression: populate_relationships with localized fields on the related collection
// used to fail because find_by_ids was called without locale_ctx, generating
// `SELECT caption` instead of `SELECT caption__en` for localized columns.
#[test]
fn populate_with_localized_related_collection() {
    let (_tmp, pool) = create_test_pool();
    let shared_registry = Registry::shared();

    // "media" collection with a localized field
    let media_def = CollectionDefinition {
        slug: "media".to_string(),
        labels: CollectionLabels::default(),
        timestamps: true,
        fields: vec![
            make_field("url", FieldType::Text),
            FieldDefinition {
                name: "caption".to_string(),
                localized: true,
                ..make_field("caption", FieldType::Text)
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
    };

    // "articles" collection with a relationship to media
    let articles_def = CollectionDefinition {
        slug: "articles".to_string(),
        labels: CollectionLabels::default(),
        timestamps: true,
        fields: vec![
            make_field("title", FieldType::Text),
            FieldDefinition {
                name: "image".to_string(),
                field_type: FieldType::Relationship,
                relationship: Some(RelationshipConfig {
                    collection: "media".to_string(),
                    has_many: false,
                    max_depth: None,
                    polymorphic: vec![],
                }),
                ..make_field("image", FieldType::Relationship)
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
    };

    {
        let mut reg = shared_registry.write().unwrap();
        reg.register_collection(media_def.clone());
        reg.register_collection(articles_def.clone());
    }

    let locale_config = LocaleConfig {
        default_locale: "en".to_string(),
        locales: vec!["en".to_string(), "de".to_string()],
        fallback: true,
    };
    migrate::sync_all(&pool, &shared_registry, &locale_config).expect("Sync failed");

    let locale_ctx = query::LocaleContext {
        mode: query::LocaleMode::Single("en".to_string()),
        config: locale_config.clone(),
    };

    // Create a media document with localized caption
    let mut conn = pool.get().expect("conn");
    let tx = conn.transaction().expect("tx");
    let mut media_data = HashMap::new();
    media_data.insert("url".to_string(), "/img/test.png".to_string());
    media_data.insert("caption".to_string(), "Test image".to_string());
    let media_doc = query::create(&tx, "media", &media_def, &media_data, Some(&locale_ctx))
        .expect("Create media");

    // Create an article referencing the media
    let mut article_data = HashMap::new();
    article_data.insert("title".to_string(), "My Article".to_string());
    article_data.insert("image".to_string(), media_doc.id.clone());
    let mut article = query::create(&tx, "articles", &articles_def, &article_data, None)
        .expect("Create article");
    tx.commit().expect("Commit");

    // Populate at depth 1 WITH locale_ctx — this used to fail with
    // "Failed to prepare find_by_ids query on 'media'" because the populate
    // code didn't forward locale_ctx to find_by_ids.
    let conn = pool.get().expect("conn");
    let mut visited = HashSet::new();
    query::populate_relationships(
        &query::PopulateContext { conn: &conn, registry: &shared_registry.read().unwrap(), collection_slug: "articles", def: &articles_def },
        &mut article, &mut visited,
        &query::PopulateOpts { depth: 1, select: None, locale_ctx: Some(&locale_ctx) },
    ).expect("Populate with localized related collection should succeed");

    // image should be populated as a full object
    let img = article.get("image").expect("image field should exist");
    assert!(img.is_object(), "image should be populated object, got: {:?}", img);
    assert_eq!(img.get("url").unwrap().as_str().unwrap(), "/img/test.png");
    assert_eq!(img.get("caption").unwrap().as_str().unwrap(), "Test image");
}

// ── 5. Global join table support (arrays, blocks, has-many) ───────────────

fn make_global_with_join_fields() -> GlobalDefinition {
    GlobalDefinition {
        slug: "homepage".to_string(),
        labels: CollectionLabels {
            singular: Some(LocalizedString::Plain("Homepage".to_string())),
            plural: None,
        },
        fields: vec![
            FieldDefinition {
                name: "title".to_string(),
                ..Default::default()
            },
            // Group field — expanded into sub-columns (same as collections)
            FieldDefinition {
                name: "seo".to_string(),
                field_type: FieldType::Group,
                fields: vec![
                    make_field("meta_title", FieldType::Text),
                    make_field("meta_description", FieldType::Textarea),
                ],
                ..Default::default()
            },
            // Array field — uses join table
            FieldDefinition {
                name: "links".to_string(),
                field_type: FieldType::Array,
                fields: vec![
                    make_field("url", FieldType::Text),
                    make_field("label", FieldType::Text),
                ],
                ..Default::default()
            },
            // Blocks field — uses join table
            FieldDefinition {
                name: "content".to_string(),
                field_type: FieldType::Blocks,
                blocks: vec![
                    BlockDefinition {
                        block_type: "paragraph".to_string(),
                        fields: vec![make_field("text", FieldType::Textarea)],
                        ..Default::default()
                    },
                    BlockDefinition {
                        block_type: "image".to_string(),
                        fields: vec![make_field("url", FieldType::Text)],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            // Has-many relationship — uses junction table
            FieldDefinition {
                name: "featured_posts".to_string(),
                field_type: FieldType::Relationship,
                relationship: Some(RelationshipConfig {
                    collection: "posts".to_string(),
                    has_many: true,
                    max_depth: None,
                    polymorphic: vec![],
                }),
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

fn setup_global_with_joins() -> (tempfile::TempDir, crap_cms::db::DbPool, GlobalDefinition) {
    let (_tmp, pool) = create_test_pool();
    let registry = Registry::shared();
    let def = make_global_with_join_fields();
    let posts_def = make_posts_def();
    {
        let mut reg = registry.write().unwrap();
        reg.register_global(def.clone());
        reg.register_collection(posts_def);
    }
    migrate::sync_all(&pool, &registry, &CrapConfig::default().locale).expect("Sync failed");
    (_tmp, pool, def)
}

/// Migration creates join tables for global array/blocks/has-many fields.
#[test]
fn global_migration_creates_join_tables() {
    let (_tmp, pool, _def) = setup_global_with_joins();
    let conn = pool.get().expect("DB connection");

    // Check that join tables exist
    let check = |table: &str| -> bool {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |row| row.get(0),
        ).unwrap();
        count > 0
    };

    assert!(check("_global_homepage"), "Parent table should exist");
    assert!(check("_global_homepage_links"), "Array join table should exist");
    assert!(check("_global_homepage_content"), "Blocks join table should exist");
    assert!(check("_global_homepage_featured_posts"), "Has-many junction table should exist");
}

/// Migration does NOT create parent columns for array/blocks/has-many fields,
/// but DOES create expanded sub-columns for group fields.
#[test]
fn global_migration_parent_table_columns() {
    let (_tmp, pool, _def) = setup_global_with_joins();
    let conn = pool.get().expect("DB connection");

    let mut stmt = conn.prepare("PRAGMA table_info(_global_homepage)").unwrap();
    let columns: HashSet<String> = stmt.query_map([], |row| {
        row.get::<_, String>(1)
    }).unwrap().filter_map(|r| r.ok()).collect();

    // Should have these columns
    assert!(columns.contains("id"), "Should have id column");
    assert!(columns.contains("title"), "Should have scalar field column");
    // Group fields are expanded into sub-columns (same as collections)
    assert!(columns.contains("seo__meta_title"), "Should have group sub-field column");
    assert!(columns.contains("seo__meta_description"), "Should have group sub-field column");
    assert!(!columns.contains("seo"), "Should NOT have single group column");
    assert!(columns.contains("created_at"), "Should have created_at");
    assert!(columns.contains("updated_at"), "Should have updated_at");

    // Should NOT have these columns (they use join tables)
    assert!(!columns.contains("links"), "Array field should NOT have parent column");
    assert!(!columns.contains("content"), "Blocks field should NOT have parent column");
    assert!(!columns.contains("featured_posts"), "Has-many field should NOT have parent column");
}

/// Global with array field: save and read back through join table.
#[test]
fn global_array_field_save_and_read() {
    let (_tmp, pool, def) = setup_global_with_joins();
    let links_field = def.fields.iter().find(|f| f.name == "links").unwrap();

    // Save array data via join table
    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");

    let rows = vec![
        {
            let mut m = HashMap::new();
            m.insert("url".to_string(), "https://example.com".to_string());
            m.insert("label".to_string(), "Example".to_string());
            m
        },
        {
            let mut m = HashMap::new();
            m.insert("url".to_string(), "https://rust-lang.org".to_string());
            m.insert("label".to_string(), "Rust".to_string());
            m
        },
    ];
    query::set_array_rows(&tx, "_global_homepage", "links", "default", &rows, &links_field.fields, None)
        .expect("Set array rows failed");
    tx.commit().expect("Commit");

    // Read back through get_global (which now calls hydrate_document)
    let conn = pool.get().expect("DB connection");
    let doc = query::get_global(&conn, "homepage", &def, None)
        .expect("Get global failed");

    let links = doc.get("links").expect("links should be populated");
    let links_arr = links.as_array().expect("links should be an array");
    assert_eq!(links_arr.len(), 2);
    assert_eq!(links_arr[0]["url"], "https://example.com");
    assert_eq!(links_arr[0]["label"], "Example");
    assert_eq!(links_arr[1]["url"], "https://rust-lang.org");
    assert_eq!(links_arr[1]["label"], "Rust");
}

/// Global with blocks field: save and read back through join table.
#[test]
fn global_blocks_field_save_and_read() {
    let (_tmp, pool, def) = setup_global_with_joins();

    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");

    let blocks = vec![
        serde_json::json!({"_block_type": "paragraph", "text": "Welcome to the homepage"}),
        serde_json::json!({"_block_type": "image", "url": "/hero.jpg"}),
    ];
    query::set_block_rows(&tx, "_global_homepage", "content", "default", &blocks, None)
        .expect("Set block rows failed");
    tx.commit().expect("Commit");

    // Read back
    let conn = pool.get().expect("DB connection");
    let doc = query::get_global(&conn, "homepage", &def, None)
        .expect("Get global failed");

    let content = doc.get("content").expect("content should be populated");
    let content_arr = content.as_array().expect("content should be an array");
    assert_eq!(content_arr.len(), 2);
    assert_eq!(content_arr[0]["_block_type"], "paragraph");
    assert_eq!(content_arr[0]["text"], "Welcome to the homepage");
    assert_eq!(content_arr[1]["_block_type"], "image");
    assert_eq!(content_arr[1]["url"], "/hero.jpg");
}

/// Global with has-many relationship: save and read back through junction table.
#[test]
fn global_has_many_field_save_and_read() {
    let (_tmp, pool, def) = setup_global_with_joins();

    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");

    let ids = vec!["post-1".to_string(), "post-2".to_string(), "post-3".to_string()];
    query::set_related_ids(&tx, "_global_homepage", "featured_posts", "default", &ids, None)
        .expect("Set related IDs failed");
    tx.commit().expect("Commit");

    // Read back
    let conn = pool.get().expect("DB connection");
    let doc = query::get_global(&conn, "homepage", &def, None)
        .expect("Get global failed");

    let posts = doc.get("featured_posts").expect("featured_posts should be populated");
    let posts_arr = posts.as_array().expect("featured_posts should be an array");
    assert_eq!(posts_arr.len(), 3);
    assert_eq!(posts_arr[0], "post-1");
    assert_eq!(posts_arr[1], "post-2");
    assert_eq!(posts_arr[2], "post-3");
}

/// save_join_table_data works with global table names (prefixed _global_).
#[test]
fn global_save_join_table_data() {
    let (_tmp, pool, def) = setup_global_with_joins();

    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");

    let mut join_data: HashMap<String, serde_json::Value> = HashMap::new();
    join_data.insert("links".to_string(), serde_json::json!([
        {"url": "https://a.com", "label": "A"},
    ]));
    join_data.insert("content".to_string(), serde_json::json!([
        {"_block_type": "paragraph", "text": "Hello"},
    ]));
    join_data.insert("featured_posts".to_string(), serde_json::json!(["p1", "p2"]));

    query::save_join_table_data(&tx, "_global_homepage", &def.fields, "default", &join_data, None)
        .expect("Save join table data failed");
    tx.commit().expect("Commit");

    // Verify everything via get_global (hydration)
    let conn = pool.get().expect("DB connection");
    let doc = query::get_global(&conn, "homepage", &def, None)
        .expect("Get global failed");

    let links = doc.get("links").unwrap().as_array().unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["url"], "https://a.com");

    let content = doc.get("content").unwrap().as_array().unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["_block_type"], "paragraph");

    let posts = doc.get("featured_posts").unwrap().as_array().unwrap();
    assert_eq!(posts.len(), 2);
    assert_eq!(posts[0], "p1");
    assert_eq!(posts[1], "p2");
}

/// Updating join table data replaces old data.
#[test]
fn global_join_table_data_replaces_on_update() {
    let (_tmp, pool, def) = setup_global_with_joins();

    let mut conn = pool.get().expect("DB connection");

    // First save
    {
        let tx = conn.transaction().expect("Start transaction");
        let mut join_data: HashMap<String, serde_json::Value> = HashMap::new();
        join_data.insert("links".to_string(), serde_json::json!([
            {"url": "https://old.com", "label": "Old"},
        ]));
        query::save_join_table_data(&tx, "_global_homepage", &def.fields, "default", &join_data, None)
            .expect("Save failed");
        tx.commit().expect("Commit");
    }

    // Second save — should replace
    {
        let tx = conn.transaction().expect("Start transaction");
        let mut join_data: HashMap<String, serde_json::Value> = HashMap::new();
        join_data.insert("links".to_string(), serde_json::json!([
            {"url": "https://new1.com", "label": "New 1"},
            {"url": "https://new2.com", "label": "New 2"},
        ]));
        query::save_join_table_data(&tx, "_global_homepage", &def.fields, "default", &join_data, None)
            .expect("Save failed");
        tx.commit().expect("Commit");
    }

    let conn2 = pool.get().expect("DB connection");
    let doc = query::get_global(&conn2, "homepage", &def, None).expect("Get failed");

    let links = doc.get("links").unwrap().as_array().unwrap();
    assert_eq!(links.len(), 2, "Old data should be replaced by new");
    assert_eq!(links[0]["url"], "https://new1.com");
    assert_eq!(links[1]["url"], "https://new2.com");
}

/// Group fields work correctly in globals using expanded sub-columns (same as collections).
#[test]
fn global_group_field_preserved() {
    let (_tmp, pool, def) = setup_global_with_joins();

    // Update global with group sub-field data (expanded columns)
    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");
    let mut data = HashMap::new();
    data.insert("title".to_string(), "My Homepage".to_string());
    data.insert("seo__meta_title".to_string(), "Home".to_string());
    data.insert("seo__meta_description".to_string(), "Welcome".to_string());
    query::update_global(&tx, "homepage", &def, &data, None)
        .expect("Update failed");
    tx.commit().expect("Commit");

    let conn = pool.get().expect("DB connection");
    let doc = query::get_global(&conn, "homepage", &def, None)
        .expect("Get global failed");

    assert_eq!(doc.get_str("title"), Some("My Homepage"));
    // Group field should be hydrated as a nested object from sub-columns
    let seo = doc.get("seo").expect("seo should be present");
    assert!(seo.is_object(), "seo should be an object (reconstructed from sub-columns)");
    assert_eq!(seo.get("meta_title").and_then(|v| v.as_str()), Some("Home"));
    assert_eq!(seo.get("meta_description").and_then(|v| v.as_str()), Some("Welcome"));
}

/// Global with mixed scalar, group, array, blocks, has-many fields all work together.
#[test]
fn global_mixed_fields_coexist() {
    let (_tmp, pool, def) = setup_global_with_joins();

    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");

    // Update scalar + group sub-field data
    let mut data = HashMap::new();
    data.insert("title".to_string(), "Homepage".to_string());
    data.insert("seo__meta_title".to_string(), "Home".to_string());
    query::update_global(&tx, "homepage", &def, &data, None)
        .expect("Update failed");

    // Save join table data
    let mut join_data: HashMap<String, serde_json::Value> = HashMap::new();
    join_data.insert("links".to_string(), serde_json::json!([
        {"url": "https://example.com", "label": "Link"},
    ]));
    join_data.insert("content".to_string(), serde_json::json!([
        {"_block_type": "paragraph", "text": "Hello world"},
    ]));
    join_data.insert("featured_posts".to_string(), serde_json::json!(["p1"]));
    query::save_join_table_data(&tx, "_global_homepage", &def.fields, "default", &join_data, None)
        .expect("Save join data failed");

    tx.commit().expect("Commit");

    // Read back — all fields should be populated
    let conn = pool.get().expect("DB connection");
    let doc = query::get_global(&conn, "homepage", &def, None)
        .expect("Get global failed");

    // Scalar
    assert_eq!(doc.get_str("title"), Some("Homepage"));

    // Group (reconstructed as nested object from sub-columns)
    let seo = doc.get("seo").expect("seo should exist");
    assert!(seo.is_object(), "seo should be an object");
    assert_eq!(seo.get("meta_title").and_then(|v| v.as_str()), Some("Home"));

    // Array
    let links = doc.get("links").unwrap().as_array().unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["url"], "https://example.com");

    // Blocks
    let content = doc.get("content").unwrap().as_array().unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["_block_type"], "paragraph");

    // Has-many
    let posts = doc.get("featured_posts").unwrap().as_array().unwrap();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0], "p1");
}

/// Empty arrays/blocks/has-many return empty JSON arrays after hydration.
#[test]
fn global_empty_join_data_returns_empty_arrays() {
    let (_tmp, pool, def) = setup_global_with_joins();

    let conn = pool.get().expect("DB connection");
    let doc = query::get_global(&conn, "homepage", &def, None)
        .expect("Get global failed");

    // All join-table fields should be empty arrays (hydrated but no data)
    let links = doc.get("links").expect("links should exist");
    assert_eq!(links.as_array().unwrap().len(), 0);

    let content = doc.get("content").expect("content should exist");
    assert_eq!(content.as_array().unwrap().len(), 0);

    let posts = doc.get("featured_posts").expect("featured_posts should exist");
    assert_eq!(posts.as_array().unwrap().len(), 0);
}

/// ALTER TABLE for existing globals adds new scalar columns.
#[test]
fn global_alter_table_adds_new_columns() {
    let (_tmp, pool) = create_test_pool();
    let registry = Registry::shared();

    // First sync: minimal global
    let def_v1 = GlobalDefinition {
        slug: "evolving".to_string(),
        labels: CollectionLabels::default(),
        fields: vec![
            make_field("name", FieldType::Text),
        ],
        hooks: CollectionHooks::default(),
        access: CollectionAccess::default(),
        mcp: Default::default(),
        live: None,
        versions: None,
    };
    {
        let mut reg = registry.write().unwrap();
        reg.register_global(def_v1.clone());
    }
    migrate::sync_all(&pool, &registry, &CrapConfig::default().locale).expect("Sync v1 failed");

    // Write data
    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");
    let mut data = HashMap::new();
    data.insert("name".to_string(), "Test".to_string());
    query::update_global(&tx, "evolving", &def_v1, &data, None).expect("Update v1 failed");
    tx.commit().expect("Commit");

    // Second sync: add a new field
    let def_v2 = GlobalDefinition {
        slug: "evolving".to_string(),
        labels: CollectionLabels::default(),
        fields: vec![
            make_field("name", FieldType::Text),
            make_field("description", FieldType::Textarea),
        ],
        hooks: CollectionHooks::default(),
        access: CollectionAccess::default(),
        mcp: Default::default(),
        live: None,
        versions: None,
    };
    {
        let mut reg = registry.write().unwrap();
        reg.globals.clear();
        reg.register_global(def_v2.clone());
    }
    migrate::sync_all(&pool, &registry, &CrapConfig::default().locale).expect("Sync v2 failed");

    // Old data should still be there, new column should exist
    let conn = pool.get().expect("DB connection");
    let doc = query::get_global(&conn, "evolving", &def_v2, None).expect("Get failed");
    assert_eq!(doc.get_str("name"), Some("Test"), "Old data should be preserved");
    // New column exists (NULL value for existing row)
    assert!(doc.fields.contains_key("description"), "New column should exist");
}

/// ALTER TABLE for existing globals adds join tables for new array fields.
#[test]
fn global_alter_table_adds_join_tables() {
    let (_tmp, pool) = create_test_pool();
    let registry = Registry::shared();

    // First sync: scalar-only global
    let def_v1 = GlobalDefinition {
        slug: "growing".to_string(),
        labels: CollectionLabels::default(),
        fields: vec![
            make_field("title", FieldType::Text),
        ],
        hooks: CollectionHooks::default(),
        access: CollectionAccess::default(),
        mcp: Default::default(),
        live: None,
        versions: None,
    };
    {
        let mut reg = registry.write().unwrap();
        reg.register_global(def_v1);
    }
    migrate::sync_all(&pool, &registry, &CrapConfig::default().locale).expect("Sync v1 failed");

    // Second sync: add array field
    let def_v2 = GlobalDefinition {
        slug: "growing".to_string(),
        labels: CollectionLabels::default(),
        fields: vec![
            make_field("title", FieldType::Text),
            FieldDefinition {
                name: "items".to_string(),
                field_type: FieldType::Array,
                fields: vec![
                    make_field("label", FieldType::Text),
                ],
                ..Default::default()
            },
        ],
        hooks: CollectionHooks::default(),
        access: CollectionAccess::default(),
        mcp: Default::default(),
        live: None,
        versions: None,
    };
    {
        let mut reg = registry.write().unwrap();
        reg.globals.clear();
        reg.register_global(def_v2.clone());
    }
    migrate::sync_all(&pool, &registry, &CrapConfig::default().locale).expect("Sync v2 failed");

    // Join table should exist
    let conn = pool.get().expect("DB connection");
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='_global_growing_items'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(count, 1, "Array join table should be created on ALTER");

    // Save and read back array data
    let mut conn2 = pool.get().expect("DB connection");
    let tx = conn2.transaction().expect("Start transaction");
    let items_field = def_v2.fields.iter().find(|f| f.name == "items").unwrap();
    let rows = vec![{
        let mut m = HashMap::new();
        m.insert("label".to_string(), "First".to_string());
        m
    }];
    query::set_array_rows(&tx, "_global_growing", "items", "default", &rows, &items_field.fields, None)
        .expect("Set array rows failed");
    tx.commit().expect("Commit");

    let conn3 = pool.get().expect("DB connection");
    let doc = query::get_global(&conn3, "growing", &def_v2, None).expect("Get failed");
    let items = doc.get("items").unwrap().as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["label"], "First");
}

/// hydrate_document Group guard: when a global stores groups as single JSON columns,
/// hydrate_document must NOT attempt to reconstruct from __-prefixed sub-columns.
#[test]
fn hydrate_document_skips_group_reconstruction_for_globals() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE _global_test (
            id TEXT PRIMARY KEY,
            title TEXT,
            seo TEXT,
            created_at TEXT,
            updated_at TEXT
        );
        INSERT INTO _global_test (id, title, seo, created_at, updated_at)
        VALUES ('default', 'Test', '{\"meta_title\":\"Hello\"}', '2024-01-01', '2024-01-01');"
    ).unwrap();

    let fields = vec![
        make_field("title", FieldType::Text),
        FieldDefinition {
            name: "seo".to_string(),
            field_type: FieldType::Group,
            fields: vec![
                make_field("meta_title", FieldType::Text),
            ],
            ..Default::default()
        },
    ];

    // Simulate what get_global does: read the row, then hydrate
    let mut doc = conn.query_row(
        "SELECT id, title, seo, created_at, updated_at FROM _global_test WHERE id = 'default'",
        [],
        |row| {
            crap_cms::db::document::row_to_document(row, &[
                "id".to_string(), "title".to_string(), "seo".to_string(),
                "created_at".to_string(), "updated_at".to_string(),
            ])
        },
    ).unwrap();

    // Hydrate should NOT touch the group field (no seo__meta_title sub-column exists)
    query::hydrate_document(&conn, "_global_test", &fields, &mut doc, None, None).unwrap();

    // Group field should still be the raw JSON string, NOT reconstructed
    assert_eq!(doc.get_str("seo"), Some("{\"meta_title\":\"Hello\"}"));
    assert_eq!(doc.get_str("title"), Some("Test"));
}

/// hydrate_document Group reconstruction still works for collections
/// (where __-prefixed sub-columns DO exist).
#[test]
fn hydrate_document_reconstructs_group_for_collections() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE pages (
            id TEXT PRIMARY KEY,
            title TEXT,
            seo__meta_title TEXT,
            seo__meta_description TEXT,
            created_at TEXT,
            updated_at TEXT
        );
        INSERT INTO pages (id, title, seo__meta_title, seo__meta_description, created_at, updated_at)
        VALUES ('p1', 'Page', 'Page Title', 'Page Desc', '2024-01-01', '2024-01-01');"
    ).unwrap();

    let fields = vec![
        make_field("title", FieldType::Text),
        FieldDefinition {
            name: "seo".to_string(),
            field_type: FieldType::Group,
            fields: vec![
                make_field("meta_title", FieldType::Text),
                make_field("meta_description", FieldType::Textarea),
            ],
            ..Default::default()
        },
    ];

    let mut doc = conn.query_row(
        "SELECT id, title, seo__meta_title, seo__meta_description, created_at, updated_at FROM pages WHERE id = 'p1'",
        [],
        |row| {
            crap_cms::db::document::row_to_document(row, &[
                "id".to_string(), "title".to_string(),
                "seo__meta_title".to_string(), "seo__meta_description".to_string(),
                "created_at".to_string(), "updated_at".to_string(),
            ])
        },
    ).unwrap();

    // Before hydration: sub-columns are separate keys
    assert!(doc.fields.contains_key("seo__meta_title"));

    query::hydrate_document(&conn, "pages", &fields, &mut doc, None, None).unwrap();

    // After hydration: reconstructed into nested object
    assert!(!doc.fields.contains_key("seo__meta_title"), "Sub-column should be removed");
    let seo = doc.get("seo").expect("seo should exist");
    let seo_obj = seo.as_object().expect("seo should be an object");
    assert_eq!(seo_obj.get("meta_title").unwrap(), "Page Title");
    assert_eq!(seo_obj.get("meta_description").unwrap(), "Page Desc");
}

/// update_global skips join-table fields (no column for them in parent table).
#[test]
fn global_update_ignores_join_table_field_values() {
    let (_tmp, pool, def) = setup_global_with_joins();

    let mut conn = pool.get().expect("DB connection");
    let tx = conn.transaction().expect("Start transaction");

    // Include both scalar data and array/blocks data in the update map.
    // The array/blocks values should be ignored by update_global (no parent column).
    let mut data = HashMap::new();
    data.insert("title".to_string(), "My Title".to_string());
    // These should not cause SQL errors even though no column exists:
    data.insert("links".to_string(), "should be ignored".to_string());
    data.insert("content".to_string(), "should be ignored".to_string());
    data.insert("featured_posts".to_string(), "should be ignored".to_string());

    let doc = query::update_global(&tx, "homepage", &def, &data, None)
        .expect("Update should succeed despite join-table field values in data");
    tx.commit().expect("Commit");

    assert_eq!(doc.get_str("title"), Some("My Title"));
}

// ── ALTER TABLE Group Field Tests ─────────────────────────────────────────────

/// Collection ALTER TABLE: adding a group field creates sub-columns.
#[test]
fn collection_alter_adds_group_sub_columns() {
    let (_tmp, pool) = create_test_pool();
    let registry = Registry::shared();

    // First sync: simple collection
    let mut def = CollectionDefinition {
        slug: "articles".to_string(),
        labels: CollectionLabels::default(),
        timestamps: true,
        fields: vec![
            make_field("title", FieldType::Text),
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
    };
    {
        let mut reg = registry.write().unwrap();
        reg.register_collection(def.clone());
    }
    migrate::sync_all(&pool, &registry, &CrapConfig::default().locale).expect("Sync v1");

    // Write initial data
    let conn = pool.get().unwrap();
    let mut data = HashMap::new();
    data.insert("title".to_string(), "My Article".to_string());
    let doc = query::create(&conn, "articles", &def, &data, None).expect("Create");

    // Second sync: add a group field
    def.fields.push(FieldDefinition {
        name: "seo".to_string(),
        field_type: FieldType::Group,
        fields: vec![
            make_field("meta_title", FieldType::Text),
            make_field("meta_description", FieldType::Textarea),
        ],
        ..Default::default()
    });
    {
        let mut reg = registry.write().unwrap();
        reg.register_collection(def.clone());
    }
    migrate::sync_all(&pool, &registry, &CrapConfig::default().locale).expect("Sync v2");

    // Verify sub-columns exist
    let mut stmt = conn.prepare("PRAGMA table_info(articles)").unwrap();
    let columns: HashSet<String> = stmt.query_map([], |row| {
        row.get::<_, String>(1)
    }).unwrap().filter_map(|r| r.ok()).collect();

    assert!(columns.contains("seo__meta_title"), "Should have seo__meta_title sub-column");
    assert!(columns.contains("seo__meta_description"), "Should have seo__meta_description sub-column");
    assert!(!columns.contains("seo"), "Should NOT have single seo column");

    // Old data preserved, new sub-columns are NULL
    let old_doc = query::find_by_id(&conn, "articles", &def, &doc.id, None).unwrap().unwrap();
    assert_eq!(old_doc.get_str("title"), Some("My Article"));

    // Write new data with group sub-fields
    let mut new_data = HashMap::new();
    new_data.insert("seo__meta_title".to_string(), "SEO Title".to_string());
    new_data.insert("seo__meta_description".to_string(), "SEO Desc".to_string());
    query::update(&conn, "articles", &def, &doc.id, &new_data, None).expect("Update");

    let updated = query::find_by_id(&conn, "articles", &def, &doc.id, None).unwrap().unwrap();
    let seo = updated.fields.get("seo").expect("seo should exist after hydration");
    assert_eq!(seo.get("meta_title").and_then(|v| v.as_str()), Some("SEO Title"));
    assert_eq!(seo.get("meta_description").and_then(|v| v.as_str()), Some("SEO Desc"));
    assert_eq!(updated.get_str("title"), Some("My Article"), "Old data preserved");
}

/// Global ALTER TABLE: adding a group field creates sub-columns.
#[test]
fn global_alter_adds_group_sub_columns() {
    let (_tmp, pool) = create_test_pool();
    let registry = Registry::shared();

    // First sync: simple global
    let def_v1 = GlobalDefinition {
        slug: "settings".to_string(),
        labels: CollectionLabels::default(),
        fields: vec![
            make_field("site_name", FieldType::Text),
        ],
        hooks: CollectionHooks::default(),
        access: CollectionAccess::default(),
        mcp: Default::default(),
        live: None,
        versions: None,
    };
    {
        let mut reg = registry.write().unwrap();
        reg.register_global(def_v1.clone());
    }
    migrate::sync_all(&pool, &registry, &CrapConfig::default().locale).expect("Sync v1");

    // Write initial data
    let conn = pool.get().unwrap();
    let mut data = HashMap::new();
    data.insert("site_name".to_string(), "My Site".to_string());
    query::update_global(&conn, "settings", &def_v1, &data, None).expect("Update v1");

    // Second sync: add a group field
    let def_v2 = GlobalDefinition {
        slug: "settings".to_string(),
        labels: CollectionLabels::default(),
        fields: vec![
            make_field("site_name", FieldType::Text),
            FieldDefinition {
                name: "seo".to_string(),
                field_type: FieldType::Group,
                fields: vec![
                    make_field("meta_title", FieldType::Text),
                    make_field("og_image", FieldType::Text),
                ],
                ..Default::default()
            },
        ],
        hooks: CollectionHooks::default(),
        access: CollectionAccess::default(),
        mcp: Default::default(),
        live: None,
        versions: None,
    };
    {
        let mut reg = registry.write().unwrap();
        reg.globals.clear();
        reg.register_global(def_v2.clone());
    }
    migrate::sync_all(&pool, &registry, &CrapConfig::default().locale).expect("Sync v2");

    // Verify sub-columns exist
    let mut stmt = conn.prepare("PRAGMA table_info(_global_settings)").unwrap();
    let columns: HashSet<String> = stmt.query_map([], |row| {
        row.get::<_, String>(1)
    }).unwrap().filter_map(|r| r.ok()).collect();

    assert!(columns.contains("seo__meta_title"), "Should have seo__meta_title sub-column");
    assert!(columns.contains("seo__og_image"), "Should have seo__og_image sub-column");
    assert!(!columns.contains("seo"), "Should NOT have single seo column");

    // Old data preserved
    let doc = query::get_global(&conn, "settings", &def_v2, None).expect("Get");
    assert_eq!(doc.get_str("site_name"), Some("My Site"), "Old data preserved");

    // Write group data
    let mut new_data = HashMap::new();
    new_data.insert("seo__meta_title".to_string(), "Global SEO".to_string());
    new_data.insert("seo__og_image".to_string(), "/og.png".to_string());
    query::update_global(&conn, "settings", &def_v2, &new_data, None).expect("Update v2");

    let updated = query::get_global(&conn, "settings", &def_v2, None).expect("Get v2");
    let seo = updated.fields.get("seo").expect("seo should exist after hydration");
    assert_eq!(seo.get("meta_title").and_then(|v| v.as_str()), Some("Global SEO"));
    assert_eq!(seo.get("og_image").and_then(|v| v.as_str()), Some("/og.png"));
}

/// Collection ALTER TABLE: adding localized group sub-fields creates locale columns.
#[test]
fn collection_alter_adds_localized_group_columns() {
    let (_tmp, pool) = create_test_pool();
    let registry = Registry::shared();
    let lc = LocaleConfig {
        default_locale: "en".to_string(),
        locales: vec!["en".to_string(), "de".to_string()],
        fallback: true,
    };

    // First sync: collection with non-localized group
    let mut def = CollectionDefinition {
        slug: "pages_alter".to_string(),
        labels: CollectionLabels::default(),
        timestamps: true,
        fields: vec![
            make_field("title", FieldType::Text),
            FieldDefinition {
                name: "seo".to_string(),
                field_type: FieldType::Group,
                fields: vec![
                    make_field("meta_title", FieldType::Text),
                ],
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
    };
    {
        let mut reg = registry.write().unwrap();
        reg.register_collection(def.clone());
    }
    migrate::sync_all(&pool, &registry, &lc).expect("Sync v1");

    // Verify non-localized sub-column
    let conn = pool.get().unwrap();
    let mut stmt = conn.prepare("PRAGMA table_info(pages_alter)").unwrap();
    let columns_v1: HashSet<String> = stmt.query_map([], |row| {
        row.get::<_, String>(1)
    }).unwrap().filter_map(|r| r.ok()).collect();
    assert!(columns_v1.contains("seo__meta_title"), "Non-localized sub-column should exist");
    assert!(!columns_v1.contains("seo__meta_title__en"), "Locale columns should not exist yet");

    // Second sync: add a new localized sub-field to the group
    def.fields[1].fields.push(FieldDefinition {
        name: "og_description".to_string(),
        localized: true,
        ..Default::default()
    });
    {
        let mut reg = registry.write().unwrap();
        reg.register_collection(def.clone());
    }
    migrate::sync_all(&pool, &registry, &lc).expect("Sync v2");

    // Verify new locale columns were added
    let mut stmt2 = conn.prepare("PRAGMA table_info(pages_alter)").unwrap();
    let columns_v2: HashSet<String> = stmt2.query_map([], |row| {
        row.get::<_, String>(1)
    }).unwrap().filter_map(|r| r.ok()).collect();

    assert!(columns_v2.contains("seo__meta_title"), "Original sub-column preserved");
    assert!(columns_v2.contains("seo__og_description__en"), "EN locale column added");
    assert!(columns_v2.contains("seo__og_description__de"), "DE locale column added");
    assert!(!columns_v2.contains("seo__og_description"), "Non-localized column should NOT exist");
}
