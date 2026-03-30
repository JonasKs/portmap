pub mod db;
pub mod known_ports;
pub mod scanner;
pub mod template;

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{
        Html, IntoResponse, Response,
        sse::{Event, Sse},
    },
    routing::{get, post},
};
use serde::Serialize;
use sqlx::SqlitePool;
use tokio::sync::{Notify, watch};
use tower_http::cors::CorsLayer;

use crate::db::{App, CreateApp, SetTagColor, TagColor, UpdateApp};
use crate::scanner::scan_ports;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub dashboard_port: u16,
    pub scan_start: u16,
    pub scan_end: u16,
    pub updates: watch::Receiver<String>,
    pub scan_notify: Arc<Notify>,
}

#[derive(Serialize)]
struct PortInfo {
    port: u16,
    name: Option<String>,
    category: Option<String>,
    registered: bool,
    alive: bool,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(dashboard))
        .route("/markdown", get(dashboard_markdown))
        .route("/api/ports", get(list_ports))
        .route("/api/apps", get(list_apps).post(create_app))
        .route("/api/apps/bulk", post(bulk_create_apps))
        .route(
            "/api/apps/{id}",
            get(get_app).put(update_app).delete(delete_app),
        )
        .route("/events", get(sse_handler))
        .route("/api/refresh", post(trigger_refresh))
        .route("/api/tag-colors", get(list_tag_colors))
        .route(
            "/api/tag-colors/{category}",
            axum::routing::put(set_tag_color).delete(delete_tag_color),
        )
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Create a router backed by an in-memory `SQLite` database (for tests).
pub async fn create_router_with_test_db() -> Router {
    let pool = SqlitePool::connect(":memory:")
        .await
        .expect("Failed to create test db");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let (_tx, rx) = watch::channel(String::new());

    let state = AppState {
        db: pool,
        dashboard_port: 1337,
        scan_start: 1000,
        scan_end: 9999,
        updates: rx,
        scan_notify: Arc::new(Notify::new()),
    };
    create_router(state)
}

// -- Shared background scanner --

/// Background task that scans ports periodically and broadcasts changes via
/// the watch channel. CRUD handlers wake it early via `scan_notify`.
pub async fn scanner_loop(
    db: SqlitePool,
    scan_start: u16,
    scan_end: u16,
    dashboard_port: u16,
    tx: watch::Sender<String>,
    notify: Arc<Notify>,
) {
    let mut prev_json = String::new();
    let mut forced = false;
    loop {
        let alive = scan_ports(scan_start, scan_end, dashboard_port).await;
        let apps = db::list_apps(&db).await.unwrap_or_default();
        let tag_colors = db::list_tag_colors(&db).await.unwrap_or_default();

        let rows = template::build_rows(&alive, &apps);
        let total = rows.len();
        let plural = if total == 1 { "" } else { "s" };
        let categories = template::extract_categories(&apps);
        let filters_html = template::render_filters(&categories);
        let custom_css = template::render_custom_css(&tag_colors);

        let payload = serde_json::json!({
            "pill": format!("{total} port{plural}"),
            "rows": rows,
            "filters_html": filters_html,
            "custom_css": custom_css,
        });

        let json = payload.to_string();
        if json != prev_json || forced {
            prev_json.clone_from(&json);
            let _ = tx.send(json);
        }

        forced = false;
        tokio::select! {
            () = tokio::time::sleep(std::time::Duration::from_secs(10)) => {},
            () = notify.notified() => {
                forced = true;
                // Brief delay to let DB writes settle
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            },
        }
    }
}

// -- Port scanning --

async fn list_ports(State(state): State<AppState>) -> Json<Vec<PortInfo>> {
    let alive = scan_ports(state.scan_start, state.scan_end, state.dashboard_port).await;
    let apps = db::list_apps(&state.db).await.unwrap_or_default();

    let mut ports: Vec<PortInfo> = alive
        .iter()
        .map(|&port| {
            let app = apps.iter().find(|a| a.port == i64::from(port));
            if let Some(app) = app {
                let name = if app.name.is_empty() {
                    None
                } else {
                    Some(app.name.clone())
                };
                PortInfo {
                    port,
                    name,
                    category: Some(app.category.clone()),
                    registered: true,
                    alive: true,
                }
            } else if let Some(known) = known_ports::lookup(port) {
                PortInfo {
                    port,
                    name: Some(known.name.to_string()),
                    category: Some("macos".to_string()),
                    registered: false,
                    alive: true,
                }
            } else {
                PortInfo {
                    port,
                    name: None,
                    category: None,
                    registered: false,
                    alive: true,
                }
            }
        })
        .collect();

    for app in &apps {
        let port = u16::try_from(app.port).unwrap_or(0);
        if !alive.contains(&port) {
            let name = if app.name.is_empty() {
                None
            } else {
                Some(app.name.clone())
            };
            ports.push(PortInfo {
                port,
                name,
                category: Some(app.category.clone()),
                registered: true,
                alive: false,
            });
        }
    }

    ports.sort_by_key(|p| p.port);
    Json(ports)
}

// -- App CRUD --

async fn list_apps(State(state): State<AppState>) -> Result<Json<Vec<App>>, StatusCode> {
    db::list_apps(&state.db)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_app(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<App>, StatusCode> {
    db::get_app(&state.db, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn create_app(
    State(state): State<AppState>,
    Json(body): Json<CreateApp>,
) -> Result<(StatusCode, Json<App>), StatusCode> {
    let app = db::create_app(&state.db, &body)
        .await
        .map_err(|_| StatusCode::CONFLICT)?;
    state.scan_notify.notify_one();
    Ok((StatusCode::CREATED, Json(app)))
}

async fn bulk_create_apps(
    State(state): State<AppState>,
    Json(body): Json<Vec<CreateApp>>,
) -> Result<(StatusCode, Json<Vec<App>>), StatusCode> {
    let mut created = Vec::with_capacity(body.len());
    for app in &body {
        match db::create_app(&state.db, app).await {
            Ok(a) => created.push(a),
            Err(_) => {
                if let Ok(Some(existing)) = db::find_app_by_port(&state.db, app.port).await {
                    let update = UpdateApp {
                        name: app.name.clone(),
                        port: None,
                        category: app.category.clone(),
                    };
                    if let Ok(Some(updated)) = db::update_app(&state.db, existing.id, &update).await
                    {
                        created.push(updated);
                    }
                }
            }
        }
    }
    if !created.is_empty() {
        state.scan_notify.notify_one();
    }
    Ok((StatusCode::CREATED, Json(created)))
}

async fn update_app(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateApp>,
) -> Result<Json<App>, StatusCode> {
    let app = db::update_app(&state.db, id, &body)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    state.scan_notify.notify_one();
    Ok(Json(app))
}

async fn delete_app(State(state): State<AppState>, Path(id): Path<i64>) -> StatusCode {
    match db::delete_app(&state.db, id).await {
        Ok(true) => {
            state.scan_notify.notify_one();
            StatusCode::NO_CONTENT
        }
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn trigger_refresh(State(state): State<AppState>) -> StatusCode {
    state.scan_notify.notify_one();
    StatusCode::NO_CONTENT
}

// -- SSE live updates --

async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let mut rx = state.updates.clone();
    let stream = async_stream::stream! {
        while rx.changed().await.is_ok() {
            let data = rx.borrow_and_update().clone();
            if !data.is_empty() {
                yield Ok(Event::default().event("refresh").data(data));
            }
        }
    };
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new().interval(std::time::Duration::from_secs(15)),
    )
}

// -- Tag colors --

async fn list_tag_colors(State(state): State<AppState>) -> Result<Json<Vec<TagColor>>, StatusCode> {
    db::list_tag_colors(&state.db)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn set_tag_color(
    State(state): State<AppState>,
    Path(category): Path<String>,
    Json(body): Json<SetTagColor>,
) -> Result<Json<TagColor>, StatusCode> {
    let tc = db::set_tag_color(&state.db, &category, &body.color)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.scan_notify.notify_one();
    Ok(Json(tc))
}

async fn delete_tag_color(
    State(state): State<AppState>,
    Path(category): Path<String>,
) -> StatusCode {
    match db::delete_tag_color(&state.db, &category).await {
        Ok(true) => {
            state.scan_notify.notify_one();
            StatusCode::NO_CONTENT
        }
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// -- Dashboard --

async fn dashboard(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let wants_markdown = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("text/markdown"));

    if wants_markdown {
        return dashboard_markdown_inner(&state).await;
    }

    let alive = scan_ports(state.scan_start, state.scan_end, state.dashboard_port).await;
    let apps = db::list_apps(&state.db).await.unwrap_or_default();
    let tag_colors = db::list_tag_colors(&state.db).await.unwrap_or_default();
    let html = template::render(
        &alive,
        &apps,
        state.scan_start,
        state.scan_end,
        state.dashboard_port,
        &tag_colors,
    );
    ([(header::VARY, "Accept")], Html(html)).into_response()
}

async fn dashboard_markdown(State(state): State<AppState>) -> Response {
    dashboard_markdown_inner(&state).await
}

async fn dashboard_markdown_inner(state: &AppState) -> Response {
    let alive = scan_ports(state.scan_start, state.scan_end, state.dashboard_port).await;
    let apps = db::list_apps(&state.db).await.unwrap_or_default();
    let md = render_markdown(&alive, &apps, state.dashboard_port);
    (
        [
            (header::CONTENT_TYPE, "text/markdown; charset=utf-8"),
            (header::VARY, "Accept"),
        ],
        md,
    )
        .into_response()
}

pub fn render_markdown(alive_ports: &[u16], apps: &[App], dashboard_port: u16) -> String {
    use std::fmt::Write;

    let mut md = format!(
        "---\ntitle: portmap\nurl: http://localhost:{dashboard_port}\n---\n\n\
         # portmap\n\n\
         Map names to localhost ports. Made for agents and humans.\n\n"
    );

    if apps.is_empty() {
        md.push_str("No registered apps. Use the API to add some.\n\n");
    } else {
        let _ = writeln!(
            md,
            "## Registered Apps\n\n| Name | Port | Category | Status |\n|------|------|----------|--------|"
        );
        for app in apps {
            let port = u16::try_from(app.port).unwrap_or(0);
            let status = if alive_ports.contains(&port) {
                "alive"
            } else {
                "down"
            };
            let name = if app.name.is_empty() {
                format!(":{}", app.port)
            } else {
                app.name.clone()
            };
            let _ = writeln!(
                md,
                "| {} | {} | {} | {status} |",
                name, app.port, app.category
            );
        }
    }

    let unregistered: Vec<u16> = alive_ports
        .iter()
        .filter(|&&p| !apps.iter().any(|a| a.port == i64::from(p)))
        .copied()
        .collect();
    if !unregistered.is_empty() {
        let _ = writeln!(
            md,
            "\n## Other Open Ports\n\n| Port | Name | URL |\n|------|------|-----|"
        );
        for port in &unregistered {
            let name = known_ports::lookup(*port).map_or("—", |k| k.name);
            let _ = writeln!(md, "| {port} | {name} | http://localhost:{port} |");
        }
    }

    let _ = write!(
        md,
        r#"
## API Reference

Base URL: `http://localhost:{dashboard_port}`

### List all open ports (with app info)

```bash
curl http://localhost:{dashboard_port}/api/ports
```

### List registered apps

```bash
curl http://localhost:{dashboard_port}/api/apps
```

### Register a new app

```bash
curl -X POST http://localhost:{dashboard_port}/api/apps \
  -H "Content-Type: application/json" \
  -d '{{"name": "my-app", "port": 3000, "category": "frontend"}}'
```

### Bulk register apps

```bash
curl -X POST http://localhost:{dashboard_port}/api/apps/bulk \
  -H "Content-Type: application/json" \
  -d '[
    {{"name": "api", "port": 8080, "category": "backend"}},
    {{"name": "web", "port": 3000, "category": "frontend"}}
  ]'
```

### Update an app

```bash
curl -X PUT http://localhost:{dashboard_port}/api/apps/1 \
  -H "Content-Type: application/json" \
  -d '{{"name": "new-name", "category": "backend"}}'
```

### Delete an app

```bash
curl -X DELETE http://localhost:{dashboard_port}/api/apps/1
```
"#
    );

    md
}
