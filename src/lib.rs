pub mod db;
pub mod known_ports;
pub mod scanner;
pub mod template;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::Serialize;
use sqlx::SqlitePool;
use tower_http::cors::CorsLayer;

use crate::db::{App, CreateApp, UpdateApp};
use crate::scanner::scan_ports;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub dashboard_port: u16,
    pub scan_start: u16,
    pub scan_end: u16,
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

    let state = AppState {
        db: pool,
        dashboard_port: 1337,
        scan_start: 1000,
        scan_end: 9999,
    };
    create_router(state)
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
                PortInfo {
                    port,
                    name: Some(app.name.clone()),
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
            ports.push(PortInfo {
                port,
                name: Some(app.name.clone()),
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
    db::create_app(&state.db, &body)
        .await
        .map(|app| (StatusCode::CREATED, Json(app)))
        .map_err(|_| StatusCode::CONFLICT)
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
                        name: Some(app.name.clone()),
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
    Ok((StatusCode::CREATED, Json(created)))
}

async fn update_app(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateApp>,
) -> Result<Json<App>, StatusCode> {
    db::update_app(&state.db, id, &body)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn delete_app(State(state): State<AppState>, Path(id): Path<i64>) -> StatusCode {
    match db::delete_app(&state.db, id).await {
        Ok(true) => StatusCode::NO_CONTENT,
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
    let html = template::render(
        &alive,
        &apps,
        state.scan_start,
        state.scan_end,
        state.dashboard_port,
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
            let _ = writeln!(
                md,
                "| {} | {} | {} | {status} |",
                app.name, app.port, app.category
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
            let name = known_ports::lookup(*port)
                .map_or("—", |k| k.name);
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
