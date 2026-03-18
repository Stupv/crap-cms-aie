use std::time::Duration;

use crate::browser;
use crate::helpers::*;

use crap_cms::core::collection::*;
use crap_cms::core::field::*;

fn make_validated_def() -> CollectionDefinition {
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
        FieldDefinition::builder("body", FieldType::Textarea).build(),
    ];
    def
}

// ── 25. client_side_validation_shows_errors ───────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn client_side_validation_shows_errors() {
    let (base_url, server_handle, app) =
        browser::spawn_server(vec![make_validated_def(), make_users_def()], vec![]).await;
    let user_id = create_test_user(&app, "bval@test.com", "pass123");
    let _ = make_auth_cookie(&app, &user_id, "bval@test.com");

    let (browser, _browser_handle) = browser::launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    browser::browser_login(&page, &base_url, "bval@test.com", "pass123").await;

    page.goto(format!("{base_url}/admin/collections/posts/create"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();
    // Wait for JS/HTMX to initialize
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Submit with empty required field using requestSubmit
    page.evaluate("() => document.querySelector('#edit-form')?.requestSubmit()")
        .await
        .unwrap();
    // Wait for validation fetch + error rendering
    tokio::time::sleep(Duration::from_millis(2000)).await;

    let result = page
        .evaluate("() => document.querySelectorAll('.form__error').length")
        .await
        .unwrap();
    let error_count: i64 = result.into_value().unwrap_or(0);
    assert!(
        error_count > 0,
        "should show .form__error after submitting empty required field"
    );

    server_handle.abort();
}

// ── 26. validation_clears_on_valid_resubmit ───────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn validation_clears_on_valid_resubmit() {
    let (base_url, server_handle, app) =
        browser::spawn_server(vec![make_validated_def(), make_users_def()], vec![]).await;
    let user_id = create_test_user(&app, "bclear@test.com", "pass123");
    let _ = make_auth_cookie(&app, &user_id, "bclear@test.com");

    let (browser, _browser_handle) = browser::launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    browser::browser_login(&page, &base_url, "bclear@test.com", "pass123").await;

    page.goto(format!("{base_url}/admin/collections/posts/create"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();
    // Wait for JS/HTMX to initialize
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Trigger validation error
    page.evaluate("() => document.querySelector('#edit-form')?.requestSubmit()")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(2000)).await;

    // Fill in the required field
    page.find_element("input[name=\"title\"]")
        .await
        .unwrap()
        .click()
        .await
        .unwrap()
        .type_str("Valid Title")
        .await
        .unwrap();

    // Resubmit via requestSubmit
    page.evaluate("() => document.querySelector('#edit-form')?.requestSubmit()")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(2000)).await;

    let result = page
        .evaluate("() => document.querySelectorAll('.form__error').length")
        .await
        .unwrap();
    let error_count: i64 = result.into_value().unwrap_or(0);
    assert_eq!(
        error_count, 0,
        "errors should be cleared after valid resubmit, got {error_count}"
    );

    server_handle.abort();
}

// ── 27. validation_expands_collapsed_array_row ────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn validation_expands_collapsed_array_row() {
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
                FieldDefinition::builder("member_name", FieldType::Text)
                    .required(true)
                    .build(),
            ])
            .build(),
    ];

    let (base_url, server_handle, app) =
        browser::spawn_server(vec![def, make_users_def()], vec![]).await;
    let user_id = create_test_user(&app, "barray@test.com", "pass123");
    let _ = make_auth_cookie(&app, &user_id, "barray@test.com");

    let (browser, _browser_handle) = browser::launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    browser::browser_login(&page, &base_url, "barray@test.com", "pass123").await;

    page.goto(format!("{base_url}/admin/collections/teams/create"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();
    // Wait for JS to initialize
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Add a row
    page.evaluate("() => document.querySelector('button[data-action=\"add-array-row\"]')?.click()")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Fill name but leave array sub-field empty
    page.find_element("input[name=\"name\"]")
        .await
        .unwrap()
        .click()
        .await
        .unwrap()
        .type_str("Test Team")
        .await
        .unwrap();

    // Submit via requestSubmit to trigger HTMX validation
    page.evaluate("() => document.querySelector('#edit-form')?.requestSubmit()")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(2000)).await;

    // Error badge or expanded state should appear on the array row
    let result = page
        .evaluate(
            "() => document.querySelectorAll('.form__array-row--has-errors, .form__array-row-error-badge, .form__error').length",
        )
        .await
        .unwrap();
    let badge_count: i64 = result.into_value().unwrap_or(0);
    assert!(
        badge_count > 0,
        "array row with validation error should show error indicator"
    );

    server_handle.abort();
}
