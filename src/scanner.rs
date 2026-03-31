use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio::time::timeout;

const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
const MAX_CONCURRENT: usize = 20;

/// Scan a range of ports on localhost and return which ones are open.
/// Tries IPv4 first, falls back to IPv6 (`::1`) only if IPv4 fails.
/// Skips `exclude_port` (the dashboard's own port).
/// Uses a bounded `JoinSet` to limit both concurrency and task count.
pub async fn scan_ports(start: u16, end: u16, exclude_port: u16) -> Vec<u16> {
    let mut ports = (start..=end).filter(|p| *p != exclude_port);
    let mut set = JoinSet::new();
    let mut alive = Vec::new();

    // Fill the initial window
    for port in ports.by_ref().take(MAX_CONCURRENT) {
        set.spawn(probe(port));
    }

    // As each task completes, spawn the next port
    while let Some(result) = set.join_next().await {
        if let Ok(Some(port)) = result {
            alive.push(port);
        }
        if let Some(port) = ports.next() {
            set.spawn(probe(port));
        }
    }

    alive.sort_unstable();
    alive
}

/// Probe a specific list of ports (no range scan). Used for quick status checks.
/// Uses bounded `JoinSet` like `scan_ports`.
pub async fn probe_ports(ports: &[u16], exclude_port: u16) -> Vec<u16> {
    let mut iter = ports.iter().filter(|&&p| p != exclude_port).copied();
    let mut set = JoinSet::new();
    let mut alive = Vec::new();

    for port in iter.by_ref().take(MAX_CONCURRENT) {
        set.spawn(probe(port));
    }

    while let Some(result) = set.join_next().await {
        if let Ok(Some(port)) = result {
            alive.push(port);
        }
        if let Some(port) = iter.next() {
            set.spawn(probe(port));
        }
    }
    alive.sort_unstable();
    alive
}

async fn probe(port: u16) -> Option<u16> {
    let v4 = SocketAddr::from(([127, 0, 0, 1], port));
    if matches!(
        timeout(CONNECT_TIMEOUT, TcpStream::connect(v4)).await,
        Ok(Ok(_))
    ) {
        return Some(port);
    }
    let v6 = SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], port));
    if matches!(
        timeout(CONNECT_TIMEOUT, TcpStream::connect(v6)).await,
        Ok(Ok(_))
    ) {
        return Some(port);
    }
    None
}
