mod db;
mod scanner;
mod template;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use clap::Parser;
use serde::Serialize;
use sqlx::SqlitePool;
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::db::{App, CreateApp, UpdateApp};
use crate::scanner::scan_ports;

#[derive(Parser)]
#[command(name = "portmap", about = "Map names to localhost ports. Made for agents and humans.")]
struct Cli {
    /// Port to run the dashboard on
    #[arg(short, long, default_value = "1337")]
    port: u16,

    /// Database file path
    #[arg(short, long, default_value = "~/.portmap.db")]
    database: String,

    /// Port range start (inclusive)
    #[arg(long, default_value = "1000")]
    scan_start: u16,

    /// Port range end (inclusive)
    #[arg(long, default_value = "9999")]
    scan_end: u16,

    /// Uninstall: stop the launch agent, remove plist and database
    #[arg(long)]
    uninstall: bool,
}

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
    dashboard_port: u16,
    scan_start: u16,
    scan_end: u16,
}

#[derive(Serialize)]
struct PortInfo {
    port: u16,
    name: Option<String>,
    category: Option<String>,
    registered: bool,
    alive: bool,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "portmap=info".into()),
        )
        .init();

    let cli = Cli::parse();

    if cli.uninstall {
        uninstall(&cli.database);
        return;
    }

    let db_path = shellexpand(&cli.database);
    let db = db::init(&db_path)
        .await
        .expect("Failed to initialize database");

    let state = AppState {
        db,
        dashboard_port: cli.port,
        scan_start: cli.scan_start,
        scan_end: cli.scan_end,
    };

    let app = Router::new()
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
        .with_state(state);

    let addr = format!("127.0.0.1:{}", cli.port);
    info!("portmap running at http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app).await.expect("Server error");
}

// -- Port scanning --

async fn list_ports(State(state): State<AppState>) -> Json<Vec<PortInfo>> {
    let alive = scan_ports(state.scan_start, state.scan_end, state.dashboard_port).await;
    let apps = db::list_apps(&state.db).await.unwrap_or_default();

    let mut ports: Vec<PortInfo> = alive
        .iter()
        .map(|&port| {
            let app = apps.iter().find(|a| a.port == i64::from(port));
            PortInfo {
                port,
                name: app.map(|a| a.name.clone()),
                category: app.map(|a| a.category.clone()),
                registered: app.is_some(),
                alive: true,
            }
        })
        .collect();

    // Include registered apps that weren't found alive
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
                // If port already exists, try to update the name/category
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

async fn delete_app(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> StatusCode {
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

fn render_markdown(alive_ports: &[u16], apps: &[App], dashboard_port: u16) -> String {
    use std::fmt::Write;

    let mut md = format!(
        "---\ntitle: portmap\nurl: http://localhost:{dashboard_port}\n---\n\n\
         # portmap\n\n\
         Map names to localhost ports. Made for agents and humans.\n\n"
    );

    // Registered apps
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

    // Unregistered open ports
    let unregistered: Vec<u16> = alive_ports
        .iter()
        .filter(|&&p| !apps.iter().any(|a| a.port == i64::from(p)))
        .copied()
        .collect();
    if !unregistered.is_empty() {
        let _ = writeln!(
            md,
            "\n## Unregistered Open Ports\n\n| Port | URL |\n|------|-----|"
        );
        for port in &unregistered {
            let _ = writeln!(md, "| {port} | http://localhost:{port} |");
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

fn uninstall(db_flag: &str) {
    use std::process::Command;

    let uid = Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let uid = uid.trim();

    let plist = shellexpand("~/Library/LaunchAgents/dev.portmap.plist");

    // Stop the launch agent
    let target = format!("gui/{uid}");
    let _ = Command::new("launchctl")
        .args(["bootout", &target, &plist])
        .status();

    // Remove plist
    if std::fs::remove_file(&plist).is_ok() {
        info!("Removed {plist}");
    }

    // Remove database
    let db_path = shellexpand(db_flag);
    if std::fs::remove_file(&db_path).is_ok() {
        info!("Removed {db_path}");
    }

    // Remove binary
    if let Ok(exe) = std::env::current_exe() {
        let path = exe.display().to_string();
        if std::fs::remove_file(&exe).is_ok() {
            info!("Removed {path}");
        }
    }

    println!("portmap has been uninstalled.");
}

fn shellexpand(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return format!("{home}/{rest}");
    }
    path.to_string()
}
