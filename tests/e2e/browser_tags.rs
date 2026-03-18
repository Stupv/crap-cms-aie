use std::time::Duration;

use crate::browser;
use crate::helpers::*;

use crap_cms::core::collection::*;
use crap_cms::core::field::*;

fn make_tags_def() -> CollectionDefinition {
    let mut def = CollectionDefinition::new("articles");
    def.labels = Labels {
        singular: Some(LocalizedString::Plain("Article".to_string())),
        plural: Some(LocalizedString::Plain("Articles".to_string())),
    };
    def.timestamps = true;
    def.fields = vec![
        FieldDefinition::builder("title", FieldType::Text)
            .required(true)
            .build(),
        FieldDefinition::builder("keywords", FieldType::Text)
            .has_many(true)
            .build(),
    ];
    def
}

// ── tags_add_via_enter ───────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn tags_add_via_enter() {
    let (base_url, server_handle, app) =
        browser::spawn_server(vec![make_tags_def(), make_users_def()], vec![]).await;
    let user_id = create_test_user(&app, "btag1@test.com", "pass123");
    let _ = make_auth_cookie(&app, &user_id, "btag1@test.com");

    let (browser, _browser_handle) = browser::launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    browser::browser_login(&page, &base_url, "btag1@test.com", "pass123").await;

    page.goto(format!("{base_url}/admin/collections/articles/create"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();

    // Find the tags text input and type a tag
    page.find_element(".form__tags-input")
        .await
        .unwrap()
        .click()
        .await
        .unwrap()
        .type_str("rust")
        .await
        .unwrap();

    // Press Enter to add the tag
    page.find_element(".form__tags-input")
        .await
        .unwrap()
        .press_key("Enter")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // A chip should appear
    let chips = page.find_elements(".form__tags-chip").await.unwrap();
    assert_eq!(
        chips.len(),
        1,
        "should have 1 tag chip after pressing Enter"
    );

    server_handle.abort();
}

// ── tags_remove_via_click ────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn tags_remove_via_click() {
    let (base_url, server_handle, app) =
        browser::spawn_server(vec![make_tags_def(), make_users_def()], vec![]).await;
    let user_id = create_test_user(&app, "btag2@test.com", "pass123");
    let _ = make_auth_cookie(&app, &user_id, "btag2@test.com");

    let (browser, _browser_handle) = browser::launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    browser::browser_login(&page, &base_url, "btag2@test.com", "pass123").await;

    page.goto(format!("{base_url}/admin/collections/articles/create"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();

    // Add a tag
    page.find_element(".form__tags-input")
        .await
        .unwrap()
        .click()
        .await
        .unwrap()
        .type_str("removeme")
        .await
        .unwrap();
    page.find_element(".form__tags-input")
        .await
        .unwrap()
        .press_key("Enter")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Click the remove button on the chip
    page.find_element(".form__tags-chip-remove")
        .await
        .unwrap()
        .click()
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    let chips = page.find_elements(".form__tags-chip").await.unwrap();
    assert_eq!(chips.len(), 0, "chip should be removed after clicking X");

    server_handle.abort();
}

// ── tags_prevent_duplicates ──────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn tags_prevent_duplicates() {
    let (base_url, server_handle, app) =
        browser::spawn_server(vec![make_tags_def(), make_users_def()], vec![]).await;
    let user_id = create_test_user(&app, "btag3@test.com", "pass123");
    let _ = make_auth_cookie(&app, &user_id, "btag3@test.com");

    let (browser, _browser_handle) = browser::launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    browser::browser_login(&page, &base_url, "btag3@test.com", "pass123").await;

    page.goto(format!("{base_url}/admin/collections/articles/create"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();

    // Add "duplicate" twice
    for _ in 0..2 {
        page.find_element(".form__tags-input")
            .await
            .unwrap()
            .click()
            .await
            .unwrap()
            .type_str("duplicate")
            .await
            .unwrap();
        page.find_element(".form__tags-input")
            .await
            .unwrap()
            .press_key("Enter")
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    let chips = page.find_elements(".form__tags-chip").await.unwrap();
    assert_eq!(chips.len(), 1, "duplicate tags should be prevented");

    server_handle.abort();
}

// ── tags_submit_persists ─────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn tags_submit_persists() {
    let (base_url, server_handle, app) =
        browser::spawn_server(vec![make_tags_def(), make_users_def()], vec![]).await;
    let user_id = create_test_user(&app, "btag4@test.com", "pass123");
    let _ = make_auth_cookie(&app, &user_id, "btag4@test.com");

    let (browser, _browser_handle) = browser::launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    browser::browser_login(&page, &base_url, "btag4@test.com", "pass123").await;

    page.goto(format!("{base_url}/admin/collections/articles/create"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();

    // Fill title
    page.find_element("input[name=\"title\"]")
        .await
        .unwrap()
        .click()
        .await
        .unwrap()
        .type_str("Tag Article")
        .await
        .unwrap();

    // Add tags
    for tag in &["alpha", "beta"] {
        page.find_element(".form__tags-input")
            .await
            .unwrap()
            .click()
            .await
            .unwrap()
            .type_str(tag)
            .await
            .unwrap();
        page.find_element(".form__tags-input")
            .await
            .unwrap()
            .press_key("Enter")
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // Verify the tags are serialized in the hidden input (comma-separated)
    let result = page
        .evaluate("() => document.querySelector('crap-tags input[type=\"hidden\"]')?.value ?? ''")
        .await
        .unwrap();
    let hidden_val: String = result.into_value().unwrap();
    assert!(
        hidden_val.contains("alpha"),
        "hidden input should contain 'alpha', got: {hidden_val}"
    );
    assert!(
        hidden_val.contains("beta"),
        "hidden input should contain 'beta', got: {hidden_val}"
    );

    server_handle.abort();
}
