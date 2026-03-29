# portmap

> Map names to localhost ports. Made for agents and humans.

A lightweight alternative to [Vercel's Portless](https://github.com/vercel-labs/portless) — discover and manage what's running on your machine. Unlike Portless, portmap doesn't hijack your localhost with subdomain routing or break OAuth flows. It simply scans your ports, lets you name them, and gives you a clean dashboard + API. Agents can use the CLI, or `curl -H "Accept: text/markdown" http://localhost:1337` to get all the information and instructions they need.

![portmap dashboard](screenshot.png)

## Install

### Homebrew (macOS & Linux)

```bash
brew install jonasks/tap/portmap
```

### From source

```bash
cargo install --path .
```

## Quick start

### Homebrew

```bash
brew services start jonasks/tap/portmap
```

### From source

```bash
portmap install        # register as startup service + start now
```

Dashboard at [localhost:1337](http://localhost:1337). That's it.

## CLI

```bash
portmap serve                          # run in foreground (default)
portmap install                        # start on login (launchd/systemd)
portmap uninstall                      # stop service + remove db
portmap status                         # check if running
portmap list                           # show registered apps
portmap scan                           # discover open ports
portmap add --name "my-app" -P 3000 -c frontend
portmap add -P 8080 -c backend         # tag a port without naming it
portmap remove 3000                    # remove by port or ID
portmap update 1 --name "new-name"
portmap --version
```

> **Homebrew users:** use `brew services start/stop portmap` instead of `portmap install/uninstall`.

## Features

- **Port scanning** — discovers all active localhost services
- **Name & tag ports** — click to navigate, right-click (or pencil icon) to edit inline
- **Optional names** — tag a port with just a category, name is not required
- **Category badges** — tag services as frontend, backend, mcp, or anything
- **Custom tag colors** — right-click filter buttons to pick a color per category
- **Filter by tag** — quickly filter the dashboard
- **Agent-friendly** — `Accept: text/markdown` or `/markdown` returns clean markdown with full API docs
- **JSON API** — CRUD for registered apps at `/api/apps`, tag colors at `/api/tag-colors`
- **SQLite persistence** — survives restarts, stored at `~/.portmap.db`
- **Auto-migration** — DB schema upgrades automatically on new versions
- **Tiny binary** — single static binary, no runtime dependencies
- **Startup service** — `portmap install` registers launchd (macOS) or systemd (Linux)

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
