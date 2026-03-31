pub mod container;
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

/// Tracks how many SSE clients are connected.
pub type SseClients = Arc<std::sync::atomic::AtomicUsize>;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub dashboard_port: u16,
    pub scan_start: u16,
    pub scan_end: u16,
    pub updates: watch::Receiver<String>,
    pub updates_tx: Arc<watch::Sender<String>>,
    pub scan_active: watch::Receiver<bool>,
    pub scan_active_tx: Arc<watch::Sender<bool>>,
    pub scan_notify: Arc<Notify>,
    pub sse_clients: SseClients,
}

#[derive(Serialize)]
struct PortInfo {
    port: u16,
    name: Option<String>,
    category: Option<String>,
    registered: bool,
    alive: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
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
        .route("/api/kill/{port}", post(kill_port))
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

    let (tx, rx) = watch::channel(String::new());
    let (sa_tx, sa_rx) = watch::channel(false);

    let state = AppState {
        db: pool,
        dashboard_port: 1337,
        scan_start: 1000,
        scan_end: 9999,
        updates: rx,
        updates_tx: Arc::new(tx),
        scan_active: sa_rx,
        scan_active_tx: Arc::new(sa_tx),
        scan_notify: Arc::new(Notify::new()),
        sse_clients: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    };
    create_router(state)
}

// -- Shared background scanner --

/// Signal that a full scan is starting, for SSE scan-status events.
fn signal_scan_start(state: &AppState) {
    let _ = state.scan_active_tx.send(true);
}

/// Signal that a full scan completed.
fn signal_scan_end(state: &AppState) {
    let _ = state.scan_active_tx.send(false);
}

/// Build the SSE payload JSON from scan results and publish it to SSE clients.
async fn publish_scan(state: &AppState, alive: &[u16]) {
    let apps = db::list_apps(&state.db).await.unwrap_or_default();
    let tag_colors = db::list_tag_colors(&state.db).await.unwrap_or_default();
    let container_ports = container::discover().await;

    // Merge container ports into alive list
    let mut merged_alive = alive.to_vec();
    for cp in &container_ports {
        if !merged_alive.contains(&cp.port) && cp.port != state.dashboard_port {
            merged_alive.push(cp.port);
        }
    }
    merged_alive.sort_unstable();

    let rows = template::build_rows(&merged_alive, &apps, &container_ports);
    let total = rows.len();
    let plural = if total == 1 { "" } else { "s" };
    let categories = template::extract_categories(&apps, &container_ports);
    let filters_html = template::render_filters(&categories, &tag_colors);
    let custom_css = template::render_custom_css(&tag_colors);

    let payload = serde_json::json!({
        "pill": format!("{total} port{plural}"),
        "rows": rows,
        "filters_html": filters_html,
        "custom_css": custom_css,
        "discovered": true,
    });

    let _ = state.updates_tx.send(payload.to_string());
}

const ACTIVE_INTERVAL_SECS: u64 = 10;
const IDLE_INTERVAL_SECS: u64 = 30;
const ACTIVE_FULL_EVERY: u32 = 6; // 6 × 10s = 60s
const IDLE_FULL_EVERY: u32 = 10; // 10 × 30s = 5min

/// Background task that scans ports periodically and broadcasts changes via
/// the watch channel. CRUD handlers wake it early via `scan_notify`.
#[allow(clippy::too_many_arguments)]
pub async fn scanner_loop(
    db: SqlitePool,
    scan_start: u16,
    scan_end: u16,
    dashboard_port: u16,
    tx: Arc<watch::Sender<String>>,
    scan_active_tx: Arc<watch::Sender<bool>>,
    notify: Arc<Notify>,
    sse_clients: SseClients,
) {
    use std::sync::atomic::Ordering;

    let mut prev_json = String::new();
    let mut forced = false;
    let mut tick: u32 = 0;
    let mut discovered_ports: Vec<u16> = Vec::new();
    let mut cached_container_ports: Vec<container::ContainerPort> = Vec::new();

    loop {
        let active = sse_clients.load(Ordering::Relaxed) > 0;
        let full_every = if active {
            ACTIVE_FULL_EVERY
        } else {
            IDLE_FULL_EVERY
        };
        let interval = if active {
            ACTIVE_INTERVAL_SECS
        } else {
            IDLE_INTERVAL_SECS
        };

        let apps = db::list_apps(&db).await.unwrap_or_default();
        let tag_colors = db::list_tag_colors(&db).await.unwrap_or_default();

        // Full discovery on first run, at interval, or when forced (CRUD/refresh/SSE connect)
        let is_full_scan = tick == 0 || tick.is_multiple_of(full_every) || forced;
        if is_full_scan {
            let _ = scan_active_tx.send(true);
        }
        let mut alive = if is_full_scan {
            let full = scan_ports(scan_start, scan_end, dashboard_port).await;
            discovered_ports.clone_from(&full);
            full
        } else {
            // Quick check: only probe registered + previously discovered ports
            let mut check_ports: Vec<u16> = apps
                .iter()
                .filter_map(|a| u16::try_from(a.port).ok())
                .collect();
            for &p in &discovered_ports {
                if !check_ports.contains(&p) {
                    check_ports.push(p);
                }
            }
            check_ports.sort_unstable();
            check_ports.dedup();
            scanner::probe_ports(&check_ports, dashboard_port).await
        };

        // Container discovery on full scans, cached for quick probes
        if is_full_scan {
            cached_container_ports = container::discover().await;
        }
        // Add container ports that the TCP scan might have missed
        for cp in &cached_container_ports {
            if !alive.contains(&cp.port) && cp.port != dashboard_port {
                alive.push(cp.port);
            }
        }
        alive.sort_unstable();

        let rows = template::build_rows(&alive, &apps, &cached_container_ports);
        let total = rows.len();
        let plural = if total == 1 { "" } else { "s" };
        let categories = template::extract_categories(&apps, &cached_container_ports);
        let filters_html = template::render_filters(&categories, &tag_colors);
        let custom_css = template::render_custom_css(&tag_colors);

        let payload = serde_json::json!({
            "pill": format!("{total} port{plural}"),
            "rows": rows,
            "filters_html": filters_html,
            "custom_css": custom_css,
            "discovered": is_full_scan,
        });

        let json = payload.to_string();
        if json != prev_json || forced {
            prev_json.clone_from(&json);
            let _ = tx.send(json);
        }
        if is_full_scan {
            let _ = scan_active_tx.send(false);
        }

        tick = tick.wrapping_add(1);
        forced = false;
        tokio::select! {
            () = tokio::time::sleep(std::time::Duration::from_secs(interval)) => {},
            () = notify.notified() => {
                forced = true;
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            },
        }
    }
}

// -- Port scanning --

async fn list_ports(State(state): State<AppState>) -> Json<Vec<PortInfo>> {
    signal_scan_start(&state);
    let alive = scan_ports(state.scan_start, state.scan_end, state.dashboard_port).await;
    let apps = db::list_apps(&state.db).await.unwrap_or_default();
    let container_ports = container::discover().await;
    let container_map: std::collections::HashMap<u16, &container::ContainerPort> =
        container_ports.iter().map(|cp| (cp.port, cp)).collect();

    let mut ports: Vec<PortInfo> = alive
        .iter()
        .map(|&port| {
            let app = apps.iter().find(|a| a.port == i64::from(port));
            let cp = container_map.get(&port);
            let source = cp.map(|c| c.source.clone());
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
                    source,
                }
            } else if let Some(c) = cp {
                PortInfo {
                    port,
                    name: Some(c.container_name.clone()),
                    category: Some(c.source.clone()),
                    registered: false,
                    alive: true,
                    source,
                }
            } else if let Some(known) = known_ports::lookup(port) {
                PortInfo {
                    port,
                    name: Some(known.name.to_string()),
                    category: Some("macos".to_string()),
                    registered: false,
                    alive: true,
                    source: None,
                }
            } else {
                PortInfo {
                    port,
                    name: None,
                    category: None,
                    registered: false,
                    alive: true,
                    source,
                }
            }
        })
        .collect();

    // Add container ports not found in TCP scan
    for cp in &container_ports {
        if !alive.contains(&cp.port) && cp.port != state.dashboard_port {
            ports.push(PortInfo {
                port: cp.port,
                name: Some(cp.container_name.clone()),
                category: Some(cp.source.clone()),
                registered: false,
                alive: true,
                source: Some(cp.source.clone()),
            });
        }
    }

    for app in &apps {
        let port = u16::try_from(app.port).unwrap_or(0);
        if !alive.contains(&port) && !container_ports.iter().any(|cp| cp.port == port) {
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
                source: None,
            });
        }
    }

    ports.sort_by_key(|p| p.port);
    // Publish scan results directly to SSE clients (no duplicate scan)
    publish_scan(&state, &alive).await;
    signal_scan_end(&state);
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

async fn kill_port(State(state): State<AppState>, Path(port): Path<u16>) -> StatusCode {
    let output = std::process::Command::new("lsof")
        .args(["-ti", &format!(":{port}"), "-sTCP:LISTEN"])
        .output();

    let Ok(output) = output else {
        return StatusCode::INTERNAL_SERVER_ERROR;
    };

    let pids: Vec<&str> = std::str::from_utf8(&output.stdout)
        .unwrap_or("")
        .lines()
        .filter(|l| !l.is_empty())
        .collect();

    if pids.is_empty() {
        return StatusCode::NOT_FOUND;
    }

    for pid in &pids {
        let _ = std::process::Command::new("kill").arg(pid).status();
    }

    state.scan_notify.notify_one();
    StatusCode::NO_CONTENT
}

// -- SSE live updates --

/// Guard that decrements the SSE client counter on drop (including cancellation).
struct SseGuard(SseClients);

impl Drop for SseGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, std::convert::Infallible>>> {
    use std::sync::atomic::Ordering;

    state.sse_clients.fetch_add(1, Ordering::Relaxed);
    state.scan_notify.notify_one();

    let guard = SseGuard(state.sse_clients.clone());
    let mut rx = state.updates.clone();
    let mut scan_rx = state.scan_active.clone();
    // Mark current values as seen so we only send *new* updates
    rx.borrow_and_update();
    scan_rx.borrow_and_update();
    let stream = async_stream::stream! {
        let _guard = guard;
        loop {
            tokio::select! {
                result = rx.changed() => {
                    if result.is_err() { break; }
                    let data = rx.borrow_and_update().clone();
                    if !data.is_empty() {
                        yield Ok(Event::default().event("refresh").data(data));
                    }
                }
                result = scan_rx.changed() => {
                    if result.is_err() { break; }
                    let active = *scan_rx.borrow_and_update();
                    yield Ok(Event::default().event("scan").data(if active { "start" } else { "done" }));
                }
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

    // Quick probe of known ports + container discovery for fast first paint
    let apps = db::list_apps(&state.db).await.unwrap_or_default();
    let container_ports = container::discover().await;
    let mut alive = scanner::probe_ports(
        &apps
            .iter()
            .filter_map(|a| u16::try_from(a.port).ok())
            .collect::<Vec<_>>(),
        state.dashboard_port,
    )
    .await;
    // Add container ports
    for cp in &container_ports {
        if !alive.contains(&cp.port) && cp.port != state.dashboard_port {
            alive.push(cp.port);
        }
    }
    alive.sort_unstable();
    let tag_colors = db::list_tag_colors(&state.db).await.unwrap_or_default();
    let html = template::render(
        &alive,
        &apps,
        state.scan_start,
        state.scan_end,
        state.dashboard_port,
        &tag_colors,
        &container_ports,
    );
    // Trigger background full scan — SSE will deliver it
    state.scan_notify.notify_one();
    ([(header::VARY, "Accept")], Html(html)).into_response()
}

async fn dashboard_markdown(State(state): State<AppState>) -> Response {
    dashboard_markdown_inner(&state).await
}

async fn dashboard_markdown_inner(state: &AppState) -> Response {
    signal_scan_start(state);
    let alive = scan_ports(state.scan_start, state.scan_end, state.dashboard_port).await;
    let apps = db::list_apps(&state.db).await.unwrap_or_default();
    let container_ports = container::discover().await;
    let md = render_markdown(&alive, &apps, state.dashboard_port, &container_ports);
    // Publish scan results directly to SSE clients (no duplicate scan)
    publish_scan(state, &alive).await;
    signal_scan_end(state);
    (
        [
            (header::CONTENT_TYPE, "text/markdown; charset=utf-8"),
            (header::VARY, "Accept"),
        ],
        md,
    )
        .into_response()
}

pub fn render_markdown(
    alive_ports: &[u16],
    apps: &[App],
    dashboard_port: u16,
    container_ports: &[container::ContainerPort],
) -> String {
    use std::fmt::Write;

    let mut md = format!(
        "---\ntitle: portmap\nurl: http://localhost:{dashboard_port}\n---\n\n\
         # portmap (:{dashboard_port})\n\n\
         Map names to localhost ports. Made for agents and humans.\n\n"
    );

    let container_map: std::collections::HashMap<u16, &container::ContainerPort> =
        container_ports.iter().map(|cp| (cp.port, cp)).collect();

    if apps.is_empty() {
        md.push_str("No registered apps. Use the API to add some.\n\n");
    } else {
        let _ = writeln!(
            md,
            "## Registered Apps\n\n| Port | Name | Category | Source | Status |\n|------|------|----------|--------|--------|"
        );
        for app in apps {
            let port = u16::try_from(app.port).unwrap_or(0);
            let status = if alive_ports.contains(&port) {
                "up"
            } else {
                "down"
            };
            let name = if app.name.is_empty() {
                "-".to_string()
            } else {
                app.name.clone()
            };
            let source = container_map.get(&port).map_or("", |cp| cp.source.as_str());
            let _ = writeln!(
                md,
                "| {} | {} | {} | {source} | {status} |",
                app.port, name, app.category
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
            "\n## Other Open Ports\n\n| Port | Name | Source | Status |\n|------|------|--------|--------|"
        );
        for port in &unregistered {
            let (name, source) = if let Some(cp) = container_map.get(port) {
                (cp.container_name.as_str(), cp.source.as_str())
            } else {
                (known_ports::lookup(*port).map_or("-", |k| k.name), "")
            };
            let _ = writeln!(md, "| {port} | {name} | {source} | up |");
        }
    }

    let _ = write!(
        md,
        r#"
## API

Base URL: `http://localhost:{dashboard_port}`

### Get all ports

```bash
curl http://localhost:{dashboard_port}/api/ports
```

Returns JSON array of all ports with name, category, registered status, and alive status.

### Register a port

```bash
curl -X POST http://localhost:{dashboard_port}/api/apps \
  -H "Content-Type: application/json" \
  -d '{{"name": "my-app", "port": 3000, "category": "frontend"}}'
```

### Update a registration

```bash
curl -X PUT http://localhost:{dashboard_port}/api/apps/ID \
  -H "Content-Type: application/json" \
  -d '{{"name": "new-name", "category": "backend"}}'
```

### Remove a registration

```bash
curl -X DELETE http://localhost:{dashboard_port}/api/apps/ID
```

### Kill a process

```bash
curl -X POST http://localhost:{dashboard_port}/api/kill/PORT
```
"#
    );

    md
}
