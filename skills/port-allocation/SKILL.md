---
description: Allocate a local dev port for a new service and register it in portmap. Use this whenever creating a new frontend or backend service.
allowed-tools: Bash(portmap *), Bash(curl *)
---

# Port Allocation

When adding a new service to the project, follow these steps to allocate a port and register it.

## 1. Pick an available port

Check which ports are already in use across projects on this machine:

```bash
portmap list
```

Also check any port allocation docs in your project (e.g. `CLAUDE.md`) for ports reserved in this repo.

Pick a port that is not already claimed in either place.

## 2. Document the port

Add the new service and its port to your project's documentation so other developers and agents know it's taken.

## 3. Register in portmap (if available)

```bash
portmap add --name "<service-name>" -P <port> -c <category>
```

Where `<category>` describes the service type (e.g. `frontend`, `backend`, `mcp`, `database`). The `--name` flag is optional — you can tag a port with just a category.

If portmap is not running, skip silently.

### Updating an existing registration

```bash
portmap remove <old-port>
portmap add --name "<new-name>" -P <new-port> -c <category>
```
