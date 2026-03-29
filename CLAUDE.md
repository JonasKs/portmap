# portmap

Map names to localhost ports. Made for agents and humans.

## Build & Run

```bash
just dev          # run on port 1337
just build        # release build
just run          # build + run release
just check        # lint + format check
cargo test        # run tests
```

## Architecture

Single-binary Rust/Axum web server with embedded SQLite (sqlx).

- `src/lib.rs` — router, handlers, markdown renderer
- `src/main.rs` — CLI (clap), server startup, uninstall
- `src/db.rs` — SQLite queries (apps CRUD, tag colors)
- `src/scanner.rs` — async TCP port scanner
- `src/template.rs` — HTML dashboard template
- `migrations/` — SQLite migrations
- `tests/api_test.rs` — integration tests using in-memory SQLite

## Key Conventions

- Pedantic clippy lints enforced
- `unsafe` code forbidden
- All handlers go through `AppState` (contains db pool + config)
- Content negotiation: `Accept: text/markdown` serves agent-friendly markdown
- Tests use `create_router_with_test_db()` which creates an in-memory SQLite
