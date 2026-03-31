use portmap::container::ContainerPort;
use portmap::db::App;
use portmap::template::build_rows;

fn app(id: i64, name: &str, port: i64, category: &str) -> App {
    App {
        id,
        name: name.to_string(),
        port,
        category: category.to_string(),
        created_at: String::new(),
    }
}

fn container(port: u16, name: &str, source: &str) -> ContainerPort {
    ContainerPort {
        port,
        container_name: name.to_string(),
        source: source.to_string(),
    }
}

#[test]
fn test_merge_container_only() {
    let alive = vec![8080];
    let apps = vec![];
    let containers = vec![container(8080, "my-nginx", "docker")];

    let rows = build_rows(&alive, &apps, &containers);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].port, 8080);
    assert_eq!(rows[0].name, "my-nginx");
    assert_eq!(rows[0].category, ""); // no user category
    assert_eq!(rows[0].source, "docker"); // source in its own column
    assert!(rows[0].alive);
}

#[test]
fn test_merge_app_overrides_container() {
    let alive = vec![3000];
    let apps = vec![app(1, "my-app", 3000, "frontend")];
    let containers = vec![container(3000, "node-container", "docker")];

    let rows = build_rows(&alive, &apps, &containers);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "my-app");
    assert_eq!(rows[0].category, "frontend");
    assert_eq!(rows[0].source, "docker"); // source hint preserved
}

#[test]
fn test_merge_no_duplicates() {
    let alive = vec![8080];
    let apps = vec![app(1, "api", 8080, "backend")];
    let containers = vec![container(8080, "api-container", "docker")];

    let rows = build_rows(&alive, &apps, &containers);
    assert_eq!(rows.len(), 1); // one row, not two
}

#[test]
fn test_container_with_no_app_uses_container_name() {
    let alive = vec![9090];
    let apps = vec![];
    let containers = vec![container(9090, "redis-cache", "podman")];

    let rows = build_rows(&alive, &apps, &containers);
    assert_eq!(rows[0].name, "redis-cache");
    assert_eq!(rows[0].source, "podman"); // source in its own column
    assert_eq!(rows[0].category, ""); // no user category
}

#[test]
fn test_no_container_no_source() {
    let alive = vec![5000];
    let apps = vec![app(1, "my-app", 5000, "backend")];
    let containers = vec![];

    let rows = build_rows(&alive, &apps, &containers);
    assert_eq!(rows[0].source, ""); // no container = no source
}

#[test]
fn test_offline_app_no_container_source() {
    let alive = vec![];
    let apps = vec![app(1, "down-app", 4000, "backend")];
    let containers = vec![];

    let rows = build_rows(&alive, &apps, &containers);
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].alive);
    assert_eq!(rows[0].source, "");
}

#[test]
fn test_sort_order_alive_then_known_then_offline() {
    let alive = vec![3000, 5000]; // 5000 is AirPlay (known)
    let apps = vec![
        app(1, "my-app", 3000, "frontend"),
        app(2, "down-app", 4000, "backend"),
    ];
    let containers = vec![];

    let rows = build_rows(&alive, &apps, &containers);

    // alive registered first, then known (AirPlay), then offline
    assert_eq!(rows[0].port, 3000);
    assert!(rows[0].alive);
    assert_eq!(rows[1].port, 5000); // AirPlay - known service
    assert!(rows[1].alive);
    assert_eq!(rows[2].port, 4000); // offline
    assert!(!rows[2].alive);
}

#[test]
fn test_registered_app_with_container_shows_source_hint() {
    // When a port is both registered AND found via Docker,
    // source should be "docker" (hint that it's a container)
    let alive = vec![3000];
    let apps = vec![app(1, "my-app", 3000, "backend")];
    let containers = vec![container(3000, "node-container", "docker")];

    let rows = build_rows(&alive, &apps, &containers);
    assert_eq!(rows[0].name, "my-app"); // app wins
    assert_eq!(rows[0].category, "backend"); // app wins
    assert_eq!(rows[0].source, "docker"); // source hint preserved
}

#[test]
fn test_container_only_no_category() {
    // Unregistered container port: empty category, source in its own field
    let alive = vec![8080];
    let apps = vec![];
    let containers = vec![container(8080, "nginx", "docker")];

    let rows = build_rows(&alive, &apps, &containers);
    assert_eq!(rows[0].category, ""); // no user category
    assert_eq!(rows[0].source, "docker"); // source column
}

#[test]
fn test_multiple_containers_different_ports() {
    let alive = vec![8080, 8081, 9000];
    let apps = vec![];
    let containers = vec![
        container(8080, "web", "docker"),
        container(8081, "api", "docker"),
    ];

    let rows = build_rows(&alive, &apps, &containers);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].name, "web");
    assert_eq!(rows[1].name, "api");
    assert_eq!(rows[2].name, ""); // port 9000 - unnamed
}
