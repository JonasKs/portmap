use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::time::timeout;

const CONNECT_TIMEOUT: Duration = Duration::from_millis(80);
const BATCH_SIZE: usize = 200;

/// Scan a range of ports on localhost and return which ones are open.
/// Skips `exclude_port` (the dashboard's own port).
pub async fn scan_ports(start: u16, end: u16, exclude_port: u16) -> Vec<u16> {
    let ports: Vec<u16> = (start..=end).filter(|p| *p != exclude_port).collect();
    let mut alive = Vec::new();

    for chunk in ports.chunks(BATCH_SIZE) {
        let mut handles = Vec::with_capacity(chunk.len());

        for &port in chunk {
            handles.push(tokio::spawn(async move {
                let addr = SocketAddr::from(([127, 0, 0, 1], port));
                match timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await {
                    Ok(Ok(_)) => Some(port),
                    _ => None,
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
