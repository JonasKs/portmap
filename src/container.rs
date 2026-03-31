use serde::Deserialize;

/// A port exposed by a running container.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainerPort {
    pub port: u16,
    pub container_name: String,
    pub source: String, // "docker" or "podman"
}

/// Discover ports exposed by Docker and Podman containers.
/// Returns an empty vec if no runtime is available.
pub async fn discover() -> Vec<ContainerPort> {
    let mut ports = Vec::new();

    #[cfg(unix)]
    {
        // Try Docker first, then Podman sockets
        let docker_sock = "/var/run/docker.sock";
        let podman_socks = podman_socket_paths();

        if tokio::fs::metadata(docker_sock).await.is_ok() {
            ports.extend(query_socket(docker_sock, "docker").await);
        }

        for sock in &podman_socks {
            if tokio::fs::metadata(sock).await.is_ok() {
                ports.extend(query_socket(sock, "podman").await);
                break; // Use first available Podman socket
            }
        }
    }

    // Deduplicate by port (first source wins)
    ports.sort_by_key(|p| p.port);
    ports.dedup_by_key(|p| p.port);
    ports
}

#[cfg(unix)]
fn podman_socket_paths() -> Vec<String> {
    let mut paths = Vec::new();
    // Rootless Podman
    if let Ok(uid) = std::env::var("UID").or_else(|_| {
        std::process::Command::new("id")
            .arg("-u")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .ok_or(std::env::VarError::NotPresent)
    }) {
        paths.push(format!("/run/user/{uid}/podman/podman.sock"));
    }
    // Rootful Podman
    paths.push("/var/run/podman/podman.sock".to_string());
    paths
}

#[cfg(unix)]
async fn query_socket(path: &str, source: &str) -> Vec<ContainerPort> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    let result = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        let mut stream = UnixStream::connect(path).await?;
        // Use HTTP/1.0 — Docker Desktop on macOS returns 500 for HTTP/1.1
        stream
            .write_all(b"GET /containers/json HTTP/1.0\r\nHost: localhost\r\n\r\n")
            .await?;

        let mut buf = Vec::with_capacity(64 * 1024);
        stream.read_to_end(&mut buf).await?;
        Ok::<_, std::io::Error>(buf)
    })
    .await;

    let Ok(Ok(buf)) = result else {
        return Vec::new();
    };

    let body = extract_http_body(&buf);
    parse_containers(body, source)
}

/// Extract the body from a raw HTTP response, checking for 200 status.
fn extract_http_body(raw: &[u8]) -> &[u8] {
    // Check status line for 200 OK
    let status_end = raw.windows(2).position(|w| w == b"\r\n").unwrap_or(0);
    let status_line = std::str::from_utf8(&raw[..status_end]).unwrap_or("");
    if !status_line.contains(" 200") {
        return &[];
    }
    let separator = b"\r\n\r\n";
    raw.windows(separator.len())
        .position(|w| w == separator)
        .map_or(&[], |pos| &raw[pos + separator.len()..])
}

/// Parse the Docker/Podman container JSON and extract exposed localhost ports.
pub fn parse_containers(body: &[u8], source: &str) -> Vec<ContainerPort> {
    let containers: Vec<ApiContainer> = match serde_json::from_slice(body) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut ports = Vec::new();
    for container in &containers {
        let name = container
            .names
            .first()
            .map(|n| n.trim_start_matches('/').to_string())
            .unwrap_or_default();

        for port_mapping in &container.ports {
            let Some(public_port) = port_mapping.public_port else {
                continue;
            };
            // Only include ports bound to localhost or all interfaces
            let ip = port_mapping.ip.as_deref().unwrap_or("0.0.0.0");
            if ip != "0.0.0.0" && ip != "127.0.0.1" && ip != "::1" && ip != "::" {
                continue;
            }
            ports.push(ContainerPort {
                port: public_port,
                container_name: name.clone(),
                source: source.to_string(),
            });
        }
    }
    ports
}

#[derive(Deserialize)]
struct ApiContainer {
    #[serde(default, rename = "Names")]
    names: Vec<String>,
    #[serde(default, rename = "Ports")]
    ports: Vec<ApiPort>,
}

#[derive(Deserialize)]
struct ApiPort {
    #[serde(rename = "PublicPort")]
    public_port: Option<u16>,
    #[serde(rename = "IP")]
    ip: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_docker_json() {
        let json = br#"[
            {
                "Names": ["/my-nginx"],
                "Ports": [
                    {"IP": "0.0.0.0", "PublicPort": 8080, "PrivatePort": 80, "Type": "tcp"},
                    {"IP": "0.0.0.0", "PublicPort": 8443, "PrivatePort": 443, "Type": "tcp"}
                ]
            },
            {
                "Names": ["/redis-cache"],
                "Ports": [
                    {"IP": "127.0.0.1", "PublicPort": 6379, "PrivatePort": 6379, "Type": "tcp"}
                ]
            }
        ]"#;

        let ports = parse_containers(json, "docker");
        assert_eq!(ports.len(), 3);
        assert_eq!(ports[0].port, 8080);
        assert_eq!(ports[0].container_name, "my-nginx");
        assert_eq!(ports[0].source, "docker");
        assert_eq!(ports[1].port, 8443);
        assert_eq!(ports[2].port, 6379);
        assert_eq!(ports[2].container_name, "redis-cache");
    }

    #[test]
    fn test_parse_empty_ports() {
        let json = br#"[{"Names": ["/no-ports"], "Ports": []}]"#;
        let ports = parse_containers(json, "docker");
        assert!(ports.is_empty());
    }

    #[test]
    fn test_parse_no_public_port() {
        let json = br#"[{
            "Names": ["/internal"],
            "Ports": [{"PrivatePort": 80, "Type": "tcp"}]
        }]"#;
        let ports = parse_containers(json, "docker");
        assert!(ports.is_empty());
    }

    #[test]
    fn test_parse_non_localhost_ports_skipped() {
        let json = br#"[{
            "Names": ["/remote-bind"],
            "Ports": [
                {"IP": "172.17.0.1", "PublicPort": 9090, "PrivatePort": 80, "Type": "tcp"},
                {"IP": "0.0.0.0", "PublicPort": 8080, "PrivatePort": 80, "Type": "tcp"}
            ]
        }]"#;
        let ports = parse_containers(json, "docker");
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 8080);
    }

    #[test]
    fn test_parse_invalid_json() {
        let ports = parse_containers(b"not json", "docker");
        assert!(ports.is_empty());
    }

    #[test]
    fn test_parse_empty_response() {
        let ports = parse_containers(b"[]", "docker");
        assert!(ports.is_empty());
    }

    #[test]
    fn test_parse_podman_source() {
        let json = br#"[{
            "Names": ["my-pod"],
            "Ports": [{"IP": "0.0.0.0", "PublicPort": 3000, "PrivatePort": 3000, "Type": "tcp"}]
        }]"#;
        let ports = parse_containers(json, "podman");
        assert_eq!(ports[0].source, "podman");
    }

    #[test]
    fn test_extract_http_body() {
        let raw =
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n[{\"Names\":[\"/test\"]}]";
        let body = extract_http_body(raw);
        assert_eq!(body, b"[{\"Names\":[\"/test\"]}]");
    }

    #[test]
    fn test_extract_http_body_no_separator() {
        let body = extract_http_body(b"no separator here");
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn test_discover_no_socket() {
        // On most CI/dev machines without Docker, this returns empty
        let ports = discover().await;
        // Just verify it doesn't panic -- may or may not find Docker
        let _ = ports;
    }
}
