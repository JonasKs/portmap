use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::time::timeout;

const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
const BATCH_SIZE: usize = 100;

/// Scan a range of ports on localhost and return which ones are open.
/// Tries both IPv4 (127.0.0.1) and IPv6 (`::1`) since dev servers may
/// bind to either.
/// Skips `exclude_port` (the dashboard's own port).
pub async fn scan_ports(start: u16, end: u16, exclude_port: u16) -> Vec<u16> {
    let ports: Vec<u16> = (start..=end).filter(|p| *p != exclude_port).collect();
    let mut alive = Vec::new();

    for chunk in ports.chunks(BATCH_SIZE) {
        let mut handles = Vec::with_capacity(chunk.len());

        for &port in chunk {
            handles.push(tokio::spawn(async move {
                let v4 = SocketAddr::from(([127, 0, 0, 1], port));
                let v6 = SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], port));
                let (r4, r6) = tokio::join!(
                    timeout(CONNECT_TIMEOUT, TcpStream::connect(v4)),
                    timeout(CONNECT_TIMEOUT, TcpStream::connect(v6)),
                );
                if matches!(r4, Ok(Ok(_))) || matches!(r6, Ok(Ok(_))) {
                    Some(port)
                } else {
                    None
                }
            }));
        }

        for handle in handles {
            if let Ok(Some(port)) = handle.await {
                alive.push(port);
            }
        }
    }

    alive.sort_unstable();
    alive
}
