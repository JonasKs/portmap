---
name: portmap
description: Check what's running on localhost, manage port names and tags via the portmap API at localhost:1337
allowed-tools: Bash(curl *)
---

portmap runs at http://localhost:1337. Use it to see what's running and manage services.

Start by fetching the markdown endpoint which has everything you need:

curl -s http://localhost:1337/markdown

That returns a full overview of registered apps, their status, and the complete API reference with examples.

If portmap isn't running, start it with: portmap --port 1337 &
