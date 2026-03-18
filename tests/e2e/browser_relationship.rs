use std::collections::HashMap;
use std::time::Duration;

use crate::browser;
use crate::helpers::*;

use crap_cms::core::collection::*;
use crap_cms::core::field::*;
use crap_cms::db::query;

fn make_categories_def() -> CollectionDefinition {
    let mut def = CollectionDefinition::new("categories");
    def.labels = Labels {
        singular: Some(LocalizedString::Plain("Category".to_string())),
        plural: Some(LocalizedString::Plain("Categories".to_string())),
    };
    def.timestamps = true;
    def.admin = AdminConfig {
        use_as_title: Some("name".to_string()),
        ..Default::default()
    };
    def.fields = vec![
        FieldDefinition::builder("name", FieldType::Text)
            .required(true)
            .build(),
    ];
    def
}

fn make_rel_posts_def() -> CollectionDefinition {
    let mut def = CollectionDefinition::new("posts");
    def.labels = Labels {
        singular: Some(LocalizedString::Plain("Post".to_string())),
        plural: Some(LocalizedString::Plain("Posts".to_string())),
    };
    def.timestamps = true;
    def.fields = vec![
        FieldDefinition::builder("title", FieldType::Text)
            .required(true)
            .build(),
        FieldDefinition::builder("category", FieldType::Relationship)
            .relationship(RelationshipConfig::new("categories", false))
            .build(),
        FieldDefinition::builder("tags", FieldType::Relationship)
            .relationship(RelationshipConfig::new("categories", true))
            .has_many(true)
            .build(),
    ];
    def
}

fn create_category(app: &TestApp, name: &str) -> String {
    let reg = app.registry.read().unwrap();
    let def = reg.get_collection("categories").unwrap().clone();
    drop(reg);

    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = HashMap::from([("name".to_string(), name.to_string())]);
    let doc = query::create(&tx, "categories", &def, &data, None).unwrap();
    tx.commit().unwrap();
    doc.id.to_string()
}

// ── relationship_search_shows_results ────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn relationship_search_shows_results() {
    let (base_url, server_handle, app) = browser::spawn_server(
        vec![
            make_categories_def(),
            make_rel_posts_def(),
            make_users_def(),
        ],
        vec![],
    )
    .await;
    let user_id = create_test_user(&app, "brel1@test.com", "pass123");
    let _ = make_auth_cookie(&app, &user_id, "brel1@test.com");

    create_category(&app, "Technology");
    create_category(&app, "Science");

    let (browser, _browser_handle) = browser::launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    browser::browser_login(&page, &base_url, "brel1@test.com", "pass123").await;

    page.goto(format!("{base_url}/admin/collections/posts/create"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Focus the has-one relationship input to trigger a search (shows all results)
    page.evaluate(
        "() => { \
            const input = document.querySelector('.relationship-search__input'); \
            if (input) input.focus(); \
        }",
    )
    .await
    .unwrap();
    // Wait for debounce (250ms) + fetch
    tokio::time::sleep(Duration::from_millis(800)).await;

    // Dropdown should appear with results
    let result = page
        .evaluate("() => document.querySelectorAll('.relationship-search__option').length")
        .await
        .unwrap();
    let option_count: i64 = result.into_value().unwrap_or(0);
    assert!(
        option_count >= 2,
        "should show search results in dropdown, got {option_count} options"
    );

    server_handle.abort();
}

// ── relationship_select_sets_value ───────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn relationship_select_sets_value() {
    let (base_url, server_handle, app) = browser::spawn_server(
        vec![
            make_categories_def(),
            make_rel_posts_def(),
            make_users_def(),
        ],
        vec![],
    )
    .await;
    let user_id = create_test_user(&app, "brel2@test.com", "pass123");
    let _ = make_auth_cookie(&app, &user_id, "brel2@test.com");

    create_category(&app, "Music");

    let (browser, _browser_handle) = browser::launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    browser::browser_login(&page, &base_url, "brel2@test.com", "pass123").await;

    page.goto(format!("{base_url}/admin/collections/posts/create"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Focus the has-one input to trigger initial search
    page.evaluate("() => document.querySelector('.relationship-search__input')?.focus()")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(800)).await;

    // Click the first option via mousedown (how the component listens)
    page.evaluate(
        "() => { \
            const opt = document.querySelector('.relationship-search__option'); \
            if (opt) opt.dispatchEvent(new MouseEvent('mousedown', {bubbles: true})); \
        }",
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Hidden input should have a value
    let result = page
        .evaluate(
            "() => document.querySelector('.relationship-search__hidden input[type=\"hidden\"]')?.value ?? ''",
        )
        .await
        .unwrap();
    let hidden_val: String = result.into_value().unwrap();
    assert!(
        !hidden_val.is_empty(),
        "hidden input should have a value after selection"
    );

    server_handle.abort();
}

// ── relationship_has_many_multiple_chips ──────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn relationship_has_many_multiple_chips() {
    let (base_url, server_handle, app) = browser::spawn_server(
        vec![
            make_categories_def(),
            make_rel_posts_def(),
            make_users_def(),
        ],
        vec![],
    )
    .await;
    let user_id = create_test_user(&app, "brel3@test.com", "pass123");
    let _ = make_auth_cookie(&app, &user_id, "brel3@test.com");

    create_category(&app, "Alpha");
    create_category(&app, "Beta");

    let (browser, _browser_handle) = browser::launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    browser::browser_login(&page, &base_url, "brel3@test.com", "pass123").await;

    page.goto(format!("{base_url}/admin/collections/posts/create"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Focus the has-many input (tags field) to trigger search
    page.evaluate(
        "() => { \
            const el = document.querySelector('crap-relationship-search[has-many]'); \
            const input = el?.querySelector('.relationship-search__input'); \
            if (input) input.focus(); \
        }",
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(800)).await;

    // Select first option
    page.evaluate(
        "() => { \
            const opt = document.querySelectorAll('crap-relationship-search[has-many] .relationship-search__option')[0]; \
            if (opt) opt.dispatchEvent(new MouseEvent('mousedown', {bubbles: true})); \
        }",
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Wait for dropdown to close after first selection
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Type a space then delete it to trigger input event which triggers search
    page.evaluate(
        "() => { \
            const el = document.querySelector('crap-relationship-search[has-many]'); \
            const input = el?.querySelector('.relationship-search__input'); \
            if (input) { \
                input.focus(); \
                input.value = ''; \
                input.dispatchEvent(new Event('input', {bubbles: true})); \
            } \
        }",
    )
    .await
    .unwrap();
    // Wait for debounce (250ms) + network fetch
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Select the unselected option
    page.evaluate(
        "() => { \
            const el = document.querySelector('crap-relationship-search[has-many]'); \
            const opts = el?.querySelectorAll('.relationship-search__option') || []; \
            for (const opt of opts) { \
                if (!opt.classList.contains('relationship-search__option--selected')) { \
                    opt.dispatchEvent(new MouseEvent('mousedown', {bubbles: true})); \
                    break; \
                } \
            } \
        }",
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Should have 2 chips
    let result = page
        .evaluate("() => document.querySelectorAll('.relationship-search__chip').length")
        .await
        .unwrap();
    let chip_count: i64 = result.into_value().unwrap_or(0);
    assert!(
        chip_count >= 2,
        "should have at least 2 chips for has-many, got {chip_count}"
    );

    server_handle.abort();
}

// ── relationship_remove_chip ─────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn relationship_remove_chip() {
    let (base_url, server_handle, app) = browser::spawn_server(
        vec![
            make_categories_def(),
            make_rel_posts_def(),
            make_users_def(),
        ],
        vec![],
    )
    .await;
    let user_id = create_test_user(&app, "brel4@test.com", "pass123");
    let _ = make_auth_cookie(&app, &user_id, "brel4@test.com");

    create_category(&app, "RemoveMe");

    let (browser, _browser_handle) = browser::launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    browser::browser_login(&page, &base_url, "brel4@test.com", "pass123").await;

    page.goto(format!("{base_url}/admin/collections/posts/create"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Focus the has-many input to trigger search
    page.evaluate(
        "() => { \
            const el = document.querySelector('crap-relationship-search[has-many]'); \
            const input = el?.querySelector('.relationship-search__input'); \
            if (input) input.focus(); \
        }",
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(800)).await;

    // Select first option
    page.evaluate(
        "() => { \
            const opt = document.querySelector('crap-relationship-search[has-many] .relationship-search__option'); \
            if (opt) opt.dispatchEvent(new MouseEvent('mousedown', {bubbles: true})); \
        }",
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Should have a chip
    let result = page
        .evaluate("() => document.querySelectorAll('.relationship-search__chip').length")
        .await
        .unwrap();
    let chips_before: i64 = result.into_value().unwrap_or(0);
    assert!(chips_before > 0, "should have a chip after selecting");

    // Click remove on the chip
    page.evaluate("() => document.querySelector('.relationship-search__chip-remove')?.click()")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    let result = page
        .evaluate("() => document.querySelectorAll('.relationship-search__chip').length")
        .await
        .unwrap();
    let chips_after: i64 = result.into_value().unwrap_or(0);
    assert_eq!(chips_after, 0, "chip should be removed after clicking X");

    server_handle.abort();
}
