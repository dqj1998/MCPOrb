# ai-governance — Public Orb Demo

**Status:** zero-config — all retrieval indices reachable from the Wizard GUI's *Advanced Options* panel.

This public Orb shows MCPOrb packaging a large, mixed-format knowledge set for governance, policy, and compliance work. The demo proves that one portable knowledge asset can be built locally, inspected with source traceability, and queried with multiple search modes over a heterogeneous document set.

## Folder layout

```
ai-governance/
├── README.md                          ← this file (zero other docs)
└── sources/
    ├── AI_Governance_Control_Compendium.md   ← primary build source
    └── others/                        ← 10 additional sources
        ├── NIST_AI_RMF_Framework.pdf
        ├── NIST_AI_RMF_GenAI_Profile.pdf
        ├── NIST_AI_RMF_Playbook.html
        ├── EC_AI_Act_Overview.html
        ├── EC_AI_Act_Overview.docx
        ├── EU_Parl_AI_Office_Presentation.pptx
        ├── AI_Governance_Bilingual_FAQ.md
        ├── AI_Governance_Risk_Scenario_Library.md
        ├── AI_Governance_Launch_Readiness_Checklist.md
        └── AI_Governance_Glossary.md
```

There is no `INDEX.md`, no `SOURCES.md`, no `build.plan.json`, no `demo-embeddings.bin`, no `tools/`. The Builder makes every planning decision from the source set; user-adjustable knobs live in the GUI's *Advanced Options* panel.

## Sources

| File | Format | Publisher | Origin |
|---|---|---|---|
| `sources/others/NIST_AI_RMF_Framework.pdf` | PDF | NIST | `https://nvlpubs.nist.gov/nistpubs/ai/NIST.AI.100-1.pdf` |
| `sources/others/NIST_AI_RMF_GenAI_Profile.pdf` | PDF | NIST | `https://nvlpubs.nist.gov/nistpubs/ai/NIST.AI.600-1.pdf` |
| `sources/others/NIST_AI_RMF_Playbook.html` | HTML | NIST | `https://airc.nist.gov/AI_RMF_Knowledge_Base/AI_RMF` |
| `sources/others/EC_AI_Act_Overview.html` | HTML | European Commission | `https://digital-strategy.ec.europa.eu/en/policies/regulatory-framework-ai` |
| `sources/others/EC_AI_Act_Overview.docx` | DOCX | European Commission | derived locally from the same Commission text export |
| `sources/others/EU_Parl_AI_Office_Presentation.pptx` | PPTX | European Parliament | `https://www.europarl.europa.eu/cmsdata/300890/Mr%20Boulange%20-%20AFCO%20Workshop%20-%20AI%20Office%20Presentation%20-%203%20December%202025%20-%20v1.2.pptx` |
| `sources/AI_Governance_Control_Compendium.md` | Markdown | MCPOrb demo supplement | locally authored — long operational control narrative |
| `sources/others/AI_Governance_Bilingual_FAQ.md` | Markdown | MCPOrb demo supplement | locally authored — Chinese + English governance prompts |
| `sources/others/AI_Governance_Risk_Scenario_Library.md` | Markdown | MCPOrb demo supplement | locally authored — ten governance scenarios |
| `sources/others/AI_Governance_Launch_Readiness_Checklist.md` | Markdown | MCPOrb demo supplement | locally authored — bilingual control checklist |
| `sources/others/AI_Governance_Glossary.md` | Markdown | MCPOrb demo supplement | locally authored — 60+ bilingual term definitions |

11 files across 5 formats (PDF, HTML, DOCX, PPTX, Markdown).

### Provenance and distribution

- NIST materials are the safest anchors for a public demo corpus.
- European Commission pages are official and useful, but the site can rate-limit repeated fetches; review the Commission legal notice before any public packaged release.
- The European Parliament PPTX is an official public presentation and is the strongest current PPTX anchor for governance-specific demo questions.
- No acceptable official direct DOCX could be verified for the EC AI Act overview, so the `.docx` here is a local conversion from the official text.
- The five Markdown supplements are intentionally local demo materials. They do not replace the official sources; they widen the retrieval surface so the corpus can light up TF-IDF, trigram, and dense retrieval in one package.
- Demo embeddings are never checked into this directory. With *Synthesize Demo Embeddings* enabled in Advanced Options, the Builder generates deterministic per-chunk vectors in memory at build time — these are not real semantic embeddings and should not be redistributed as model output.

## How to run

### Wizard GUI

1. Drop the eleven `sources/` files into the wizard. Any one can be the primary — the choice only affects the orb's default name/title.
2. Open *Advanced Options* and set:
   - ☑ **Allow Typo Tolerance** (default — activates Trigram)
   - ☑ **Force TF-IDF** (activates TF-IDF on thematic content where auto-gates would skip it)
   - ☑ **Synthesize Demo Embeddings**, *Demo Embedding Dimension* = 64 (activates FlatVector via in-Builder synthesis)
   - **Force Retrieval Plan → BM25 + HNSW** (promotes the dense tier from FlatVector to HNSW on this small corpus)
3. Leave *Plan File* and *Embeddings File* blank.
4. *Preview Plan* — the rationale should read:

```
- rationale.bm25_always
- rationale.tfidf_forced          {avg_chunk_len=524, term_density=Low}
- rationale.trigram_enabled_typo
- rationale.plan_forced           {plan=bm25-hnsw}
```

5. *Build Orb Assets* — the output directory will contain `bm25_index.postcard`, `tfidf_index.postcard`, `trigram_index.postcard`, `vector_store.postcard`, `hnsw_index.postcard`.

### CLI (single-source spot-check)

The CLI builds one source at a time; for a full 11-file build use the Wizard
GUI. For a spot-check using the primary Markdown source:

```bash
cd MCPOrbBuilder
cargo run -p mcporb-cli -- build \
    ../MCPOrb/public-orb/AI-Governance/sources/AI_Governance_Control_Compendium.md \
    --name ai-governance \
    --description "Public AI governance corpus for MCPOrb demo" \
    --allow-typo-tolerance \
    --force-tfidf \
    --synthesize-embeddings-demo \
    --plan bm25-hnsw

cargo run -p mcporb-cli -- package target/orbs/ai-governance \
    --output ../MCPOrb/public-orb/AI-Governance/ai-governance.orb
```

## Indexing methods on this corpus

| Capability | Auto trigger | What this corpus actually needs |
|---|---|---|
| BM25 | always on | nothing |
| TF-IDF | `avg_chunk_len >= 600` AND `term_density >= High` | check **Force TF-IDF** — auto-gates skip because the aggregated unique-token ratio on thematic governance content stays in the Low band (~0.22) even though individual supplements hit ~0.45 locally |
| Trigram (typo-tolerant) | CJK ratio ≥ 0.20 OR typo-tolerance | the default **Allow Typo Tolerance** checkbox already covers this |
| FlatVector | embeddings file present OR synthesize-demo on, AND `is_dense_worth_it` | check **Synthesize Demo Embeddings** |
| HNSW | FlatVector AND `chunk_count >= hnsw_min_chunks` (default 15 000) | set **Force Retrieval Plan → BM25 + HNSW** — 772 chunks is below the production HNSW threshold, so the override is the explicit way to demo HNSW on a small corpus |

## Demo questions this Orb should support

- What controls should a team prepare before launching a high-risk AI workflow?
- Which sources discuss human oversight requirements?
- Where can I find a practical AI risk assessment checklist?
- Which documents discuss model transparency and documentation?
- 高风险 AI 工作流上线前需要准备哪些治理材料？
- 哪些文档涉及人工监督和事故响应？

## Why this corpus is a good demo

**Source availability:** high.

- Many core documents are published by governments, standards bodies, and policy organizations.
- The corpus mixes PDF, HTML, DOCX, PPTX, and Markdown — useful for testing multi-format ingestion.
- The topic is broad enough that we do not depend on one single publisher.

**Caveat:** some standards and guidance documents are public to read but not always safe to redistribute verbatim. Prefer materials with clearly public distribution terms or official government publication status.

**Wishlist (for future revisions):**

1. the European Commission AI Act FAQ page (license review pending)
2. a stronger official direct DOCX alternative to replace the locally-generated `.docx`
3. an official OECD or regulator-published policy explainer once redistribution terms have been verified
