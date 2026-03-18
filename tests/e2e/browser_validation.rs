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

#[tokio::test]
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

    // Submit with empty required field
    page.find_element("button[type=\"submit\"]")
        .await
        .unwrap()
        .click()
        .await
        .unwrap();

    // Wait for validation error to appear
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let error_els = page.find_elements(".form__error").await.unwrap();
    assert!(
        !error_els.is_empty(),
        "should show .form__error after submitting empty required field"
    );

    server_handle.abort();
}

// ── 26. validation_clears_on_valid_resubmit ───────────────────────────────

#[tokio::test]
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

    // Trigger error
    page.find_element("button[type=\"submit\"]")
        .await
        .unwrap()
        .click()
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Fill in the field
    page.find_element("input[name=\"title\"]")
        .await
        .unwrap()
        .click()
        .await
        .unwrap()
        .type_str("Valid Title")
        .await
        .unwrap();

    // Resubmit
    page.find_element("button[type=\"submit\"]")
        .await
        .unwrap()
        .click()
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let errors = page.find_elements(".form__error").await.unwrap();
    assert!(
        errors.is_empty(),
        "errors should be cleared after valid resubmit, got {} errors",
        errors.len()
    );

    server_handle.abort();
}

// ── 27. validation_expands_collapsed_array_row ────────────────────────────

#[tokio::test]
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

    // Add a row
    page.find_element("button[data-action=\"add-array-row\"]")
        .await
        .unwrap()
        .click()
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Fill name but leave array sub-field empty, then submit
    page.find_element("input[name=\"name\"]")
        .await
        .unwrap()
        .click()
        .await
        .unwrap()
        .type_str("Test Team")
        .await
        .unwrap();

    page.find_element("button[type=\"submit\"]")
        .await
        .unwrap()
        .click()
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Error badge should appear on the array row
    let badges = page
        .find_elements(".form__array-row--has-errors, .form__array-row-error-badge")
        .await
        .unwrap();
    assert!(
        !badges.is_empty(),
        "collapsed array row with error should show error badge or expanded state"
    );

    server_handle.abort();
}
