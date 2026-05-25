# mda-guide — Public Orb Demo

**MCPOrb: The PDF for AI-native knowledge delivery.**

This Orb packages the [MDA Guide v1.0.1](https://www.omg.org/mda/) — the Object Management Group's
foundational document on Model Driven Architecture — into a self-contained, locally-runnable MCP
knowledge asset.

---

## Folder contents

| File | Purpose |
|---|---|
| `MDA_ModelDrivenArchitecture_Guide_v1.0.1.orb` | Packaged Orb executable — use this directly |
| `00-2_MDA_Guide_v1.0.1.pdf` | Original public source document |

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

## Open the Web UI

Run the Orb without `--stdio-only` to start the built-in Web UI:

```bash
/path/to/MDA_ModelDrivenArchitecture_Guide_v1.0.1.orb --open
```

- `--open` automatically opens the browser at `http://127.0.0.1:<port>/<token>/`.
- Omit `--open` to start the server without launching a browser; the URL is printed to stdout.
- The Web UI lets you run queries interactively and inspect retrieved chunks with scores and page numbers.

Once the Orb is running, you can also ask the LLM to call the `get_web_ui_url` tool — it returns
the URL, which you can open directly in your browser.

---

## Use with MCP clients

### Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "mda-guide": {
      "command": "/path/to/MDA_ModelDrivenArchitecture_Guide_v1.0.1.orb",
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
    "command": "/path/to/MDA_ModelDrivenArchitecture_Guide_v1.0.1.orb",
    "args": ["--stdio-only"]
  }
}
```

### VS Code (with MCP extension)

```json
{
  "mcp.servers": {
    "mda-guide": {
      "command": "/path/to/MDA_ModelDrivenArchitecture_Guide_v1.0.1.orb",
      "args": ["--stdio-only"]
    }
  }
}
```

Replace `/path/to/MDA_ModelDrivenArchitecture_Guide_v1.0.1.orb` with the absolute path to the Orb file.

---

## Available MCP tools

| Tool | Description |
|---|---|
| `search_knowledge` | Lexical search over the MDA Guide. Returns top-k chunks with scores and page numbers. Accepts an optional `method` parameter (`"bm25"` — default for this Orb). |
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

## Source and license

- **MDA Guide v1.0.1** — © 2003 Object Management Group. Reproduced for demonstration purposes. See the original document for copyright terms.
- **MCPOrb Runtime** — Apache License 2.0, see `MCPOrb/` repository.
- **MCPOrbBuilder** — commercial product, see `MCPOrbBuilder/` repository.
