use bollard::Docker;
use bollard::query_parameters::ListContainersOptions;

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

    // bollard auto-detects the socket (Docker Desktop, rootless, etc.)
    if let Ok(docker) = Docker::connect_with_local_defaults()
        && let Ok(containers) = docker
            .list_containers(Some(ListContainersOptions {
                all: false, // only running containers
                ..Default::default()
            }))
            .await
    {
        let source = detect_source(&docker).await;
        for container in &containers {
            let name = container
                .names
                .as_ref()
                .and_then(|n| n.first())
                .map(|n| n.trim_start_matches('/').to_string())
                .unwrap_or_default();

            if let Some(port_bindings) = &container.ports {
                for pm in port_bindings {
                    let Some(public_port) = pm.public_port else {
                        continue;
                    };
                    let ip = pm.ip.as_deref().unwrap_or("0.0.0.0");
                    if ip != "0.0.0.0" && ip != "127.0.0.1" && ip != "::1" && ip != "::" {
                        continue;
                    }
                    ports.push(ContainerPort {
                        port: public_port,
                        container_name: name.clone(),
                        source: source.clone(),
                    });
                }
            }
        }
    }

    // Deduplicate
    ports.sort_by_key(|p| p.port);
    ports.dedup_by_key(|p| p.port);
    ports
}

/// Detect whether we're talking to Docker or Podman.
async fn detect_source(docker: &Docker) -> String {
    if let Ok(version) = docker.version().await
        && let Some(components) = &version.components
    {
        for c in components {
            let name_lower = c.name.to_lowercase();
            if name_lower.contains("podman") {
                return "podman".to_string();
            }
        }
    }
    "docker".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_discover_graceful_when_no_socket() {
        // Should return empty vec, not panic, even without Docker
        let ports = discover().await;
        let _ = ports; // may or may not find Docker
    }

    #[test]
    fn test_container_port_struct() {
        let cp = ContainerPort {
            port: 8080,
            container_name: "my-nginx".to_string(),
            source: "docker".to_string(),
        };
        assert_eq!(cp.port, 8080);
        assert_eq!(cp.container_name, "my-nginx");
        assert_eq!(cp.source, "docker");
    }
}
