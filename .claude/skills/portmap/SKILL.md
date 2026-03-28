---
name: portmap
description: Check what's running on localhost, manage port names and tags via the portmap API at localhost:1337
allowed-tools: Bash(curl *)
---

# portmap — localhost port manager

portmap runs at http://localhost:1337 and tracks what services are running on your machine.

## View what's running

```bash
# All open ports with registered app info
curl -s http://localhost:1337/api/ports

# Just registered apps
curl -s http://localhost:1337/api/apps

# Human-readable markdown summary
curl -s http://localhost:1337/markdown
```

## Register a new service

```bash
curl -X POST http://localhost:1337/api/apps \
  -H "Content-Type: application/json" \
  -d '{"name": "SERVICE_NAME", "port": PORT, "category": "CATEGORY"}'
```

Categories are freeform strings. Common ones: `frontend`, `backend`, `mcp`.

## Bulk register

```bash
curl -X POST http://localhost:1337/api/apps/bulk \
  -H "Content-Type: application/json" \
  -d '[
    {"name": "web", "port": 3000, "category": "frontend"},
    {"name": "api", "port": 8080, "category": "backend"}
  ]'
```

## Update a service

```bash
curl -X PUT http://localhost:1337/api/apps/ID \
  -H "Content-Type: application/json" \
  -d '{"name": "new-name", "category": "new-category"}'
```

## Delete a service

```bash
curl -X DELETE http://localhost:1337/api/apps/ID
```
