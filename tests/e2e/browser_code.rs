use std::time::Duration;

use crate::browser;
use crate::helpers::*;

use crap_cms::core::collection::*;
use crap_cms::core::field::*;

fn make_code_def() -> CollectionDefinition {
    let mut def = CollectionDefinition::new("snippets");
    def.labels = Labels {
        singular: Some(LocalizedString::Plain("Snippet".to_string())),
        plural: Some(LocalizedString::Plain("Snippets".to_string())),
    };
    def.timestamps = true;
    def.fields = vec![
        FieldDefinition::builder("title", FieldType::Text)
            .required(true)
            .build(),
        FieldDefinition::builder("code", FieldType::Code).build(),
    ];
    def
}

// ── code_renders_codemirror ──────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn code_renders_codemirror() {
    let (base_url, server_handle, app) =
        browser::spawn_server(vec![make_code_def(), make_users_def()], vec![]).await;
    let user_id = create_test_user(&app, "bcode1@test.com", "pass123");
    let _ = make_auth_cookie(&app, &user_id, "bcode1@test.com");

    let (browser, _browser_handle) = browser::launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    browser::browser_login(&page, &base_url, "bcode1@test.com", "pass123").await;

    page.goto(format!("{base_url}/admin/collections/snippets/create"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Check for CodeMirror editor inside shadow root
    let has_editor = browser::shadow_eval(
        &page,
        "crap-code",
        "return root.querySelector('.cm-editor') ? 'true' : 'false';",
    )
    .await;
    assert_eq!(
        has_editor, "true",
        "crap-code shadow root should contain .cm-editor"
    );

    server_handle.abort();
}

// ── code_typing_updates_hidden_input ─────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn code_typing_updates_hidden_input() {
    let (base_url, server_handle, app) =
        browser::spawn_server(vec![make_code_def(), make_users_def()], vec![]).await;
    let user_id = create_test_user(&app, "bcode2@test.com", "pass123");
    let _ = make_auth_cookie(&app, &user_id, "bcode2@test.com");

    let (browser, _browser_handle) = browser::launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    browser::browser_login(&page, &base_url, "bcode2@test.com", "pass123").await;

    page.goto(format!("{base_url}/admin/collections/snippets/create"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Type into the CodeMirror editor via JS (direct interaction with shadow DOM)
    page.evaluate(
        "() => { \
            const host = document.querySelector('crap-code'); \
            const view = host._view; \
            if (view) { \
                view.dispatch({ changes: { from: 0, insert: 'hello world' } }); \
            } \
        }",
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Check that the hidden textarea has been updated
    let result = page
        .evaluate("() => document.querySelector('crap-code textarea')?.value ?? ''")
        .await
        .unwrap();
    let textarea_val: String = result.into_value().unwrap();
    assert!(
        textarea_val.contains("hello world"),
        "hidden textarea should be updated with typed content, got: {textarea_val}"
    );

    server_handle.abort();
}
