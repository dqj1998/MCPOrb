# What's New in MCPOrb Runner 1.2.0

## Run MCP Servers Over HTTP

You can now run MCP servers over HTTP in addition to the existing STDIO mode. This means you can connect to MCP servers remotely — not just on the same machine.

Open the new **HTTP** tab in the app to configure host, port, and transport settings. STDIO mode is still fully supported with the same experience you're used to.

## Orb Import & Delete — Smoother Workflow

After importing a new Orb or deleting an existing one, a banner now appears at the top of the app reminding you to restart your connected MCP clients. No more wondering whether the changes took effect — you'll know right away.

## Stronger Store Foundation

The Store integration now uses a dedicated public API (`mcporb.store`). This means more reliable Orb downloads and a solid foundation for future Store features — like browsing and installing Orbs directly from within the app.

## Under the Hood

- MCP gateway processes are now labeled clearly in logs, making it easier to identify which process is which when running multiple gateways.
- The `--mcp-transport` flag is available on the runtime binary for accurate metrics tracking when using HTTP mode.

---

*This release focuses on expanding how you run MCP servers. HTTP mode opens up remote connection possibilities, while the improved import UX keeps your workflow moving.*
