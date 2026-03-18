use std::net::SocketAddr;

use chromiumoxide::Browser;
use chromiumoxide::BrowserConfig;
use chromiumoxide::Page;
use tokio::task::JoinHandle;

use crap_cms::core::collection::{CollectionDefinition, GlobalDefinition};

use crate::helpers::{self, TestApp};

/// Spawn a real HTTP server bound to 127.0.0.1:0 and return the base URL,
/// a join handle for the server task, and the TestApp.
pub async fn spawn_server(
    collections: Vec<CollectionDefinition>,
    globals: Vec<GlobalDefinition>,
) -> (String, JoinHandle<()>, TestApp) {
    let app = helpers::setup_app(collections, globals);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");

    let router = app.router.clone();
    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    (base_url, handle, app)
}

/// Launch a headless Chrome browser. Returns the browser and a join handle
/// for the websocket event loop.
pub async fn launch_browser() -> (Browser, JoinHandle<()>) {
    let (browser, mut handler) = Browser::launch(
        BrowserConfig::builder()
            .no_sandbox()
            .arg("--headless=new")
            .build()
            .unwrap(),
    )
    .await
    .unwrap();

    let handle = tokio::spawn(async move {
        while let Some(h) = handler.next().await {
            if h.is_err() {
                break;
            }
        }
    });

    (browser, handle)
}

/// Log in via the browser by navigating to the login page, filling
/// email/password, and submitting.
pub async fn browser_login(page: &Page, base_url: &str, email: &str, password: &str) {
    page.goto(format!("{base_url}/admin/login"))
        .await
        .unwrap()
        .wait_for_navigation()
        .await
        .unwrap();

    page.find_element("input[name=\"email\"]")
        .await
        .unwrap()
        .click()
        .await
        .unwrap()
        .type_str(email)
        .await
        .unwrap();

    page.find_element("input[name=\"password\"]")
        .await
        .unwrap()
        .click()
        .await
        .unwrap()
        .type_str(password)
        .await
        .unwrap();

    page.find_element("button[type=\"submit\"]")
        .await
        .unwrap()
        .click()
        .await
        .unwrap();

    page.wait_for_navigation().await.unwrap();
}
