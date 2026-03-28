use clap::Parser;
use portmap::AppState;
use tracing::info;

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
    let db = portmap::db::init(&db_path)
        .await
        .expect("Failed to initialize database");

    let state = AppState {
        db,
        dashboard_port: cli.port,
        scan_start: cli.scan_start,
        scan_end: cli.scan_end,
    };

    let app = portmap::create_router(state);

    let addr = format!("127.0.0.1:{}", cli.port);
    info!("portmap running at http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app).await.expect("Server error");
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

    let target = format!("gui/{uid}");
    let _ = Command::new("launchctl")
        .args(["bootout", &target, &plist])
        .status();

    if std::fs::remove_file(&plist).is_ok() {
        info!("Removed {plist}");
    }

    let db_path = shellexpand(db_flag);
    if std::fs::remove_file(&db_path).is_ok() {
        info!("Removed {db_path}");
    }

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
