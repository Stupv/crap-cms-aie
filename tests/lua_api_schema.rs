use std::path::PathBuf;

use crap_cms::config::CrapConfig;
use crap_cms::db::DbPool;
use crap_cms::core::SharedRegistry;
use crap_cms::hooks;
use crap_cms::hooks::lifecycle::HookRunner;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hook_tests")
}

fn setup_lua() -> HookRunner {
    let config_dir = fixture_dir();
    let config = CrapConfig::default();
    let registry = hooks::init_lua(&config_dir, &config).expect("init_lua failed");
    HookRunner::new(&config_dir, registry, &config).expect("HookRunner::new failed")
}

/// Helper to eval Lua code and get a string result (no DB connection needed for pure functions).
/// This uses a temporary in-memory DB for the eval.
fn eval_lua(runner: &HookRunner, code: &str) -> String {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut config = CrapConfig::default();
    config.database.path = "test.db".to_string();
    let pool = crap_cms::db::pool::create_pool(tmp.path(), &config).expect("pool");
    let conn = pool.get().expect("conn");
    runner.eval_lua_with_conn(code, &conn, None).expect("eval failed")
}

/// Set up a HookRunner with a real synced database (tables created from Lua definitions).
/// Returns (tempdir, pool, registry, runner). The tempdir must be kept alive for the DB.
#[allow(dead_code)]
fn setup_with_db() -> (tempfile::TempDir, DbPool, SharedRegistry, HookRunner) {
    let config_dir = fixture_dir();
    let config = CrapConfig::default();
    let registry = hooks::init_lua(&config_dir, &config).expect("init_lua failed");

    // Create a pool and sync tables from Lua-defined collections/globals
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut db_config = CrapConfig::default();
    db_config.database.path = "test.db".to_string();
    let pool = crap_cms::db::pool::create_pool(tmp.path(), &db_config).expect("pool");
    crap_cms::db::migrate::sync_all(&pool, &registry, &config.locale).expect("sync failed");

    let runner = HookRunner::new(&config_dir, registry.clone(), &config)
        .expect("HookRunner::new failed");
    (tmp, pool, registry, runner)
}

/// Helper to eval Lua code with a real synced DB connection. CRUD functions work here.
#[allow(dead_code)]
fn eval_lua_db(runner: &HookRunner, pool: &DbPool, code: &str) -> String {
    let conn = pool.get().expect("conn");
    runner.eval_lua_with_conn(code, &conn, None).expect("eval failed")
}

// ── 3D. Definition Parsing Edge Cases ────────────────────────────────────────

#[test]
fn parse_collection_minimal() {
    let config_dir = fixture_dir();
    let config = CrapConfig::default();
    let registry = hooks::init_lua(&config_dir, &config).expect("init_lua failed");

    let reg = registry.read().unwrap();
    let def = reg.get_collection("articles").expect("articles should be registered");
    assert_eq!(def.slug, "articles");
    assert!(!def.fields.is_empty());
}

#[test]
fn parse_collection_with_all_field_types() {
    // Use a temp dir with a custom collection that has all field types
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("everything.lua"),
        r#"
crap.collections.define("everything", {
    fields = {
        { name = "text_field", type = "text" },
        { name = "num_field", type = "number" },
        { name = "email_field", type = "email" },
        { name = "textarea_field", type = "textarea" },
        { name = "select_field", type = "select", options = {
            { label = "A", value = "a" },
            { label = "B", value = "b" },
        }},
        { name = "checkbox_field", type = "checkbox" },
        { name = "date_field", type = "date" },
        { name = "json_field", type = "json" },
        { name = "richtext_field", type = "richtext" },
        { name = "group_field", type = "group", fields = {
            { name = "sub1", type = "text" },
            { name = "sub2", type = "number" },
        }},
    },
})
        "#,
    ).unwrap();

    // Create empty init.lua
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = hooks::init_lua(tmp.path(), &config).expect("init_lua failed");

    let reg = registry.read().unwrap();
    let def = reg.get_collection("everything").expect("everything should be registered");
    assert_eq!(def.fields.len(), 10);

    // Verify types
    let field_types: Vec<&str> = def.fields.iter().map(|f| f.field_type.as_str()).collect();
    assert!(field_types.contains(&"text"));
    assert!(field_types.contains(&"number"));
    assert!(field_types.contains(&"email"));
    assert!(field_types.contains(&"select"));
    assert!(field_types.contains(&"checkbox"));
    assert!(field_types.contains(&"group"));

    // Verify group sub-fields
    let group = def.fields.iter().find(|f| f.name == "group_field").unwrap();
    assert_eq!(group.fields.len(), 2);
}

#[test]
fn parse_auth_config_true() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("users.lua"),
        r#"
crap.collections.define("users", {
    auth = true,
    fields = {
        { name = "name", type = "text" },
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = hooks::init_lua(tmp.path(), &config).expect("init_lua failed");

    let reg = registry.read().unwrap();
    let def = reg.get_collection("users").expect("users should be registered");
    assert!(def.is_auth_collection(), "should be auth collection");
    // Email field should have been auto-injected
    assert!(
        def.fields.iter().any(|f| f.name == "email" && f.field_type == crap_cms::core::field::FieldType::Email),
        "email field should be auto-injected for auth collections"
    );
}

#[test]
fn parse_auth_config_table() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("members.lua"),
        r#"
crap.collections.define("members", {
    auth = {
        verify_email = true,
        forgot_password = false,
    },
    fields = {
        { name = "role", type = "text" },
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = hooks::init_lua(tmp.path(), &config).expect("init_lua failed");

    let reg = registry.read().unwrap();
    let def = reg.get_collection("members").expect("members should be registered");
    assert!(def.is_auth_collection());
    let auth = def.auth.as_ref().unwrap();
    assert!(auth.verify_email);
    assert!(!auth.forgot_password);
}

#[test]
fn parse_global_definition() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let globals_dir = tmp.path().join("globals");
    std::fs::create_dir_all(&globals_dir).unwrap();

    std::fs::write(
        globals_dir.join("settings.lua"),
        r#"
crap.globals.define("settings", {
    labels = { singular = "Settings" },
    fields = {
        { name = "site_name", type = "text" },
        { name = "maintenance_mode", type = "checkbox" },
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = hooks::init_lua(tmp.path(), &config).expect("init_lua failed");

    let reg = registry.read().unwrap();
    let def = reg.get_global("settings").expect("settings should be registered");
    assert_eq!(def.slug, "settings");
    assert_eq!(def.fields.len(), 2);
    assert_eq!(def.fields[0].name, "site_name");
    assert_eq!(def.fields[1].name, "maintenance_mode");
}

// ── 4B. Collection Definition Parsing ────────────────────────────────────────

#[test]
fn parse_upload_config() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("media.lua"),
        r#"
crap.collections.define("media", {
    upload = {
        mime_types = { "image/*", "application/pdf" },
        max_file_size = 10485760,
        image_sizes = {
            { name = "thumbnail", width = 300, height = 300, fit = "cover" },
            { name = "card", width = 640, height = 480 },
        },
        format_options = {
            webp = { quality = 80 },
        },
    },
    fields = {
        { name = "alt", type = "text" },
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = crap_cms::hooks::init_lua(tmp.path(), &config).expect("init_lua");
    let reg = registry.read().unwrap();
    let def = reg.get_collection("media").expect("media should be registered");
    assert!(def.is_upload_collection());
    let upload = def.upload.as_ref().unwrap();
    assert_eq!(upload.mime_types.len(), 2);
    assert_eq!(upload.max_file_size, Some(10485760));
    assert_eq!(upload.image_sizes.len(), 2);
    assert_eq!(upload.image_sizes[0].name, "thumbnail");
    assert_eq!(upload.image_sizes[0].width, 300);
    assert!(upload.format_options.webp.is_some());
}

#[test]
fn parse_auth_strategies() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("users.lua"),
        r#"
crap.collections.define("users", {
    auth = {
        strategies = {
            { name = "api_key", authenticate = "hooks.auth.api_key" },
            { name = "oauth", authenticate = "hooks.auth.oauth" },
        },
    },
    fields = {
        { name = "name", type = "text" },
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = crap_cms::hooks::init_lua(tmp.path(), &config).expect("init_lua");
    let reg = registry.read().unwrap();
    let def = reg.get_collection("users").expect("users should be registered");
    assert!(def.is_auth_collection());
    let auth = def.auth.as_ref().unwrap();
    assert_eq!(auth.strategies.len(), 2);
    assert_eq!(auth.strategies[0].name, "api_key");
    assert_eq!(auth.strategies[0].authenticate, "hooks.auth.api_key");
    assert_eq!(auth.strategies[1].name, "oauth");
}

#[test]
fn parse_live_setting_function() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("events.lua"),
        r#"
crap.collections.define("events", {
    live = "hooks.live.filter",
    fields = {
        { name = "name", type = "text" },
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = crap_cms::hooks::init_lua(tmp.path(), &config).expect("init_lua");
    let reg = registry.read().unwrap();
    let def = reg.get_collection("events").expect("events should be registered");
    match &def.live {
        Some(crap_cms::core::collection::LiveSetting::Function(f)) => {
            assert_eq!(f, "hooks.live.filter");
        }
        other => panic!("Expected LiveSetting::Function, got {:?}", other),
    }
}

#[test]
fn parse_live_setting_disabled() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("private.lua"),
        r#"
crap.collections.define("private", {
    live = false,
    fields = {
        { name = "secret", type = "text" },
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = crap_cms::hooks::init_lua(tmp.path(), &config).expect("init_lua");
    let reg = registry.read().unwrap();
    let def = reg.get_collection("private").expect("private should be registered");
    assert!(matches!(&def.live, Some(crap_cms::core::collection::LiveSetting::Disabled)));
}

#[test]
fn parse_blocks_definition() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("pages.lua"),
        r#"
crap.collections.define("pages", {
    fields = {
        { name = "title", type = "text", required = true },
        { name = "content", type = "blocks", blocks = {
            { type = "text", label = "Text Block", fields = {
                { name = "body", type = "richtext" },
            }},
            { type = "image", label = "Image Block", fields = {
                { name = "src", type = "text" },
                { name = "alt", type = "text" },
            }},
        }},
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = crap_cms::hooks::init_lua(tmp.path(), &config).expect("init_lua");
    let reg = registry.read().unwrap();
    let def = reg.get_collection("pages").expect("pages should be registered");
    let blocks_field = def.fields.iter().find(|f| f.name == "content").expect("content field");
    assert_eq!(blocks_field.field_type, crap_cms::core::field::FieldType::Blocks);
    assert_eq!(blocks_field.blocks.len(), 2);
    assert_eq!(blocks_field.blocks[0].block_type, "text");
    assert_eq!(blocks_field.blocks[0].fields.len(), 1);
    assert_eq!(blocks_field.blocks[1].block_type, "image");
    assert_eq!(blocks_field.blocks[1].fields.len(), 2);
}

#[test]
fn parse_select_options_localized() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("polls.lua"),
        r#"
crap.collections.define("polls", {
    fields = {
        { name = "answer", type = "select", options = {
            { label = { en = "Yes", de = "Ja" }, value = "yes" },
            { label = { en = "No", de = "Nein" }, value = "no" },
        }},
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = crap_cms::hooks::init_lua(tmp.path(), &config).expect("init_lua");
    let reg = registry.read().unwrap();
    let def = reg.get_collection("polls").expect("polls should be registered");
    let answer_field = def.fields.iter().find(|f| f.name == "answer").expect("answer field");
    assert_eq!(answer_field.options.len(), 2);
    assert_eq!(answer_field.options[0].value, "yes");
    // The label should be a LocalizedString::Localized
    match &answer_field.options[0].label {
        crap_cms::core::field::LocalizedString::Localized(map) => {
            assert_eq!(map.get("en"), Some(&"Yes".to_string()));
            assert_eq!(map.get("de"), Some(&"Ja".to_string()));
        }
        crap_cms::core::field::LocalizedString::Plain(s) => {
            // Some implementations may flatten it — that's also acceptable
            assert!(!s.is_empty(), "Should have a non-empty label");
        }
    }
}

#[test]
fn parse_localized_label() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("articles.lua"),
        r#"
crap.collections.define("articles", {
    labels = {
        singular = { en = "Article", de = "Artikel" },
        plural = { en = "Articles", de = "Artikel" },
    },
    fields = {
        { name = "title", type = "text" },
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = crap_cms::hooks::init_lua(tmp.path(), &config).expect("init_lua");
    let reg = registry.read().unwrap();
    let def = reg.get_collection("articles").expect("articles should be registered");

    // Test resolving for different locales
    assert_eq!(def.singular_name_for("en", "en"), "Article");
    assert_eq!(def.singular_name_for("de", "en"), "Artikel");
    assert_eq!(def.display_name_for("en", "en"), "Articles");
}

// ── 5A. Versions Config Parsing ─────────────────────────────────────────────

#[test]
fn parse_versions_config_boolean_true() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("docs.lua"),
        r#"
crap.collections.define("docs", {
    versions = true,
    fields = {
        { name = "title", type = "text" },
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = crap_cms::hooks::init_lua(tmp.path(), &config).expect("init_lua");
    let reg = registry.read().unwrap();
    let def = reg.get_collection("docs").expect("docs should be registered");
    assert!(def.has_versions(), "versions=true should enable versions");
    assert!(def.has_drafts(), "versions=true enables drafts by default (PayloadCMS convention)");
    let vc = def.versions.as_ref().unwrap();
    assert!(vc.drafts);
    assert_eq!(vc.max_versions, 0);
}

#[test]
fn parse_versions_config_table_with_drafts() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("posts.lua"),
        r#"
crap.collections.define("posts", {
    versions = {
        drafts = true,
        max_versions = 50,
    },
    fields = {
        { name = "title", type = "text" },
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = crap_cms::hooks::init_lua(tmp.path(), &config).expect("init_lua");
    let reg = registry.read().unwrap();
    let def = reg.get_collection("posts").expect("posts should be registered");
    assert!(def.has_versions(), "should have versions");
    assert!(def.has_drafts(), "should have drafts");
    let vc = def.versions.as_ref().unwrap();
    assert!(vc.drafts);
    assert_eq!(vc.max_versions, 50);
}

#[test]
fn parse_versions_config_false() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("notes.lua"),
        r#"
crap.collections.define("notes", {
    versions = false,
    fields = {
        { name = "body", type = "textarea" },
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = crap_cms::hooks::init_lua(tmp.path(), &config).expect("init_lua");
    let reg = registry.read().unwrap();
    let def = reg.get_collection("notes").expect("notes should be registered");
    assert!(!def.has_versions(), "versions=false should not enable versions");
    assert!(def.versions.is_none());
}

#[test]
fn parse_versions_config_omitted() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("plain.lua"),
        r#"
crap.collections.define("plain", {
    fields = {
        { name = "name", type = "text" },
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = crap_cms::hooks::init_lua(tmp.path(), &config).expect("init_lua");
    let reg = registry.read().unwrap();
    let def = reg.get_collection("plain").expect("plain should be registered");
    assert!(!def.has_versions(), "no versions config should mean no versions");
    assert!(def.versions.is_none());
}

// ══════════════════════════════════════════════════════════════════════════════
// NEW FEATURES: crap.crypto
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn crypto_sha256() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local hash = crap.crypto.sha256("hello")
        -- Known SHA-256 of "hello"
        if hash ~= "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824" then
            return "WRONG:" .. hash
        end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn crypto_sha256_empty() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local hash = crap.crypto.sha256("")
        -- Known SHA-256 of empty string
        if hash ~= "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" then
            return "WRONG:" .. hash
        end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn crypto_hmac_sha256() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local mac = crap.crypto.hmac_sha256("hello", "secret-key")
        -- Should be 64 hex characters (32 bytes)
        if #mac ~= 64 then return "LEN:" .. #mac end
        -- Should be deterministic
        local mac2 = crap.crypto.hmac_sha256("hello", "secret-key")
        if mac ~= mac2 then return "NOT_DETERMINISTIC" end
        -- Different key should give different result
        local mac3 = crap.crypto.hmac_sha256("hello", "other-key")
        if mac == mac3 then return "SAME_WITH_DIFF_KEY" end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn crypto_base64_roundtrip() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local original = "Hello, World! 123 Special chars: @#$%"
        local encoded = crap.crypto.base64_encode(original)
        local decoded = crap.crypto.base64_decode(encoded)
        if decoded ~= original then
            return "MISMATCH:" .. decoded
        end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn crypto_base64_known_value() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local encoded = crap.crypto.base64_encode("hello")
        if encoded ~= "aGVsbG8=" then return "WRONG:" .. encoded end
        local decoded = crap.crypto.base64_decode("aGVsbG8=")
        if decoded ~= "hello" then return "DECODE_WRONG:" .. decoded end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn crypto_encrypt_decrypt_roundtrip() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local plaintext = "secret message 123!"
        local encrypted = crap.crypto.encrypt(plaintext)
        -- Encrypted should be different from plaintext
        if encrypted == plaintext then return "NOT_ENCRYPTED" end
        -- Should be base64 encoded
        if #encrypted < #plaintext then return "TOO_SHORT" end

        local decrypted = crap.crypto.decrypt(encrypted)
        if decrypted ~= plaintext then
            return "MISMATCH:" .. decrypted
        end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn crypto_encrypt_produces_different_ciphertexts() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        -- Same plaintext should produce different ciphertexts (random nonce)
        local a = crap.crypto.encrypt("same text")
        local b = crap.crypto.encrypt("same text")
        if a == b then return "SAME_CIPHERTEXT" end
        -- But both should decrypt to the same thing
        if crap.crypto.decrypt(a) ~= "same text" then return "A_WRONG" end
        if crap.crypto.decrypt(b) ~= "same text" then return "B_WRONG" end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn crypto_decrypt_invalid_input() {
    let runner = setup_lua();
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut config = CrapConfig::default();
    config.database.path = "test.db".to_string();
    let pool = crap_cms::db::pool::create_pool(tmp.path(), &config).expect("pool");
    let conn = pool.get().expect("conn");
    let result = runner.eval_lua_with_conn(r#"
        local ok, err = pcall(function()
            crap.crypto.decrypt("not-valid-base64!@#$")
        end)
        if ok then return "SHOULD_HAVE_FAILED" end
        return "ok"
    "#, &conn, None).expect("eval");
    assert_eq!(result, "ok");
}

#[test]
fn crypto_random_bytes() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local hex = crap.crypto.random_bytes(16)
        -- 16 bytes = 32 hex characters
        if #hex ~= 32 then return "LEN:" .. #hex end
        -- Should be hex (only 0-9a-f)
        if hex:find("[^0-9a-f]") then return "NOT_HEX:" .. hex end
        -- Two calls should produce different results
        local hex2 = crap.crypto.random_bytes(16)
        if hex == hex2 then return "SAME" end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn crypto_random_bytes_various_sizes() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local h1 = crap.crypto.random_bytes(1)
        if #h1 ~= 2 then return "1B:" .. #h1 end
        local h32 = crap.crypto.random_bytes(32)
        if #h32 ~= 64 then return "32B:" .. #h32 end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

// ══════════════════════════════════════════════════════════════════════════════
// NEW FEATURES: crap.schema
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn schema_get_collection() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local schema = crap.schema.get_collection("articles")
        if schema == nil then return "NIL" end
        if schema.slug ~= "articles" then return "SLUG:" .. tostring(schema.slug) end
        if schema.labels.singular ~= "Article" then return "SINGULAR:" .. tostring(schema.labels.singular) end
        if schema.labels.plural ~= "Articles" then return "PLURAL:" .. tostring(schema.labels.plural) end
        if #schema.fields < 1 then return "NO_FIELDS" end
        -- Check first field
        local title_field = nil
        for _, f in ipairs(schema.fields) do
            if f.name == "title" then title_field = f; break end
        end
        if title_field == nil then return "NO_TITLE" end
        if title_field.type ~= "text" then return "TITLE_TYPE:" .. title_field.type end
        if not title_field.required then return "TITLE_NOT_REQUIRED" end
        if not title_field.unique then return "TITLE_NOT_UNIQUE" end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn schema_get_collection_nonexistent() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local schema = crap.schema.get_collection("nonexistent")
        if schema ~= nil then return "NOT_NIL" end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn schema_get_global() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local schema = crap.schema.get_global("settings")
        if schema == nil then return "NIL" end
        if schema.slug ~= "settings" then return "SLUG:" .. tostring(schema.slug) end
        if #schema.fields ~= 2 then return "FIELDS:" .. tostring(#schema.fields) end
        if schema.fields[1].name ~= "site_name" then return "F1:" .. schema.fields[1].name end
        if schema.fields[2].name ~= "maintenance_mode" then return "F2:" .. schema.fields[2].name end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn schema_get_global_nonexistent() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local schema = crap.schema.get_global("nonexistent")
        if schema ~= nil then return "NOT_NIL" end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn schema_list_collections() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local list = crap.schema.list_collections()
        if #list < 1 then return "EMPTY" end
        -- Should contain articles
        local found = false
        for _, item in ipairs(list) do
            if item.slug == "articles" then
                found = true
                if item.labels.singular ~= "Article" then
                    return "LABEL:" .. tostring(item.labels.singular)
                end
            end
        end
        if not found then return "NO_ARTICLES" end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn schema_list_globals() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local list = crap.schema.list_globals()
        if #list < 1 then return "EMPTY" end
        -- Should contain settings
        local found = false
        for _, item in ipairs(list) do
            if item.slug == "settings" then
                found = true
            end
        end
        if not found then return "NO_SETTINGS" end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn schema_collection_metadata() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local schema = crap.schema.get_collection("articles")
        -- articles fixture doesn't have auth/upload/versions
        if schema.has_auth then return "HAS_AUTH" end
        if schema.has_upload then return "HAS_UPLOAD" end
        if schema.has_versions then return "HAS_VERSIONS" end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn schema_field_with_options() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local schema = crap.schema.get_collection("articles")
        -- Find the status field which has select options
        local status_field = nil
        for _, f in ipairs(schema.fields) do
            if f.name == "status" then status_field = f; break end
        end
        if status_field == nil then return "NO_STATUS" end
        if status_field.type ~= "select" then return "TYPE:" .. status_field.type end
        if #status_field.options ~= 9 then return "OPTS:" .. #status_field.options end
        if status_field.options[1].value ~= "draft" then return "OPT1:" .. status_field.options[1].value end
        if status_field.options[2].value ~= "published" then return "OPT2:" .. status_field.options[2].value end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

// ══════════════════════════════════════════════════════════════════════════════
// crap.crypto.* tests (additional)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn lua_crypto_sha256() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local hash = crap.crypto.sha256("hello")
        -- Known SHA256 of "hello"
        if hash == "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824" then
            return "ok"
        end
        return "WRONG:" .. hash
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn lua_crypto_hmac_sha256() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local sig = crap.crypto.hmac_sha256("message", "secret_key")
        -- HMAC should be a 64-char hex string
        if #sig ~= 64 then return "BAD_LEN:" .. tostring(#sig) end
        -- Verify it's hex only
        if sig:match("^[0-9a-f]+$") == nil then return "NOT_HEX" end
        -- Same inputs should always produce the same output
        local sig2 = crap.crypto.hmac_sha256("message", "secret_key")
        if sig ~= sig2 then return "NOT_DETERMINISTIC" end
        -- Different key should produce different output
        local sig3 = crap.crypto.hmac_sha256("message", "other_key")
        if sig == sig3 then return "SAME_WITH_DIFFERENT_KEY" end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn lua_crypto_base64_encode_decode() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local encoded = crap.crypto.base64_encode("Hello, World!")
        if encoded ~= "SGVsbG8sIFdvcmxkIQ==" then
            return "ENCODE:" .. encoded
        end
        local decoded = crap.crypto.base64_decode(encoded)
        if decoded ~= "Hello, World!" then
            return "DECODE:" .. decoded
        end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn lua_crypto_base64_decode_invalid() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local ok, err = pcall(function()
            crap.crypto.base64_decode("!!!invalid!!!")
        end)
        if ok then return "SHOULD_FAIL" end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn lua_crypto_encrypt_decrypt_roundtrip() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local plaintext = "secret message 123"
        local encrypted = crap.crypto.encrypt(plaintext)
        -- Encrypted text should be a base64 string, different from plaintext
        if encrypted == plaintext then return "NOT_ENCRYPTED" end
        if #encrypted == 0 then return "EMPTY_ENCRYPTED" end
        -- Decrypt should produce the original
        local decrypted = crap.crypto.decrypt(encrypted)
        if decrypted ~= plaintext then
            return "MISMATCH:" .. decrypted
        end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn lua_crypto_decrypt_invalid() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local ok, err = pcall(function()
            -- Too-short ciphertext (less than 12 bytes for nonce)
            crap.crypto.decrypt("AQID")
        end)
        if ok then return "SHOULD_FAIL" end
        local err_str = tostring(err)
        if err_str:find("too short") or err_str:find("decrypt") then
            return "ok"
        end
        return "UNEXPECTED:" .. err_str
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn lua_crypto_random_bytes() {
    let runner = setup_lua();
    let result = eval_lua(&runner, r#"
        local bytes16 = crap.crypto.random_bytes(16)
        -- Should produce 32-char hex string (16 bytes * 2 chars per byte)
        if #bytes16 ~= 32 then return "BAD_LEN:" .. tostring(#bytes16) end
        -- Should be hex
        if bytes16:match("^[0-9a-f]+$") == nil then return "NOT_HEX" end
        -- Different calls should produce different results
        local bytes16_2 = crap.crypto.random_bytes(16)
        if bytes16 == bytes16_2 then return "NOT_RANDOM" end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

// ══════════════════════════════════════════════════════════════════════════════
// crap.schema.* tests (covers hooks/api/schema.rs) - with setup_with_db
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn lua_schema_get_collection() {
    let (_tmp, pool, _reg, runner) = setup_with_db();
    let result = eval_lua_db(&runner, &pool, r#"
        local schema = crap.schema.get_collection("articles")
        if schema == nil then return "NIL" end
        if schema.slug ~= "articles" then return "WRONG_SLUG:" .. tostring(schema.slug) end
        if schema.timestamps ~= true then return "NO_TIMESTAMPS" end
        -- Should have fields
        if schema.fields == nil then return "NO_FIELDS" end
        if #schema.fields == 0 then return "EMPTY_FIELDS" end
        -- Check first field
        local f = schema.fields[1]
        if f.name == nil then return "NO_FIELD_NAME" end
        if f.type == nil then return "NO_FIELD_TYPE" end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn lua_schema_get_collection_nonexistent() {
    let (_tmp, pool, _reg, runner) = setup_with_db();
    let result = eval_lua_db(&runner, &pool, r#"
        local schema = crap.schema.get_collection("nonexistent")
        if schema == nil then return "ok" end
        return "NOT_NIL"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn lua_schema_get_global() {
    let (_tmp, pool, _reg, runner) = setup_with_db();
    let result = eval_lua_db(&runner, &pool, r#"
        local schema = crap.schema.get_global("settings")
        if schema == nil then return "NIL" end
        if schema.slug ~= "settings" then return "WRONG_SLUG:" .. tostring(schema.slug) end
        if schema.fields == nil then return "NO_FIELDS" end
        if #schema.fields == 0 then return "EMPTY_FIELDS" end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn lua_schema_get_global_nonexistent() {
    let (_tmp, pool, _reg, runner) = setup_with_db();
    let result = eval_lua_db(&runner, &pool, r#"
        local schema = crap.schema.get_global("nonexistent")
        if schema == nil then return "ok" end
        return "NOT_NIL"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn lua_schema_list_collections() {
    let (_tmp, pool, _reg, runner) = setup_with_db();
    let result = eval_lua_db(&runner, &pool, r#"
        local list = crap.schema.list_collections()
        if list == nil then return "NIL" end
        if #list == 0 then return "EMPTY" end
        -- Each entry should have slug and labels
        local found_articles = false
        for _, item in ipairs(list) do
            if item.slug == "articles" then
                found_articles = true
            end
        end
        if not found_articles then return "NO_ARTICLES" end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn lua_schema_list_globals() {
    let (_tmp, pool, _reg, runner) = setup_with_db();
    let result = eval_lua_db(&runner, &pool, r#"
        local list = crap.schema.list_globals()
        if list == nil then return "NIL" end
        if #list == 0 then return "EMPTY" end
        local found_settings = false
        for _, item in ipairs(list) do
            if item.slug == "settings" then
                found_settings = true
            end
        end
        if not found_settings then return "NO_SETTINGS" end
        return "ok"
    "#);
    assert_eq!(result, "ok");
}

#[test]
fn lua_schema_field_options() {
    // Test that schema introspection returns select field options
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();
    std::fs::write(
        collections_dir.join("items.lua"),
        r#"
crap.collections.define("items", {
    fields = {
        { name = "status", type = "select", options = {
            { label = "Active", value = "active" },
            { label = "Inactive", value = "inactive" },
        }},
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = hooks::init_lua(tmp.path(), &config).expect("init_lua");
    let runner = HookRunner::new(tmp.path(), registry, &config).expect("HookRunner");

    let mut db_config = CrapConfig::default();
    db_config.database.path = "test.db".to_string();
    let pool = crap_cms::db::pool::create_pool(tmp.path(), &db_config).expect("pool");
    let conn = pool.get().expect("conn");
    let result = runner.eval_lua_with_conn(r#"
        local schema = crap.schema.get_collection("items")
        if schema == nil then return "NIL" end
        local status_field = schema.fields[1]
        if status_field.name ~= "status" then return "WRONG_FIELD:" .. tostring(status_field.name) end
        if status_field.options == nil then return "NO_OPTIONS" end
        if #status_field.options ~= 2 then return "WRONG_COUNT:" .. tostring(#status_field.options) end
        if status_field.options[1].value ~= "active" then
            return "WRONG_VALUE:" .. tostring(status_field.options[1].value)
        end
        return "ok"
    "#, &conn, None).expect("eval");
    assert_eq!(result, "ok");
}

#[test]
fn lua_schema_field_relationship() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();
    std::fs::write(
        collections_dir.join("posts.lua"),
        r#"
crap.collections.define("posts", {
    fields = {
        { name = "author", type = "relationship", relationship = {
            collection = "users",
            has_many = false,
        }},
        { name = "tags", type = "relationship", relationship = {
            collection = "tags",
            has_many = true,
            max_depth = 2,
        }},
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = hooks::init_lua(tmp.path(), &config).expect("init_lua");
    let runner = HookRunner::new(tmp.path(), registry, &config).expect("HookRunner");

    let mut db_config = CrapConfig::default();
    db_config.database.path = "test.db".to_string();
    let pool = crap_cms::db::pool::create_pool(tmp.path(), &db_config).expect("pool");
    let conn = pool.get().expect("conn");
    let result = runner.eval_lua_with_conn(r#"
        local schema = crap.schema.get_collection("posts")
        if schema == nil then return "NIL" end
        -- Check author field
        local author = schema.fields[1]
        if author.relationship == nil then return "NO_REL" end
        if author.relationship.collection ~= "users" then
            return "WRONG_COL:" .. tostring(author.relationship.collection)
        end
        if author.relationship.has_many ~= false then return "SHOULD_NOT_HAVE_MANY" end
        -- Check tags field
        local tags = schema.fields[2]
        if tags.relationship == nil then return "NO_TAGS_REL" end
        if tags.relationship.has_many ~= true then return "TAGS_SHOULD_HAVE_MANY" end
        if tags.relationship.max_depth ~= 2 then
            return "WRONG_MAX_DEPTH:" .. tostring(tags.relationship.max_depth)
        end
        return "ok"
    "#, &conn, None).expect("eval");
    assert_eq!(result, "ok");
}

#[test]
fn lua_schema_blocks_and_subfields() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();
    std::fs::write(
        collections_dir.join("pages.lua"),
        r#"
crap.collections.define("pages", {
    fields = {
        { name = "layout", type = "blocks", blocks = {
            { type = "hero", label = "Hero Section", fields = {
                { name = "heading", type = "text" },
                { name = "image", type = "text" },
            }},
        }},
        { name = "meta", type = "group", fields = {
            { name = "title", type = "text" },
            { name = "desc", type = "textarea" },
        }},
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = hooks::init_lua(tmp.path(), &config).expect("init_lua");
    let runner = HookRunner::new(tmp.path(), registry, &config).expect("HookRunner");

    let mut db_config = CrapConfig::default();
    db_config.database.path = "test.db".to_string();
    let pool = crap_cms::db::pool::create_pool(tmp.path(), &db_config).expect("pool");
    let conn = pool.get().expect("conn");
    let result = runner.eval_lua_with_conn(r#"
        local schema = crap.schema.get_collection("pages")
        if schema == nil then return "NIL" end
        -- Check blocks field
        local layout = schema.fields[1]
        if layout.type ~= "blocks" then return "NOT_BLOCKS:" .. tostring(layout.type) end
        if layout.blocks == nil then return "NO_BLOCKS" end
        if #layout.blocks ~= 1 then return "WRONG_BLOCK_COUNT:" .. tostring(#layout.blocks) end
        local hero = layout.blocks[1]
        if hero.type ~= "hero" then return "WRONG_BLOCK_TYPE:" .. tostring(hero.type) end
        if hero.label ~= "Hero Section" then return "WRONG_LABEL:" .. tostring(hero.label) end
        if #hero.fields ~= 2 then return "WRONG_FIELD_COUNT:" .. tostring(#hero.fields) end
        -- Check group field sub-fields
        local meta = schema.fields[2]
        if meta.type ~= "group" then return "NOT_GROUP:" .. tostring(meta.type) end
        if meta.fields == nil then return "NO_GROUP_FIELDS" end
        if #meta.fields ~= 2 then return "WRONG_GROUP_FIELDS:" .. tostring(#meta.fields) end
        return "ok"
    "#, &conn, None).expect("eval");
    assert_eq!(result, "ok");
}

// ── 5A. crap.schema with blocks ───────────────────────────────────────────────

#[test]
fn schema_get_collection_with_blocks() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("pages.lua"),
        r#"
crap.collections.define("pages", {
    fields = {
        { name = "title", type = "text", required = true },
        { name = "content", type = "blocks", blocks = {
            { type = "text", label = "Text Block", fields = {
                { name = "body", type = "richtext" },
            }},
            { type = "image", label = "Image Block", fields = {
                { name = "src", type = "text" },
                { name = "alt", type = "text" },
            }},
        }},
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = crap_cms::hooks::init_lua(tmp.path(), &config).expect("init_lua");
    let runner = crap_cms::hooks::lifecycle::HookRunner::new(
        tmp.path(), registry, &config,
    ).expect("HookRunner::new");

    let tmp2 = tempfile::tempdir().expect("tempdir2");
    let mut db_config = CrapConfig::default();
    db_config.database.path = "test.db".to_string();
    let pool = crap_cms::db::pool::create_pool(tmp2.path(), &db_config).expect("pool");
    let conn = pool.get().expect("conn");

    let result = runner.eval_lua_with_conn(r#"
        local def = crap.schema.get_collection("pages")
        if def == nil then return "NIL" end
        -- Find the blocks field
        local blocks_field = nil
        for _, f in ipairs(def.fields) do
            if f.name == "content" then blocks_field = f end
        end
        if blocks_field == nil then return "NO_CONTENT_FIELD" end
        if blocks_field.type ~= "blocks" then return "WRONG_TYPE:" .. blocks_field.type end
        if blocks_field.blocks == nil then return "NO_BLOCKS" end
        if #blocks_field.blocks ~= 2 then return "WRONG_BLOCK_COUNT:" .. tostring(#blocks_field.blocks) end
        if blocks_field.blocks[1].type ~= "text" then return "WRONG_BLOCK_1:" .. blocks_field.blocks[1].type end
        if blocks_field.blocks[1].label ~= "Text Block" then return "WRONG_LABEL_1" end
        if #blocks_field.blocks[1].fields ~= 1 then return "WRONG_FIELDS_1" end
        if blocks_field.blocks[2].type ~= "image" then return "WRONG_BLOCK_2" end
        if #blocks_field.blocks[2].fields ~= 2 then return "WRONG_FIELDS_2" end
        return "ok"
    "#, &conn, None).expect("eval");
    assert_eq!(result, "ok");
}

// ── 5F. crap.schema with sub-fields (array/group) ────────────────────────────

#[test]
fn schema_get_collection_with_array_subfields() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("posts.lua"),
        r#"
crap.collections.define("posts", {
    fields = {
        { name = "title", type = "text" },
        { name = "tags", type = "array", fields = {
            { name = "label", type = "text", required = true },
            { name = "value", type = "text" },
        }},
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = crap_cms::hooks::init_lua(tmp.path(), &config).expect("init_lua");
    let runner = crap_cms::hooks::lifecycle::HookRunner::new(
        tmp.path(), registry, &config,
    ).expect("HookRunner::new");

    let tmp2 = tempfile::tempdir().expect("tempdir2");
    let mut db_config = CrapConfig::default();
    db_config.database.path = "test.db".to_string();
    let pool = crap_cms::db::pool::create_pool(tmp2.path(), &db_config).expect("pool");
    let conn = pool.get().expect("conn");

    let result = runner.eval_lua_with_conn(r#"
        local def = crap.schema.get_collection("posts")
        if def == nil then return "NIL" end
        local tags_field = nil
        for _, f in ipairs(def.fields) do
            if f.name == "tags" then tags_field = f end
        end
        if tags_field == nil then return "NO_TAGS_FIELD" end
        if tags_field.fields == nil then return "NO_SUB_FIELDS" end
        if #tags_field.fields ~= 2 then return "WRONG_SUB_COUNT:" .. tostring(#tags_field.fields) end
        if tags_field.fields[1].name ~= "label" then return "WRONG_NAME_1" end
        if tags_field.fields[1].required ~= true then return "NOT_REQUIRED" end
        return "ok"
    "#, &conn, None).expect("eval");
    assert_eq!(result, "ok");
}

// ── 5G. crap.schema with relationship fields ─────────────────────────────────

#[test]
fn schema_get_collection_with_relationship() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("posts.lua"),
        r#"
crap.collections.define("posts", {
    fields = {
        { name = "title", type = "text" },
        { name = "author", type = "relationship", relationship = {
            collection = "users",
            has_many = true,
            max_depth = 2,
        }},
    },
})
        "#,
    ).unwrap();
    std::fs::write(
        collections_dir.join("users.lua"),
        r#"
crap.collections.define("users", {
    fields = {
        { name = "name", type = "text" },
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = crap_cms::hooks::init_lua(tmp.path(), &config).expect("init_lua");
    let runner = crap_cms::hooks::lifecycle::HookRunner::new(
        tmp.path(), registry, &config,
    ).expect("HookRunner::new");

    let tmp2 = tempfile::tempdir().expect("tempdir2");
    let mut db_config = CrapConfig::default();
    db_config.database.path = "test.db".to_string();
    let pool = crap_cms::db::pool::create_pool(tmp2.path(), &db_config).expect("pool");
    let conn = pool.get().expect("conn");

    let result = runner.eval_lua_with_conn(r#"
        local def = crap.schema.get_collection("posts")
        if def == nil then return "NIL" end
        local author_field = nil
        for _, f in ipairs(def.fields) do
            if f.name == "author" then author_field = f end
        end
        if author_field == nil then return "NO_AUTHOR" end
        if author_field.relationship == nil then return "NO_REL" end
        if author_field.relationship.collection ~= "users" then return "WRONG_COLLECTION" end
        if author_field.relationship.has_many ~= true then return "NOT_HAS_MANY" end
        if author_field.relationship.max_depth ~= 2 then return "WRONG_MAX_DEPTH" end
        return "ok"
    "#, &conn, None).expect("eval");
    assert_eq!(result, "ok");
}

// ── 5H. crap.schema with select options ──────────────────────────────────────

#[test]
fn schema_get_collection_with_options() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("posts.lua"),
        r#"
crap.collections.define("posts", {
    fields = {
        { name = "status", type = "select", options = {
            { label = "Draft", value = "draft" },
            { label = "Published", value = "published" },
        }},
    },
})
        "#,
    ).unwrap();
    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = crap_cms::hooks::init_lua(tmp.path(), &config).expect("init_lua");
    let runner = crap_cms::hooks::lifecycle::HookRunner::new(
        tmp.path(), registry, &config,
    ).expect("HookRunner::new");

    let tmp2 = tempfile::tempdir().expect("tempdir2");
    let mut db_config = CrapConfig::default();
    db_config.database.path = "test.db".to_string();
    let pool = crap_cms::db::pool::create_pool(tmp2.path(), &db_config).expect("pool");
    let conn = pool.get().expect("conn");

    let result = runner.eval_lua_with_conn(r#"
        local def = crap.schema.get_collection("posts")
        local status_field = nil
        for _, f in ipairs(def.fields) do
            if f.name == "status" then status_field = f end
        end
        if status_field == nil then return "NO_STATUS" end
        if status_field.options == nil then return "NO_OPTIONS" end
        if #status_field.options ~= 2 then return "WRONG_OPTION_COUNT" end
        if status_field.options[1].value ~= "draft" then return "WRONG_VALUE_1" end
        if status_field.options[1].label ~= "Draft" then return "WRONG_LABEL_1" end
        return "ok"
    "#, &conn, None).expect("eval");
    assert_eq!(result, "ok");
}

#[test]
fn parse_richtext_format_json() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("pages.lua"),
        r#"
crap.collections.define("pages", {
    fields = {
        { name = "content", type = "richtext", admin = { format = "json" } },
    },
})
        "#,
    ).unwrap();

    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = hooks::init_lua(tmp.path(), &config).expect("init_lua failed");

    let reg = registry.read().unwrap();
    let def = reg.get_collection("pages").expect("pages should be registered");
    let field = def.fields.iter().find(|f| f.name == "content").unwrap();
    assert_eq!(field.admin.richtext_format.as_deref(), Some("json"));
}

#[test]
fn parse_richtext_format_absent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let collections_dir = tmp.path().join("collections");
    std::fs::create_dir_all(&collections_dir).unwrap();

    std::fs::write(
        collections_dir.join("pages.lua"),
        r#"
crap.collections.define("pages", {
    fields = {
        { name = "content", type = "richtext" },
    },
})
        "#,
    ).unwrap();

    std::fs::write(tmp.path().join("init.lua"), "").unwrap();

    let config = CrapConfig::default();
    let registry = hooks::init_lua(tmp.path(), &config).expect("init_lua failed");

    let reg = registry.read().unwrap();
    let def = reg.get_collection("pages").expect("pages should be registered");
    let field = def.fields.iter().find(|f| f.name == "content").unwrap();
    assert!(field.admin.richtext_format.is_none());
}
