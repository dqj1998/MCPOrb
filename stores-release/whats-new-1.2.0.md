# What's New in MCPOrb Runner 1.2.0

## Run MCP Servers Over HTTP

You can now run MCP servers over HTTP in addition to the existing STDIO mode. This means you can connect to MCP servers remotely — not just on the same machine.

Open the new **HTTP** tab in the app to configure host, port, and transport settings. STDIO mode is still fully supported with the same experience you're used to.

## Orb Import & Delete — Smoother Workflow

After importing a new Orb or deleting an existing one, a banner now appears at the top of the app reminding you to restart your connected MCP clients. No more wondering whether the changes took effect — you'll know right away.

## Store Tab — Browse, Search & Install Orbs

The **Store** tab is now open and ready to use. Browse the MCP Orb marketplace directly from the app:

- **Search** — Find Orbs by name or description with full-text search
- **Filter** — Narrow results by tag or MCP method (bm25, tfidf, trigram, vector, knowledge_graph, hybrid)
- **Orb Details** — View description, version history, available artifacts, supported methods, tags, and password status
- **Password-Protected Downloads** — For private Orbs, a password dialog appears before download; the app verifies credentials via the Store API before transferring
- **One-Click Import** — Download and import an Orb directly from the Store tab — no manual ZIP handling needed

Behind the scenes, the Store integration was rebuilt on a dedicated public API (`mcporb.store`), providing a reliable foundation for future marketplace features.

## Under the Hood

- MCP gateway processes are now labeled clearly in logs, making it easier to identify which process is which when running multiple gateways.
- The `--mcp-transport` flag is available on the runtime binary for accurate metrics tracking when using HTTP mode.

---

*This release focuses on expanding how you run MCP servers. HTTP mode opens up remote connection possibilities, while the improved import UX keeps your workflow moving.*
