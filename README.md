<h1 align="center">
  ◎ portmap
</h1>

<p align="center">
  <em>Map names to localhost ports. Made for agents and humans.</em>
</p>

A lightweight alternative to [Vercel's Portless](https://github.com/vercel-labs/portless) — discover and manage what's running on your machine. Unlike Portless, portmap doesn't hijack your localhost with subdomain routing or break OAuth flows. It simply scans your ports, lets you name them, and gives you a clean dashboard, CLI and API.
Agents can use the CLI, or `curl -H "Accept: text/markdown" http://localhost:1337` to get all the information and instructions they need.

<p align="center">
  <img src="screenshot.png" alt="portmap dashboard" />
</p>

## Install

### Homebrew (macOS & Linux)

```bash
brew update 
brew install jonasks/tap/portmap
brew services start portmap         # start now + auto-start on login
# or: portmap serve                 # one-time foreground run
```
Bookmark the dashboard at [localhost:1337](http://localhost:1337), or use the `portmap` CLI:
 
```bash
❯ portmap list
PORT   NAME               CATEGORY  STATUS
1337   portmap            portmap   down
9900   my vite app        frontend  up
9951   website            frontend  up
9952   azure-mcp          mcp       up
9953   logfire-http-mcp   mcp       up
9954   vibber-mcp         mcp       up
9955   api                backend   up
9956   google-mcp         backend   down
9957   billing            backend   up
5000   AirPlay Receiver             up
7000   AirPlay Streaming            up
9958   -                            up
```

### From source

```bash
cargo install --path .
portmap install                      # start now + auto-start on login
# or: portmap serve                  # one-time foreground run
```


## CLI

```bash
portmap serve                          # run in foreground (default)
portmap install                        # start on login (launchd/systemd)
portmap uninstall                      # stop service + remove db
portmap status                         # check if running
portmap list                           # show all ports (registered + open)
portmap add --name "my-app" -P 3000 -c frontend
portmap add -P 8080 -c backend         # tag a port without naming it
portmap remove 3000                    # remove by port or name
portmap update 3000 --name "new-name"  # update by port or name
portmap kill 3000                      # kill process on port (by port or name)
portmap --version
```

> **Homebrew users:** use `brew services start/stop portmap` instead of `portmap install/uninstall`.

## Configuration

portmap reads `~/.config/portmap/config.toml` on startup. All fields are optional:

```toml
listen = 1337        # dashboard port
scan_start = 1000    # port scan range start (inclusive)
scan_end = 9999      # port scan range end (inclusive)
```

CLI flags override config file values. For example, to scan the full port range:

```toml
scan_start = 1000
scan_end = 65535
```

The database is stored at `~/.config/portmap/portmap.db`. If you're upgrading from an older version that used `~/.portmap.db`, portmap will automatically migrate it on first run.

## Features

- **Port scanning** — scans ports 1000–9999 by default (configurable via `config.toml`) on IPv4 and IPv6. Known ports are checked every 10s. Full discovery runs every 60s while the dashboard is open, or every 5 minutes when no one is watching.
- **Live dashboard** — real-time updates via SSE
- **Name, tag & kill ports** — right-click to edit, change colors, or kill processes
- **Agent-friendly** — `Accept: text/markdown` or `/markdown` for LLM-ready output
- **JSON API** — `/api/ports` for all port data
- **CLI** - `portmap`
- **SQLite persistence** — survives restarts, auto-migrates on upgrade
- **Tiny binary** — single static binary, no runtime dependencies
- **Startup service** — `portmap install` for launchd (macOS) or systemd (Linux)

## Claude Code skills

This repo is a [Claude Code plugin marketplace](https://docs.anthropic.com/en/docs/claude-code/skills) with two installable skills:

| Plugin | Description |
|--------|-------------|
| `portmap` | Teaches Claude to query and manage ports via the portmap API or CLI |
| `port-allocation` | Teaches Claude to pick an available port, document it, and register it when creating new services |

### Install as plugins

```
/plugin marketplace add JonasKs/portmap
/plugin install portmap@portmap
/plugin install port-allocation@portmap
```

Copy the skill files from [`skills/`](skills/) into your project's `.claude/skills/` directory and adapt to your conventions.

## License

MIT

## AI Use Disclaimer

This codebase has been built with a lot of support of AI. AI contributions welcome.
