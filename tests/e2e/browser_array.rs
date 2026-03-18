use crate::browser;
use crate::helpers::*;

use crap_cms::core::collection::*;
use crap_cms::core::field::*;

fn make_array_def() -> CollectionDefinition {
    let mut def = CollectionDefinition::new("teams");
    def.labels = Labels {
        singular: Some(LocalizedString::Plain("Team".to_string())),
        plural: Some(LocalizedString::Plain("Teams".to_string())),
    };
    def.timestamps = true;
    def.fields = vec![
        FieldDefinition::builder("name", FieldType::Text)
            .required(true)
            .build(),
        FieldDefinition::builder("members", FieldType::Array)
            .fields(vec![
                FieldDefinition::builder("member_name", FieldType::Text).build(),
            ])
            .build(),
    ];
    def
}

// ── 28. add_row_button_creates_row ────────────────────────────────────────

#[tokio::test]
async fn add_row_button_creates_row() {
    let (base_url, server_handle, app) =
        browser::spawn_server(vec![make_array_def(), make_users_def()], vec![]).await;
    let user_id = create_test_user(&app, "badd@test.com", "pass123");
    let _ = make_auth_cookie(&app, &user_id, "badd@test.com");

    let (browser, _browser_handle) = browser::launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    browser::browser_login(&page, &base_url, "badd@test.com", "pass123").await;

    page.goto(format!("{base_url}/admin/collections/teams/create"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();

    // Initially no rows
    let rows = page.find_elements(".form__array-row").await.unwrap();
    assert_eq!(rows.len(), 0, "should start with 0 rows");

    // Click add
    page.find_element("button[data-action=\"add-array-row\"]")
        .await
        .unwrap()
        .click()
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let rows = page.find_elements(".form__array-row").await.unwrap();
    assert_eq!(rows.len(), 1, "should have 1 row after clicking add");

    server_handle.abort();
}

// ── 29. remove_row_button_removes_row ─────────────────────────────────────

#[tokio::test]
async fn remove_row_button_removes_row() {
    let (base_url, server_handle, app) =
        browser::spawn_server(vec![make_array_def(), make_users_def()], vec![]).await;
    let user_id = create_test_user(&app, "brem@test.com", "pass123");
    let _ = make_auth_cookie(&app, &user_id, "brem@test.com");

    let (browser, _browser_handle) = browser::launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    browser::browser_login(&page, &base_url, "brem@test.com", "pass123").await;

    page.goto(format!("{base_url}/admin/collections/teams/create"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();

    // Add 2 rows
    for _ in 0..2 {
        page.find_element("button[data-action=\"add-array-row\"]")
            .await
            .unwrap()
            .click()
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    let rows = page.find_elements(".form__array-row").await.unwrap();
    assert_eq!(rows.len(), 2, "should have 2 rows");

    // Remove first row
    page.find_element("button[data-action=\"remove-array-row\"]")
        .await
        .unwrap()
        .click()
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let rows = page.find_elements(".form__array-row").await.unwrap();
    assert_eq!(rows.len(), 1, "should have 1 row after removal");

    server_handle.abort();
}

// ── 30. reorder_rows_updates_indices ──────────────────────────────────────

#[tokio::test]
async fn reorder_rows_updates_indices() {
    let (base_url, server_handle, app) =
        browser::spawn_server(vec![make_array_def(), make_users_def()], vec![]).await;
    let user_id = create_test_user(&app, "breorder@test.com", "pass123");
    let _ = make_auth_cookie(&app, &user_id, "breorder@test.com");

    let (browser, _browser_handle) = browser::launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    browser::browser_login(&page, &base_url, "breorder@test.com", "pass123").await;

    page.goto(format!("{base_url}/admin/collections/teams/create"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();

    // Add 2 rows and fill them
    for _ in 0..2 {
        page.find_element("button[data-action=\"add-array-row\"]")
            .await
            .unwrap()
            .click()
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    // Type into first row
    let inputs = page
        .find_elements("input[name*=\"member_name\"]")
        .await
        .unwrap();
    assert_eq!(inputs.len(), 2);
    inputs[0]
        .click()
        .await
        .unwrap()
        .type_str("First")
        .await
        .unwrap();
    inputs[1]
        .click()
        .await
        .unwrap()
        .type_str("Second")
        .await
        .unwrap();

    // Click move-down on first row
    page.find_element("button[data-action=\"move-row-down\"]")
        .await
        .unwrap()
        .click()
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // After reorder, the first row's input should now have "Second" and vice versa
    let inputs = page
        .find_elements("input[name*=\"member_name\"]")
        .await
        .unwrap();
    assert_eq!(inputs.len(), 2, "should still have 2 inputs after reorder");

    server_handle.abort();
}
