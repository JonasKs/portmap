# portmap

> Map names to localhost ports. Made for agents and humans.

A lightweight alternative to [Vercel's Portless](https://github.com/vercel-labs/portless) — discover and manage what's running on your machine. Unlike Portless, portmap doesn't hijack your localhost with subdomain routing or break OAuth flows. It simply scans your ports, lets you name them, and gives you a clean dashboard + API.

![portmap dashboard](screenshot.png)

## Install

### Homebrew (macOS & Linux)

```bash
brew tap jonasKs/portmap
brew install portmap
```

### From source

```bash
cargo install --path .
```

## Usage

```bash
portmap                    # start on port 1337
portmap --port 8080        # custom port
portmap --scan-start 3000 --scan-end 9000  # custom scan range
```

Then visit [localhost:1337](http://localhost:1337).

## Features

- **Port scanning** — discovers all active localhost services
- **Name & tag ports** — click to navigate, right-click (or pencil icon) to edit
- **Category badges** — tag services as frontend, backend, mcp, or anything
- **Filter by tag** — quickly filter the dashboard
- **Agent-friendly** — `Accept: text/markdown` or `/markdown` returns clean markdown with full API docs
- **JSON API** — CRUD for registered apps at `/api/apps`
- **SQLite persistence** — survives restarts, stored at `~/.portmap.db`
- **Tiny binary** — single static binary, no runtime dependencies

## API

```bash
# List all open ports with app info
curl localhost:1337/api/ports

# List registered apps
curl localhost:1337/api/apps

# Register an app
curl -X POST localhost:1337/api/apps \
  -H "Content-Type: application/json" \
  -d '{"name": "my-app", "port": 3000, "category": "frontend"}'

# Bulk register
curl -X POST localhost:1337/api/apps/bulk \
  -H "Content-Type: application/json" \
  -d '[{"name": "api", "port": 8080, "category": "backend"}]'

# Update
curl -X PUT localhost:1337/api/apps/1 \
  -H "Content-Type: application/json" \
  -d '{"name": "new-name"}'

# Delete
curl -X DELETE localhost:1337/api/apps/1

# Markdown (for agents)
curl -H "Accept: text/markdown" localhost:1337/
```

## Run on startup

### macOS (launchd)

```bash
just install-service    # install binary + register launch agent
just uninstall-service  # stop + remove
just status             # check if running
just logs               # tail logs
```

### Linux (systemd)

```bash
# Create a user service
mkdir -p ~/.config/systemd/user
cat > ~/.config/systemd/user/portmap.service << EOF
[Unit]
Description=portmap

[Service]
ExecStart=%h/.cargo/bin/portmap --port 1337
Restart=always

[Install]
WantedBy=default.target
EOF

systemctl --user enable --now portmap
```

### Full uninstall

```bash
portmap --uninstall   # removes agent, database, and binary
```

## License

MIT
