# mda-guide — Public Orb Demo

**MCPOrb: The PDF for AI-native knowledge delivery.**

This is the first public demo Orb for MCPOrb. It packages the
[MDA Guide v1.0.1](https://www.omg.org/mda/) — the Object Management Group's
foundational document on Model Driven Architecture — into a self-contained,
locally-runnable MCP knowledge asset.

---

## What this Orb contains

| Property | Value |
|---|---|
| Source | `00-2_MDA_Guide_v1.0.1.pdf` (OMG, 2003, public domain) |
| Pages | 62 |
| Chunks | 172 |
| Retrieval plan | `bm25-only` |
| Capabilities | BM25 lexical search |
| Format | Orb v0.1 |

---

## What you can ask

This Orb is best at answering questions about Model Driven Architecture concepts,
terminology, and the structure of the MDA approach. Good questions include:

1. **What is Model Driven Architecture?**
2. **What is the role of a Platform Independent Model (PIM)?**
3. **How does MDA describe transformation between models?**
4. **What is a Computation Independent Model (CIM)?**
5. **What is a Platform Specific Model (PSM)?**
6. **How does MDA relate to UML?**
7. **What is the MDA pattern?**
8. **Where does the guide discuss computation independent models?**

Known limitations:
- Single-word queries (e.g. `architects`) may return no results due to BM25 tokenization.
- Natural-language paraphrase queries work better with longer phrases.
- This Orb uses BM25-only retrieval (v0.1). Dense semantic search is planned for v0.2.

---

## Run locally (developer demo)

### Prerequisites

- Rust toolchain (`rustup`, `cargo`) — [install](https://rustup.rs)
- Both `MCPOrb` and `MCPOrbBuilder` repositories cloned as siblings

### Step 1: Build the MDA Orb

```bash
cd MCPOrbBuilder
cargo build --workspace

cargo run -p mcporb-cli -- build \
  ../MCPOrbEtc/fixtures/samples/00-2_MDA_Guide_v1.0.1.pdf \
  --name mda-guide \
  --description "MDA Guide v1.0.1 — Model Driven Architecture Guide from OMG"
```

Output: `MCPOrbBuilder/target/orbs/mda-guide/`

### Step 2: Inspect the Orb

```bash
cargo run -p mcporb-cli -- inspect target/orbs/mda-guide
```

### Step 3: Run a test query

```bash
cargo run -p mcporb-cli -- test-query target/orbs/mda-guide \
  "What is Model Driven Architecture" --top-k 5
```

### Step 4: Launch the Web UI

```bash
# Build the runtime first
cd ../MCPOrb
cargo build -p mcporb-runtime

# Launch with Web UI
cd ../MCPOrbBuilder
cargo run -p mcporb-cli -- run target/orbs/mda-guide --open
```

This opens `http://127.0.0.1:<port>/<token>/` in your browser.

The Web UI provides:
- **Search tab** — full-text BM25 search with scores, page numbers, and expandable results
- **Documents tab** — source document and section overview
- **MCP Config tab** — copy-paste config for Claude Desktop, Cursor, VS Code
- **About tab** — manifest details, retrieval plan, and planning rationale

---

## Use with MCP clients

### Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "mda-guide": {
      "command": "/path/to/mda-guide",
      "args": ["--stdio-only"]
    }
  }
}
```

### Cursor

Add to `.cursor/mcp.json` or Cursor settings:

```json
{
  "mda-guide": {
    "command": "/path/to/mda-guide",
    "args": ["--stdio-only"]
  }
}
```

### VS Code (with MCP extension)

```json
{
  "mcp.servers": {
    "mda-guide": {
      "command": "/path/to/mda-guide",
      "args": ["--stdio-only"]
    }
  }
}
```

Replace `/path/to/mda-guide` with the path to the packaged Orb binary.

---

## Available MCP tools

| Tool | Description |
|---|---|
| `search_knowledge` | BM25 search over the MDA Guide. Returns top-k chunks with scores and page numbers. |
| `get_web_ui_url` | Returns the local Web UI URL when the Orb is running in GUI mode. |

### Example `search_knowledge` call

```json
{
  "name": "search_knowledge",
  "arguments": {
    "query": "What is Model Driven Architecture",
    "top_k": 5
  }
}
```

---

## Package as a single executable (coming in v0.2)

```bash
cd MCPOrbBuilder
cargo run -p mcporb-cli -- package target/orbs/mda-guide \
  --output ./mda-guide.orb
```

This produces a single self-contained executable `mda-guide.orb` that embeds
all knowledge assets and the runtime. No Rust toolchain required to run it.

---

## Source and license

- **MDA Guide v1.0.1** — © 2003 Object Management Group. Reproduced for
  demonstration purposes. See the original document for copyright terms.
- **MCPOrb Runtime** — Apache License 2.0, see `MCPOrb/` repository.
- **MCPOrbBuilder** — commercial product, see `MCPOrbBuilder/` repository.

---

## About MCPOrb

> MCPOrb: The PDF for AI-native knowledge delivery.

MCPOrb lets B2B teams package product docs, service playbooks, and expert
knowledge into portable MCP Orbs that run locally inside AI tools.

- **Runtime** is open source — inspect the code, trust the binary.
- **Builder** is the commercial tool for producing and maintaining Orbs at scale.
- **public-orb/** contains curated demo Orbs for the community.

Learn more: [MCPOrb.com](https://mcporb.com) · [GitHub](https://github.com/mcporb)
