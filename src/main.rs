use clap::{Parser, Subcommand};
use portmap::AppState;
use portmap::config::{self, Config};
use tracing::info;

#[derive(Parser)]
#[command(
    name = "portmap",
    about = "Map names to localhost ports. Made for agents and humans.",
    version
)]
struct Cli {
    /// Database file path (default: ~/.config/portmap/portmap.db)
    #[arg(short, long, default_value = config::DEFAULT_DB_SENTINEL, global = true)]
    database: String,

    /// Port for the dashboard server
    #[arg(long, global = true)]
    listen: Option<u16>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Start the dashboard server (default if no command given)
    Serve {
        /// Port range start (inclusive)
        #[arg(long)]
        scan_start: Option<u16>,

        /// Port range end (inclusive)
        #[arg(long)]
        scan_end: Option<u16>,
    },

    /// List all ports (registered apps + open ports)
    List,

    /// Add an app
    Add {
        /// App name (optional — can tag a port without naming it)
        #[arg(short, long)]
        name: Option<String>,

        /// Port number
        #[arg(short = 'P', long)]
        port: i64,

        /// Category tag (e.g. frontend, backend, mcp)
        #[arg(short, long, default_value = "other")]
        category: String,
    },

    /// Remove an app by port or name
    Remove {
        /// Port number or app name
        target: String,
    },

    /// Update an app by port or name
    Update {
        /// Port number or app name
        target: String,

        /// New name
        #[arg(short, long)]
        name: Option<String>,

        /// New port
        #[arg(short = 'P', long)]
        port: Option<i64>,

        /// New category
        #[arg(short, long)]
        category: Option<String>,
    },

    /// Kill the process running on a port
    Kill {
        /// Port number or app name
        target: String,
    },

    /// Install as a launch agent (macOS) or systemd service (Linux)
    Install,

    /// Uninstall: stop service, remove config and database
    Uninstall,

    /// Show service status
    Status,
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
    let cfg = config::load();
    let db_path = config::resolve_db_path(&cli.database);
    let listen = cli.listen.unwrap_or_else(|| cfg.listen());

    match cli.command {
        None | Some(Command::Serve { .. }) => {
            let (scan_start, scan_end) = match &cli.command {
                Some(Command::Serve {
                    scan_start,
                    scan_end,
                }) => (
                    scan_start.unwrap_or_else(|| cfg.scan_start()),
                    scan_end.unwrap_or_else(|| cfg.scan_end()),
                ),
                _ => (cfg.scan_start(), cfg.scan_end()),
            };
            cmd_serve(&db_path, listen, scan_start, scan_end).await;
        }
        Some(Command::List) => cmd_list(&db_path, listen).await,
        Some(Command::Add {
            name,
            port,
            category,
        }) => cmd_add(&db_path, name.as_deref(), port, &category).await,
        Some(Command::Remove { target }) => cmd_remove(&db_path, &target).await,
        Some(Command::Update {
            target,
            name,
            port,
            category,
        }) => cmd_update(&db_path, &target, name, port, category).await,
        Some(Command::Kill { target }) => cmd_kill(&db_path, &target).await,
        Some(Command::Install) => cmd_install(&cfg, listen),
        Some(Command::Uninstall) => cmd_uninstall(&db_path),
        Some(Command::Status) => cmd_status(listen).await,
    }
}

async fn cmd_serve(db_path: &str, port: u16, scan_start: u16, scan_end: u16) {
    let db = portmap::db::init(db_path)
        .await
        .expect("Failed to initialize database");

    let (tx, rx) = tokio::sync::watch::channel(String::new());
    let tx = std::sync::Arc::new(tx);
    let (sa_tx, sa_rx) = tokio::sync::watch::channel(false);
    let sa_tx = std::sync::Arc::new(sa_tx);
    let scan_notify = std::sync::Arc::new(tokio::sync::Notify::new());
    let sse_clients = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let state = AppState {
        db: db.clone(),
        dashboard_port: port,
        scan_start,
        scan_end,
        updates: rx,
        updates_tx: tx.clone(),
        scan_active: sa_rx,
        scan_active_tx: sa_tx.clone(),
        scan_notify: scan_notify.clone(),
        sse_clients: sse_clients.clone(),
    };

    tokio::spawn(portmap::scanner_loop(
        db,
        scan_start,
        scan_end,
        port,
        tx,
        sa_tx,
        scan_notify,
        sse_clients,
    ));

    let app = portmap::create_router(state);

    let addr = format!("127.0.0.1:{port}");
    info!("portmap running at http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app).await.expect("Server error");
}

async fn cmd_list(db_path: &str, dashboard_port: u16) {
    let db = portmap::db::init(db_path)
        .await
        .expect("Failed to open database");

    let apps = portmap::db::list_apps(&db)
        .await
        .expect("Failed to list apps");
    let mut alive = portmap::scanner::scan_ports(1000, 9999, 0).await;
    let container_ports = portmap::container::discover().await;

    // Add container ports that TCP scan might have missed
    for cp in &container_ports {
        if !alive.contains(&cp.port) {
            alive.push(cp.port);
        }
    }
    alive.sort_unstable();

    if apps.is_empty() && alive.is_empty() {
        println!("No ports found.");
        return;
    }

    let container_map: std::collections::HashMap<u16, &portmap::container::ContainerPort> =
        container_ports.iter().map(|cp| (cp.port, cp)).collect();

    // Merge registered apps, container ports, and unregistered open ports
    // (name, port, category, source, status)
    let mut registered_ports: std::collections::HashSet<u16> = std::collections::HashSet::new();
    let mut rows: Vec<(String, u16, String, String, &str)> = Vec::new();

    // Portmap itself at the top
    let pm_status = if alive.contains(&dashboard_port) {
        "up"
    } else {
        "down"
    };
    rows.push((
        "portmap".to_string(),
        dashboard_port,
        "portmap".to_string(),
        String::new(),
        pm_status,
    ));

    for app in &apps {
        let port = u16::try_from(app.port).unwrap_or(0);
        registered_ports.insert(port);
        let name = if app.name.is_empty() {
            "-".to_string()
        } else {
            app.name.clone()
        };
        let source = container_map
            .get(&port)
            .map_or(String::new(), |cp| cp.source.clone());
        let status = if alive.contains(&port) { "up" } else { "down" };
        rows.push((name, port, app.category.clone(), source, status));
    }

    // Sort registered rows (skip portmap at index 0) by port
    rows[1..].sort_by_key(|r| r.1);

    for &port in &alive {
        if registered_ports.contains(&port) || port == dashboard_port {
            continue;
        }
        let (name, category) = if let Some(cp) = container_map.get(&port) {
            (cp.container_name.clone(), cp.source.clone())
        } else {
            (
                portmap::known_ports::lookup(port).map_or("-".to_string(), |k| k.name.to_string()),
                String::new(),
            )
        };
        rows.push((name, port, category, String::new(), "up"));
    }

    let w_name = std::cmp::max(rows.iter().map(|r| r.0.len()).max().unwrap_or(4), 4);
    let w_cat = std::cmp::max(rows.iter().map(|r| r.2.len()).max().unwrap_or(8), 8);

    println!(
        "{:<6} {:<w_name$}  {:<w_cat$}  STATUS",
        "PORT", "NAME", "CATEGORY"
    );
    for (name, port, category, source, status) in &rows {
        if source.is_empty() {
            println!("{port:<6} {name:<w_name$}  {category:<w_cat$}  {status}");
        } else {
            println!("{port:<6} {name:<w_name$}  {category:<w_cat$}  {status} ({source})");
        }
    }
}

async fn cmd_add(db_path: &str, name: Option<&str>, port: i64, category: &str) {
    let db = portmap::db::init(db_path)
        .await
        .expect("Failed to open database");

    let app = portmap::db::CreateApp {
        name: name.map(String::from),
        port,
        category: Some(category.to_string()),
    };

    match portmap::db::create_app(&db, &app).await {
        Ok(created) => {
            let display = if created.name.is_empty() {
                format!(":{}", created.port)
            } else {
                created.name.clone()
            };
            println!(
                "Added #{}: {} on :{} [{}]",
                created.id, display, created.port, created.category
            );
        }
        Err(_) => eprintln!("Failed — port {port} may already be registered"),
    }
}

/// Resolve a target (port number or app name) to an app.
async fn resolve_app(db: &sqlx::SqlitePool, target: &str) -> Option<portmap::db::App> {
    if let Ok(port) = target.parse::<i64>()
        && let Ok(Some(app)) = portmap::db::find_app_by_port(db, port).await
    {
        return Some(app);
    }
    if let Ok(Some(app)) = portmap::db::find_app_by_name(db, target).await {
        return Some(app);
    }
    None
}

/// Resolve a target to a port number (from DB or direct parse).
fn resolve_port(target: &str, app: Option<&portmap::db::App>) -> Option<u16> {
    if let Some(app) = app {
        return u16::try_from(app.port).ok();
    }
    target.parse::<u16>().ok()
}

async fn cmd_remove(db_path: &str, target: &str) {
    let db = portmap::db::init(db_path)
        .await
        .expect("Failed to open database");

    if let Some(app) = resolve_app(&db, target).await
        && portmap::db::delete_app(&db, app.id).await.unwrap_or(false)
    {
        let display = if app.name.is_empty() {
            format!(":{}", app.port)
        } else {
            app.name
        };
        println!("Removed {display} (port {})", app.port);
        return;
    }
    eprintln!("No app found for: {target}");
}

async fn cmd_update(
    db_path: &str,
    target: &str,
    name: Option<String>,
    port: Option<i64>,
    category: Option<String>,
) {
    let db = portmap::db::init(db_path)
        .await
        .expect("Failed to open database");

    let Some(app) = resolve_app(&db, target).await else {
        eprintln!("No app found for: {target}");
        return;
    };

    let update = portmap::db::UpdateApp {
        name,
        port,
        category,
    };

    match portmap::db::update_app(&db, app.id, &update).await {
        Ok(Some(updated)) => {
            let display = if updated.name.is_empty() {
                format!(":{}", updated.port)
            } else {
                updated.name
            };
            println!(
                "Updated {display} on :{} [{}]",
                updated.port, updated.category
            );
        }
        Ok(None) => eprintln!("No app found for: {target}"),
        Err(e) => eprintln!("Failed: {e}"),
    }
}

async fn cmd_kill(db_path: &str, target: &str) {
    use std::process::Command as Cmd;

    let db = portmap::db::init(db_path)
        .await
        .expect("Failed to open database");

    let app = resolve_app(&db, target).await;
    let Some(port) = resolve_port(target, app.as_ref()) else {
        eprintln!("Could not resolve port for: {target}");
        return;
    };

    // Find PIDs *listening* on this port (not clients connected to it)
    let output = Cmd::new("lsof")
        .args(["-ti", &format!(":{port}"), "-sTCP:LISTEN"])
        .output();

    let Ok(output) = output else {
        eprintln!("Failed to run lsof");
        return;
    };

    let pids: Vec<&str> = std::str::from_utf8(&output.stdout)
        .unwrap_or("")
        .lines()
        .filter(|l| !l.is_empty())
        .collect();

    if pids.is_empty() {
        println!("Nothing running on :{port}");
        return;
    }

    let display = app.as_ref().map_or(format!(":{port}"), |a| {
        if a.name.is_empty() {
            format!(":{port}")
        } else {
            a.name.clone()
        }
    });

    for pid in &pids {
        let _ = Cmd::new("kill").arg(pid).status();
    }
    println!("Killed {display} (port {port})");
}

fn is_homebrew_install() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .is_some_and(|p| p.display().to_string().contains("/Cellar/"))
}

fn cmd_install(cfg: &Config, port: u16) {
    use std::process::Command as Cmd;

    if is_homebrew_install() {
        println!("portmap was installed via Homebrew.");
        println!("Use brew to manage the service:\n");
        println!("  brew services start jonasks/tap/portmap");
        println!("  brew services stop jonasks/tap/portmap");
        println!("  brew services info jonasks/tap/portmap");
        return;
    }

    let exe = std::env::current_exe().expect("Failed to get binary path");
    let exe_str = exe.display().to_string();
    let scan_start = cfg.scan_start();
    let scan_end = cfg.scan_end();

    if cfg!(target_os = "macos") {
        let plist_path = shellexpand("~/Library/LaunchAgents/dev.portmap.plist");
        let uid = get_uid();

        // Stop existing (ignore errors — may not exist yet)
        let target = format!("gui/{uid}");
        let _ = Cmd::new("launchctl")
            .args(["bootout", &target, &plist_path])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>dev.portmap</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe_str}</string>
        <string>serve</string>
        <string>--listen</string>
        <string>{port}</string>
        <string>--scan-start</string>
        <string>{scan_start}</string>
        <string>--scan-end</string>
        <string>{scan_end}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/portmap.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/portmap.log</string>
</dict>
</plist>"#
        );

        if let Some(parent) = std::path::Path::new(&plist_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&plist_path, plist).expect("Failed to write plist");

        let status = Cmd::new("launchctl")
            .args(["bootstrap", &target, &plist_path])
            .status();

        if status.is_ok_and(|s| s.success()) {
            // Kick to start immediately (bootstrap only registers)
            let service = format!("{target}/dev.portmap");
            let _ = Cmd::new("launchctl")
                .args(["kickstart", &service])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            println!("Installed and started on port {port}.");
            println!("Dashboard: http://localhost:{port}");
            println!("Logs:      tail -f /tmp/portmap.log");
        } else {
            eprintln!("Failed to bootstrap launch agent.");
        }
    } else {
        // Linux systemd
        let service_dir = shellexpand("~/.config/systemd/user");
        let _ = std::fs::create_dir_all(&service_dir);

        let unit = format!(
            "[Unit]\nDescription=portmap\n\n[Service]\nExecStart={exe_str} serve --listen {port} --scan-start {scan_start} --scan-end {scan_end}\nRestart=always\n\n[Install]\nWantedBy=default.target\n"
        );

        let service_path = format!("{service_dir}/portmap.service");
        std::fs::write(&service_path, unit).expect("Failed to write systemd unit");

        let _ = Cmd::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
        let status = Cmd::new("systemctl")
            .args(["--user", "enable", "--now", "portmap"])
            .status();

        if status.is_ok_and(|s| s.success()) {
            println!("Installed and started on port {port}.");
            println!("Dashboard: http://localhost:{port}");
            println!("Logs:      journalctl --user -u portmap -f");
        } else {
            eprintln!("Failed to enable systemd service.");
        }
    }
}

fn cmd_uninstall(db_path: &str) {
    use std::process::Command as Cmd;

    if is_homebrew_install() {
        println!("portmap was installed via Homebrew.");
        println!("Use brew to uninstall:\n");
        println!("  brew services stop jonasks/tap/portmap");
        println!("  brew uninstall jonasks/tap/portmap");
        return;
    }

    if cfg!(target_os = "macos") {
        let plist = shellexpand("~/Library/LaunchAgents/dev.portmap.plist");
        let uid = get_uid();
        let target = format!("gui/{uid}");
        let _ = Cmd::new("launchctl")
            .args(["bootout", &target, &plist])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if std::fs::remove_file(&plist).is_ok() {
            println!("Removed launch agent.");
        }
    } else {
        let _ = Cmd::new("systemctl")
            .args(["--user", "disable", "--now", "portmap"])
            .status();
        let service = shellexpand("~/.config/systemd/user/portmap.service");
        if std::fs::remove_file(&service).is_ok() {
            println!("Removed systemd service.");
        }
    }

    if std::fs::remove_file(db_path).is_ok() {
        println!("Removed database.");
    }

    // Also remove old database location if it exists.
    let old_db = shellexpand("~/.portmap.db");
    if std::fs::remove_file(&old_db).is_ok() {
        println!("Removed old database.");
    }

    // Remove config directory if empty.
    let config_dir = config::config_dir();
    let _ = std::fs::remove_dir(&config_dir);

    println!("portmap has been uninstalled.");
}

async fn cmd_status(listen: u16) {
    use std::process::Command as Cmd;

    let alive = portmap::scanner::scan_ports(listen, listen, 0).await;
    let running = alive.contains(&listen);
    let status_dot = if running { "●" } else { "○" };
    let status_text = if running { "running" } else { "stopped" };

    // Detect service manager
    let service = if cfg!(target_os = "macos") {
        let uid = get_uid();
        let brew_ok = Cmd::new("launchctl")
            .args(["print", &format!("gui/{uid}/homebrew.mxcl.portmap")])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        let manual_ok = Cmd::new("launchctl")
            .args(["print", &format!("gui/{uid}/dev.portmap")])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if brew_ok {
            "homebrew"
        } else if manual_ok {
            "launchd"
        } else {
            "none"
        }
    } else {
        let ok = Cmd::new("systemctl")
            .args(["--user", "is-enabled", "portmap"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if ok { "systemd" } else { "none" }
    };

    let startup = if service == "none" { "no" } else { "yes" };

    println!("+------------+-------------------------------+");
    println!("| portmap    | {status_dot} {status_text:<27} |");
    println!("+------------+-------------------------------+");
    if running {
        let url = format!("http://localhost:{listen}");
        println!("| dashboard  | {url:<29} |");
    }
    println!("| service    | {service:<29} |");
    println!("| on startup | {startup:<29} |");
    println!("+------------+-------------------------------+");
}

fn get_uid() -> String {
    std::process::Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn shellexpand(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return format!("{home}/{rest}");
    }
    path.to_string()
}
