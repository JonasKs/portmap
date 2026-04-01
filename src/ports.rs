use crate::container::ContainerPort;
use crate::db::App;

/// Merge TCP scan results with container ports into a single alive list.
/// Container ports outside the scan range are added automatically.
pub fn merge_alive(alive: &mut Vec<u16>, container_ports: &[ContainerPort], exclude_port: u16) {
    for cp in container_ports {
        if !alive.contains(&cp.port) && cp.port != exclude_port {
            alive.push(cp.port);
        }
    }
    alive.sort_unstable();
}

/// A unified port entry used by both CLI and API.
#[derive(Clone)]
pub struct PortEntry {
    pub port: u16,
    pub name: String,
    pub category: String,
    pub source: String,
    pub registered: bool,
    pub alive: bool,
}

/// Build a merged list of port entries from apps, alive ports, and container data.
/// This is the single source of truth for how ports are presented.
pub fn build_port_entries(
    alive: &[u16],
    apps: &[App],
    container_ports: &[ContainerPort],
) -> Vec<PortEntry> {
    let container_map: std::collections::HashMap<u16, &ContainerPort> =
        container_ports.iter().map(|cp| (cp.port, cp)).collect();
    let mut entries = Vec::new();
    let mut seen_ports = std::collections::HashSet::new();

    // Alive ports first
    for &port in alive {
        seen_ports.insert(port);
        let app = apps.iter().find(|a| a.port == i64::from(port));
        let cp = container_map.get(&port);
        let source = cp.map_or(String::new(), |c| c.source.clone());

        if let Some(a) = app {
            entries.push(PortEntry {
                port,
                name: if a.name.is_empty() {
                    String::new()
                } else {
                    a.name.clone()
                },
                category: a.category.clone(),
                source,
                registered: true,
                alive: true,
            });
        } else if let Some(c) = cp {
            entries.push(PortEntry {
                port,
                name: c.container_name.clone(),
                category: String::new(),
                source: c.source.clone(),
                registered: false,
                alive: true,
            });
        } else if let Some(k) = crate::known_ports::lookup(port) {
            entries.push(PortEntry {
                port,
                name: k.name.to_string(),
                category: "macos".to_string(),
                source: String::new(),
                registered: false,
                alive: true,
            });
        } else {
            entries.push(PortEntry {
                port,
                name: String::new(),
                category: String::new(),
                source: String::new(),
                registered: false,
                alive: true,
            });
        }
    }

    // Offline registered apps
    for app in apps {
        let port = u16::try_from(app.port).unwrap_or(0);
        if seen_ports.contains(&port) {
            continue;
        }
        entries.push(PortEntry {
            port,
            name: if app.name.is_empty() {
                String::new()
            } else {
                app.name.clone()
            },
            category: app.category.clone(),
            source: String::new(),
            registered: true,
            alive: false,
        });
    }

    entries
}
