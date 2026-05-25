# public-orb — Curated Public Demo Orbs

**MCPOrb: The PDF for AI-native knowledge delivery.**

This directory contains public demo Orbs ready to use with any MCP-compatible AI client.
Each `.orb` file is a self-contained, locally-runnable knowledge asset — no build step required.

## Available Orbs

| Folder | Theme | Orb file | Status |
|---|---|---|---|
| `MDA/` | Model Driven Architecture reference guide | `MDA_ModelDrivenArchitecture_Guide_v1.0.1.orb` | available |
| `AI-Governance/` | AI governance, policy, and compliance corpus | `AI_Governance_Control_Compendium.orb` | available |
| `Industrial-Safety-Ops/` | Industrial safety, maintenance, and training corpus | — | planned |
| `Public-Procurement-Delivery/` | Public procurement and project delivery corpus | — | planned |

## How to use an Orb

An `.orb` file is a single self-contained executable.

### With an MCP client (stdio mode)

Point your MCP client at the Orb directly:

```json
{
  "mcpServers": {
    "orb-name": {
      "command": "/path/to/YourOrb.orb",
      "args": ["--stdio-only"]
    }
  }
}
```

### With the built-in Web UI

Run the Orb directly to open an interactive query interface in your browser:

```bash
/path/to/YourOrb.orb --open
```

- `--open` launches the browser automatically at `http://127.0.0.1:<port>/<token>/`.
- Omit `--open` to start the server only; the URL is printed to stdout.
- You can also ask the LLM to call the `get_web_ui_url` tool — it returns the URL, which you can then open in your browser.

See each demo folder's README for client-specific configuration and example queries.
