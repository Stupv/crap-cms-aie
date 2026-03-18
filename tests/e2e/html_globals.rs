use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use crap_cms::core::field::*;

use crate::helpers::*;
use crate::html;

fn make_global_with_fields() -> crap_cms::core::collection::GlobalDefinition {
    use crap_cms::core::collection::*;

    let mut def = GlobalDefinition::new("settings");
    def.labels = Labels {
        singular: Some(LocalizedString::Plain("Settings".to_string())),
        plural: None,
    };
    def.fields = vec![
        FieldDefinition::builder("site_name", FieldType::Text)
            .required(true)
            .build(),
        FieldDefinition::builder("tagline", FieldType::Text).build(),
    ];
    def
}

// ── 23. global_edit_form_renders_fields ───────────────────────────────────

#[tokio::test]
async fn global_edit_form_renders_fields() {
    let app = setup_app(vec![make_users_def()], vec![make_global_with_fields()]);
    let user_id = create_test_user(&app, "global@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "global@test.com");

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/globals/settings")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    let doc = html::parse(&body);

    html::assert_field_exists(&doc, "site_name");
    html::assert_field_exists(&doc, "tagline");
    html::assert_exists(&doc, "input[name=\"site_name\"]", "site_name input");
    html::assert_exists(&doc, "input[name=\"tagline\"]", "tagline input");
}

// ── 24. global_form_has_validate_wrapper ──────────────────────────────────

#[tokio::test]
async fn global_form_has_validate_wrapper() {
    let app = setup_app(vec![make_users_def()], vec![make_global_with_fields()]);
    let user_id = create_test_user(&app, "gval@test.com", "pass123");
    let cookie = make_auth_cookie(&app, &user_id, "gval@test.com");

    let resp = app
        .router
        .oneshot(
            Request::get("/admin/globals/settings")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp.into_body()).await;
    let doc = html::parse(&body);

    html::assert_exists(
        &doc,
        "crap-validate-form",
        "global form should be wrapped in <crap-validate-form>",
    );
}
