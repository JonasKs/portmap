use axum::Router;
use axum::body::Body;
use axum::http::Request;
use axum::http::StatusCode;
use tower::ServiceExt;

async fn setup_app() -> Router {
    portmap::create_router_with_test_db().await
}

async fn setup_app_with_state() -> (Router, portmap::AppState) {
    let state = portmap::create_test_state().await;
    let router = portmap::create_router(state.clone());
    (router, state)
}

#[tokio::test]
async fn test_list_apps_empty() {
    let app = setup_app().await;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/apps")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let apps: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(apps.is_empty());
}

#[tokio::test]
async fn test_create_and_get_app() {
    let app = setup_app().await;

    // Create
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/apps")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"name":"test-app","port":3000,"category":"frontend"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(created["name"], "test-app");
    assert_eq!(created["port"], 3000);
    assert_eq!(created["category"], "frontend");

    let id = created["id"].as_i64().unwrap();

    // Get
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/apps/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let fetched: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(fetched["name"], "test-app");
}

#[tokio::test]
async fn test_update_app() {
    let app = setup_app().await;

    // Create
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/apps")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"name":"old-name","port":4000,"category":"backend"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = created["id"].as_i64().unwrap();

    // Update
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/apps/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"new-name"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated["name"], "new-name");
    assert_eq!(updated["category"], "backend");
}

#[tokio::test]
async fn test_delete_app() {
    let app = setup_app().await;

    // Create
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/apps")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"name":"to-delete","port":5000,"category":"mcp"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = created["id"].as_i64().unwrap();

    // Delete
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/apps/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify gone
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/apps/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_bulk_create() {
    let app = setup_app().await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/apps/bulk")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"[
                        {"name":"app-a","port":3001,"category":"frontend"},
                        {"name":"app-b","port":3002,"category":"backend"}
                    ]"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(created.len(), 2);

    // List should show both
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/apps")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let apps: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(apps.len(), 2);
}

#[tokio::test]
async fn test_duplicate_port_conflict() {
    let app = setup_app().await;

    // Create first
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/apps")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"name":"first","port":6000,"category":"frontend"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Duplicate port should conflict
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/apps")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"name":"second","port":6000,"category":"backend"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_markdown_endpoint() {
    let app = setup_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/markdown")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ct.contains("text/markdown"));
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(text.contains("# portmap"));
    assert!(text.contains("## API"));
}

#[tokio::test]
async fn test_content_negotiation_markdown() {
    let app = setup_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("accept", "text/markdown")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ct.contains("text/markdown"));
    let vary = resp.headers().get("vary").unwrap().to_str().unwrap();
    assert!(vary.contains("Accept"));
}

#[tokio::test]
async fn test_create_app_without_name() {
    let app = setup_app().await;

    // Create with only port and category (no name)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/apps")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"port":7777,"category":"backend"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(created["name"], "");
    assert_eq!(created["port"], 7777);
    assert_eq!(created["category"], "backend");
}

#[tokio::test]
async fn test_tag_color_crud() {
    let app = setup_app().await;

    // Set a tag color
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/tag-colors/frontend")
                .header("content-type", "application/json")
                .body(Body::from(r##"{"color":"#ef4444"}"##))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let tc: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(tc["category"], "frontend");
    assert_eq!(tc["color"], "#ef4444");

    // List tag colors
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/tag-colors")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let colors: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(colors.len(), 1);
    assert_eq!(colors[0]["color"], "#ef4444");

    // Update the color
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/tag-colors/frontend")
                .header("content-type", "application/json")
                .body(Body::from(r##"{"color":"#22c55e"}"##))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let tc: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(tc["color"], "#22c55e");

    // Delete the color
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/tag-colors/frontend")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify gone
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/tag-colors")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let colors: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(colors.is_empty());
}

#[tokio::test]
async fn test_content_negotiation_html() {
    let app = setup_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("accept", "text/html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(text.contains("<!DOCTYPE html>"));
}

#[tokio::test]
async fn test_crud_publishes_to_sse_channel() {
    let (app, state) = setup_app_with_state().await;
    let mut rx = state.updates.clone();

    // Mark current value as seen
    rx.borrow_and_update();

    // Create an app — should republish to the watch channel
    let _resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/apps")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"name":"sse-test","port":9999,"category":"test"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // The watch channel should have a new value
    assert!(
        rx.has_changed().unwrap_or(false),
        "CRUD should publish to the SSE watch channel"
    );
    let payload: serde_json::Value = serde_json::from_str(&*rx.borrow_and_update()).unwrap();
    assert!(payload["rows"].is_array());
    assert!(payload["pill"].as_str().unwrap().contains("port"));
}

#[tokio::test]
async fn test_crud_before_first_scan_shows_probed_status() {
    let (app, state) = setup_app_with_state().await;
    let mut rx = state.updates.clone();
    rx.borrow_and_update();

    // No scan has run yet — cached_ports is empty.
    // Create an app on a port that definitely isn't listening.
    let _resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/apps")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"name":"offline-app","port":19,"category":"test"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Republish should have run a quick probe (not use empty cache)
    let payload: serde_json::Value = serde_json::from_str(&*rx.borrow_and_update()).unwrap();
    let rows = payload["rows"].as_array().unwrap();
    // The app should appear in rows (as offline since port 19 isn't listening)
    let row = rows.iter().find(|r| r["port"] == 19);
    assert!(row.is_some(), "registered app should appear in rows");
    assert!(!row.unwrap()["alive"].as_bool().unwrap_or(true));
}
