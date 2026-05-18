# MCPOrb

A single-binary, self-contained MCP capability package ecosystem.

Turn a PDF or Markdown knowledge base into a standalone executable Orb that:
- Exposes a **MCP Server** (stdio JSON-RPC) for AI clients like Claude Desktop, Cursor, VS Code
- Serves a **local Web UI** at `http://127.0.0.1:<port>/<token>/` for human inspection
- Provides **BM25 full-text search** over the embedded knowledge base

## Quick Start

```bash
# Build an Orb from a PDF
cargo run -p mcporb-cli -- build mda-orb/00-2_MDA_Guide_v1.0.1.pdf --name mda-guide

# Inspect the built Orb
cargo run -p mcporb-cli -- inspect target/orbs/mda-guide

# Test BM25 search directly
cargo run -p mcporb-cli -- test-query target/orbs/mda-guide "model driven architecture"

# Launch the Orb with Web UI (opens browser)
cargo build -p mcporb-runtime
cargo run -p mcporb-cli -- run target/orbs/mda-guide --open

# Or run the runtime directly
cargo run -p mcporb-runtime -- --assets target/orbs/mda-guide --gui-only --open
```

## Single-File Orb Release

If you want a single distributable file that contains both the runtime and the Orb data, use the `package` command.

### Build the single-file Orb

```bash
# 1) Build the Orb assets directory
cargo run -p mcporb-cli -- build mda-orb/00-2_MDA_Guide_v1.0.1.pdf --name mda-guide

# 2) Package runtime + assets into one executable
cargo run -p mcporb-cli -- package target/orbs/mda-guide
```

This produces:

```text
target/orbs/mda-guide.orb
```

That file is the distributable artifact. It already contains:

- MCP runtime
- Web UI assets
- Orb manifest
- document metadata
- chunk data
- BM25 index

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
│   ├── mcporb-core/        # Document import, chunking, BM25 index, serialization
│   ├── mcporb-cli/         # CLI: build, inspect, list, run, test-query
│   ├── mcporb-runtime/     # Orb runtime: MCP stdio + axum Web UI
│   └── mcporb-wizard-gui/  # Tauri wizard GUI (v0.4+, scaffold only)
└── mda-orb/                # Sample PDF: MDA Guide v1.0.1
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
cargo bench -p mcporb-core
```
