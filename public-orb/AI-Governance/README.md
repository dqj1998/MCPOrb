# ai-governance — Public Orb Demo

**MCPOrb: The PDF for AI-native knowledge delivery.**

This Orb packages a large, mixed-format knowledge set covering AI governance, policy, and compliance
into a single portable knowledge asset. It supports BM25 lexical search, TF-IDF, trigram
(typo-tolerant), and dense vector retrieval in one package.

---

## Folder contents

| File | Purpose |
|---|---|
| `AI_Governance_Control_Compendium.orb` | Packaged Orb executable — use this directly |
| `sources/` | Original source documents (for reference) |

---

## What this Orb contains

| Property | Value |
|---|---|
| Sources | 11 files across 5 formats (PDF, HTML, DOCX, PPTX, Markdown) |
| Retrieval plan | BM25 + TF-IDF + Trigram + HNSW |
| Capabilities | Lexical, typo-tolerant, and dense vector search |
| Format | Orb v0.1 |

### Source documents

| File | Format | Publisher |
|---|---|---|
| `NIST_AI_RMF_Framework.pdf` | PDF | NIST |
| `NIST_AI_RMF_GenAI_Profile.pdf` | PDF | NIST |
| `NIST_AI_RMF_Playbook.html` | HTML | NIST |
| `EC_AI_Act_Overview.html` | HTML | European Commission |
| `EC_AI_Act_Overview.docx` | DOCX | European Commission |
| `EU_Parl_AI_Office_Presentation.pptx` | PPTX | European Parliament |
| `AI_Governance_Control_Compendium.md` | Markdown | MCPOrb demo supplement |
| `AI_Governance_Bilingual_FAQ.md` | Markdown | MCPOrb demo supplement |
| `AI_Governance_Risk_Scenario_Library.md` | Markdown | MCPOrb demo supplement |
| `AI_Governance_Launch_Readiness_Checklist.md` | Markdown | MCPOrb demo supplement |
| `AI_Governance_Glossary.md` | Markdown | MCPOrb demo supplement |

---

## Open the Web UI

Run the Orb without `--stdio-only` to start the built-in Web UI:

```bash
/path/to/AI_Governance_Control_Compendium.orb --open
```

- `--open` automatically opens the browser at `http://127.0.0.1:<port>/<token>/`.
- Omit `--open` to start the server without launching a browser; the URL is printed to stdout.
- The Web UI lets you run queries interactively and inspect retrieved chunks with scores and source attribution.

Once the Orb is running, you can also ask the LLM to call the `get_web_ui_url` tool — it returns
the URL, which you can open directly in your browser.

---

## Use with MCP clients

### Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "ai-governance": {
      "command": "/path/to/AI_Governance_Control_Compendium.orb",
      "args": ["--stdio-only"]
    }
  }
}
```

### Cursor

Add to `.cursor/mcp.json` or Cursor settings:

```json
{
  "ai-governance": {
    "command": "/path/to/AI_Governance_Control_Compendium.orb",
    "args": ["--stdio-only"]
  }
}
```

### VS Code (with MCP extension)

```json
{
  "mcp.servers": {
    "ai-governance": {
      "command": "/path/to/AI_Governance_Control_Compendium.orb",
      "args": ["--stdio-only"]
    }
  }
}
```

Replace `/path/to/AI_Governance_Control_Compendium.orb` with the absolute path to the Orb file.

---

## Available MCP tools

| Tool | Description |
|---|---|
| `search_knowledge` | Search the governance corpus. Accepts an optional `method` parameter: `"auto"` (default), `"bm25"`, `"vector"`, or `"hybrid"`. |
| `get_web_ui_url` | Returns the local Web UI URL when the Orb is running in GUI mode. |

### Example `search_knowledge` call

```json
{
  "name": "search_knowledge",
  "arguments": {
    "query": "What controls should a team prepare before launching a high-risk AI workflow?",
    "top_k": 5
  }
}
```

---

## What you can ask

- What controls should a team prepare before launching a high-risk AI workflow?
- Which sources discuss human oversight requirements?
- Where can I find a practical AI risk assessment checklist?
- Which documents discuss model transparency and documentation?
- 高风险 AI 工作流上线前需要准备哪些治理材料？
- 哪些文档涉及人工监督和事故响应？

---

## Source and license

- NIST materials are U.S. government publications in the public domain.
- European Commission and European Parliament materials are official public publications. Review the Commission legal notice before any public packaged redistribution.
- The five Markdown supplements are locally authored demo materials for MCPOrb demonstration purposes.
- **MCPOrb Runtime** — Apache License 2.0, see `MCPOrb/` repository.
- **MCPOrbBuilder** — commercial product, see `MCPOrbBuilder/` repository.
