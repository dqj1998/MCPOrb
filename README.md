# MCPOrb

A runtime-only repository for packaged MCP Orbs.

This repository owns the runtime that powers a standalone Orb executable:
- Exposes a **MCP Server** (stdio JSON-RPC) for AI clients like Claude Desktop, Cursor, VS Code
- Serves a **local Web UI** at `http://127.0.0.1:<port>/<token>/` for human inspection
- Loads Orb assets produced elsewhere and serves BM25-based retrieval at runtime

Builder-side code now lives in the sibling `../MCPOrbBuilder` repository.
Shared plans, fixtures, and reports live in `../MCPOrbEtc`.

## Quick Start

```bash
# Build the runtime
cargo build -p mcporb-runtime

# Run the runtime directly against an Orb assets directory
cargo run -p mcporb-runtime -- --assets target/orbs/mda-guide --gui-only --open
```

To build, inspect, or package Orbs, use the sibling `MCPOrbBuilder` repository.

## Packaged Orb Release

The packaged `.orb` file is still the preferred distributable artifact. It is produced by `MCPOrbBuilder`, but executed by the runtime in this repository.

### Run the packaged Orb

```bash
# Open the local Web UI
./target/orbs/mda-guide.orb --gui-only --open

# Expose MCP over stdio only
./target/orbs/mda-guide.orb --stdio-only

# Run both MCP stdio and Web UI
./target/orbs/mda-guide.orb --stdio-gui
```

### MCP client configuration for the packaged Orb

When you distribute the single-file Orb, point your MCP client at the packaged executable and do not pass `--assets`.

## MCP Client Configuration

Development setup with an external assets directory:

```json
{
  "mcpServers": {
    "mda-guide": {
      "command": "/Users/qingjie.du/HDD/MCPOrb/target/debug/mcporb-runtime",
      "args": ["--assets", "/Users/qingjie.du/HDD/MCPOrb/target/orbs/mda-guide", "--stdio-gui"]
    }
  }
}
```

> **Note:** Build the runtime first with `cargo build -p mcporb-runtime`. For production use, replace `debug` with `release` and build with `cargo build --release -p mcporb-runtime`.

Single-file packaged Orb setup:

```json
{
  "mcpServers": {
    "mda-guide": {
      "command": "/Users/qingjie.du/HDD/MCPOrb/target/orbs/mda-guide.orb",
      "args": ["--stdio-gui"]
    }
  }
}
```

For production distribution, prefer the packaged `.orb` file over `target/debug/mcporb-runtime` plus a separate `target/orbs/<name>/` directory.

When an Orb runs with `--stdio-gui`, MCP clients should call the `get_web_ui_url` tool to discover the local Web UI address. The URL is not exposed as an MCP resource.

## Architecture

```
MCPOrb/
├── crates/
│   ├── mcporb-runtime/        # Orb runtime: MCP stdio + axum Web UI
│   ├── mcporb-runtime-core/   # Runtime-only data contracts and BM25 query logic
│   └── mcporb-size-spike/     # Runtime binary size spike
├── public-orb/                # Published showcase Orb artifacts and collateral
└── scripts/
```

## Startup Modes

| Mode | Command | Behavior |
|------|---------|----------|
| Auto (TTY) | `./orb` | Opens Web UI in browser |
| Auto (piped) | `./orb` | MCP stdio + silent Web UI |
| GUI only | `./orb --gui-only --open` | Web UI only, opens browser |
| Stdio only | `./orb --stdio-only` | MCP stdio, no HTTP server |
| Both | `./orb --stdio-gui` | MCP stdio + Web UI |

## Binary Size Budget

| Version | Budget |
|---------|--------|
| v0.1 (MVP) | ≤ 15 MB |
| v0.2 | ≤ 20 MB |

Check: `bash scripts/check-binary-size.sh`

## Development

```bash
cargo check --workspace
cargo test --workspace
cargo build -p mcporb-runtime --release
```

## Public Orb

Selected showcase Orbs should be published under `public-orb/`.
This directory is intentionally kept in the runtime repository so public Orb releases can ship alongside the runtime brand.

