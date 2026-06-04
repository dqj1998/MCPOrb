# MCPOrb

🌐 [MCPOrb.ai](https://MCPOrb.ai) — The PDF for AI-native knowledge delivery.

A runtime-only repository for packaged MCP Orbs.

This repository owns the runtime that powers a standalone Orb executable:
- Exposes a **MCP Server** over stdio JSON-RPC for AI clients like Claude Desktop, Cursor, VS Code
- Serves a **local Web UI** at `http://127.0.0.1:<port>/<token>/` for human inspection
- Exposes **MCP Streamable HTTP** at `http://127.0.0.1:<port>/<token>/mcp` when the Web UI server is running
- Loads Orb assets produced elsewhere and serves multi-strategy retrieval at runtime (BM25, TF-IDF, Trigram, Vector, Hybrid)

## Quick Start

```bash
# Build the runtime
cargo build -p mcporb-runtime

# Build Builder-ready full/lite runtimes in target/release/
bash scripts/build-builder-runtimes.sh

# Run the runtime directly against an Orb assets directory
cargo run -p mcporb-runtime -- --assets target/orbs/mda-guide --gui-only --open
```

To build, inspect, or package Orbs, use the `MCPOrbBuilder` that is undergoing development.

## Packaged Orb Release

The packaged `.orb` file is still the preferred distributable artifact. It is produced by `MCPOrbBuilder`, but executed by the runtime in this repository.

### Run the packaged Orb

```bash
# Open the local Web UI
./target/orbs/mda-guide.orb --gui-only --open

# Expose MCP over stdio only
./target/orbs/mda-guide.orb --stdio-only

# Run MCP stdio, Web UI, and Streamable HTTP
./target/orbs/mda-guide.orb --all-gui
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
      "args": ["--assets", "/Users/qingjie.du/HDD/MCPOrb/target/orbs/mda-guide", "--all-gui"]
    }
  }
}
```

> **Note:** Build the runtime first with `cargo build -p mcporb-runtime`. For production use, replace `debug` with `release` and build with `cargo build --release -p mcporb-runtime`. If you want Builder-compatible staged binaries (`mcporb-runtime-full` / `mcporb-runtime-lite`) in a multi-repo workspace, run `bash scripts/build-builder-runtimes.sh`.

Single-file packaged Orb setup:

```json
{
  "mcpServers": {
    "mda-guide": {
      "command": "/Users/qingjie.du/HDD/MCPOrb/target/orbs/mda-guide.orb",
      "args": ["--all-gui"]
    }
  }
}
```

For production distribution, prefer the packaged `.orb` file over `target/debug/mcporb-runtime` plus a separate `target/orbs/<name>/` directory.

When an Orb runs with `--all-gui`, MCP clients should call the `get_web_ui_url` tool to discover the local Web UI address. The URL is not exposed as an MCP resource.

When the Web UI server is running, local MCP clients that support Streamable HTTP can connect to `http://127.0.0.1:<port>/<token>/mcp`. ChatGPT web/cloud cannot reach that local URL; ChatGPT Desktop can use it only if the installed build supports user-configured local Streamable HTTP MCP servers. If a client expects a remote connector/app URL, publish the Orb through a trusted HTTPS MCP bridge instead.

## Architecture

```
MCPOrb/
├── crates/
│   ├── mcporb-runtime/        # Orb runtime: MCP stdio + axum Web UI
│   ├── mcporb-runtime-core/   # Runtime-only data contracts and multi-strategy search logic
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
| All GUI | `./orb --all-gui` | MCP stdio + Web UI + Streamable HTTP |
| Remember unlock | `./orb --unlock` | Prompt once, store the unlock key in the OS keychain, then exit |

## Password Protection

An Orb can be packaged with an optional access password (see the Builder's
`mcporb package --password`). When enabled:

- The Web UI shows a login screen before the dashboard.
- MCP clients see only `unlock_orb` and `get_web_ui_url` until unlocked;
  `search_knowledge` and resources return a locked error.
- A single successful unlock opens the whole process. Because the default mode
  when an MCP client launches the Orb is `--all-gui`, unlocking once in the
  browser (via `get_web_ui_url`) also unlocks the in-process stdio MCP, so
  **the password never has to be typed into the LLM conversation**.
- `unlock_orb` (password passed to the tool) and `--unlock` (terminal prompt
  that remembers the unlock on this device via the OS keychain) are fallbacks,
  mainly for `--stdio-only` deployments.

Optionally, assets can be encrypted (`--encrypt-assets`): the packaged `.orb`
then embeds only the ciphertext, so documents, chunks, and indexes cannot be
extracted by unpacking the file without the password.

**Security limits.** Password gating and asset encryption protect the static
`.orb` at rest and gate local access; they are not unbreakable DRM. After a
correct password the runtime necessarily holds the decrypted, searchable data
in memory — encryption does **not** protect already-decrypted runtime memory.
Weak passwords remain crackable offline, so the Builder warns on short ones.

## Binary Size Budget

| Version | Budget |
|---------|--------|
| v0.1 (MVP) | ≤ 15 MB |
| v0.2 | ≤ 20 MB |

Check: `bash scripts/check-binary-size.sh`

## License

MCPOrb Runtime is licensed under the [Apache License 2.0](LICENSE).

MCPOrbBuilder is a separate commercial product with its own terms.

## Development

```bash
cargo check --workspace
cargo test --workspace
cargo build -p mcporb-runtime --release
```

## Public Orb

Selected showcase Orbs should be published under `public-orb/`.
This directory is intentionally kept in the runtime repository so public Orb releases can ship alongside the runtime brand.

