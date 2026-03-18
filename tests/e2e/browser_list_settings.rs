use std::collections::HashMap;
use std::time::Duration;

use crate::browser;
use crate::helpers::*;

use crap_cms::core::collection::*;
use crap_cms::core::field::*;
use crap_cms::db::query;

fn make_list_def() -> CollectionDefinition {
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
        FieldDefinition::builder("status", FieldType::Select)
            .options(vec![
                SelectOption::new(LocalizedString::Plain("Draft".to_string()), "draft"),
                SelectOption::new(LocalizedString::Plain("Published".to_string()), "published"),
            ])
            .build(),
        FieldDefinition::builder("views", FieldType::Number).build(),
    ];
    def
}

fn create_list_post(app: &TestApp, title: &str) {
    let reg = app.registry.read().unwrap();
    let def = reg.get_collection("posts").unwrap().clone();
    drop(reg);

    let mut conn = app.pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    let data = HashMap::from([("title".to_string(), title.to_string())]);
    query::create(&tx, "posts", &def, &data, None).unwrap();
    tx.commit().unwrap();
}

// ── column_picker_opens_drawer ───────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn column_picker_opens_drawer() {
    let (base_url, server_handle, app) =
        browser::spawn_server(vec![make_list_def(), make_users_def()], vec![]).await;
    let user_id = create_test_user(&app, "blist1@test.com", "pass123");
    let _ = make_auth_cookie(&app, &user_id, "blist1@test.com");

    create_list_post(&app, "Sample Post");

    let (browser, _browser_handle) = browser::launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    browser::browser_login(&page, &base_url, "blist1@test.com", "pass123").await;

    page.goto(format!("{base_url}/admin/collections/posts"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();
    // Wait for JS components to initialize
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Click the "Columns" button
    page.evaluate("() => document.querySelector('[data-action=\"open-column-picker\"]')?.click()")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // The drawer dialog should be open (shadow DOM)
    let result = browser::shadow_eval(
        &page,
        "crap-drawer",
        "return root.querySelector('dialog')?.hasAttribute('open') ? 'true' : 'false';",
    )
    .await;
    assert_eq!(result, "true", "drawer should be open for column picker");

    // Column picker items with checkboxes should be inside the drawer's shadow DOM body
    let checkbox_count = browser::shadow_eval(
        &page,
        "crap-drawer",
        "return String(root.querySelectorAll('.column-picker__item input[type=\"checkbox\"]').length);",
    )
    .await;
    let count: i64 = checkbox_count.parse().unwrap_or(0);
    assert!(
        count > 0,
        "column picker should contain checkboxes, got {count}"
    );

    server_handle.abort();
}

// ── filter_builder_adds_condition ─────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn filter_builder_adds_condition() {
    let (base_url, server_handle, app) =
        browser::spawn_server(vec![make_list_def(), make_users_def()], vec![]).await;
    let user_id = create_test_user(&app, "blist2@test.com", "pass123");
    let _ = make_auth_cookie(&app, &user_id, "blist2@test.com");

    create_list_post(&app, "Filterable Post");

    let (browser, _browser_handle) = browser::launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    browser::browser_login(&page, &base_url, "blist2@test.com", "pass123").await;

    page.goto(format!("{base_url}/admin/collections/posts"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();
    // Wait for JS components
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Click "Filters" button
    page.evaluate("() => document.querySelector('[data-action=\"open-filter-builder\"]')?.click()")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // The drawer should be open
    let result = browser::shadow_eval(
        &page,
        "crap-drawer",
        "return root.querySelector('dialog')?.hasAttribute('open') ? 'true' : 'false';",
    )
    .await;
    assert_eq!(result, "true", "drawer should be open for filter builder");

    // Filter builder rows and field selects should be inside the drawer's shadow DOM body
    let row_count = browser::shadow_eval(
        &page,
        "crap-drawer",
        "return String(root.querySelectorAll('.filter-builder__row').length);",
    )
    .await;
    let rows: i64 = row_count.parse().unwrap_or(0);
    assert!(
        rows > 0,
        "filter builder should have at least one condition row, got {rows}"
    );

    let field_count = browser::shadow_eval(
        &page,
        "crap-drawer",
        "return String(root.querySelectorAll('.filter-builder__field').length);",
    )
    .await;
    let fields: i64 = field_count.parse().unwrap_or(0);
    assert!(
        fields > 0,
        "filter builder should have a field select, got {fields}"
    );

    server_handle.abort();
}
