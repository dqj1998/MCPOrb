# public-orb — Curated Public Demo Orbs

**MCPOrb: The PDF for AI-native knowledge delivery.**

This directory contains public demo Orbs and source packs used to explain what an
Orb looks like in practice.

Each demo folder is intended to hold:

- raw source materials
- a packaged `.orb` output when built
- a local README describing scope, sources, build notes, and naming conventions

## Available folders

| Folder | Theme | Source availability | Status |
|---|---|---|---|
| `MDA/` | Model Driven Architecture reference guide | high | built |
| `AI-Governance/` | AI governance, policy, and compliance corpus | high | seeded with first public source batch |
| `Industrial-Safety-Ops/` | industrial safety, maintenance, and training corpus | medium | planned |
| `Public-Procurement-Delivery/` | public procurement and project delivery corpus | medium-high | planned |

## Current layout decision

Yes: single-demo documentation belongs in the demo folder itself.

That means:

- `MDA/README.md` holds the MDA-specific walkthrough
- this top-level README stays as the index for all public demo folders

## Multi-source builds

The **Wizard GUI** accepts multiple source files and folders in one build.
Drop any mix of PDF, HTML, DOCX, PPTX, and Markdown files into the source
list; the Builder ingests them all, merges the chunks, and selects a single
retrieval plan over the combined corpus.

The **CLI** (`mcporb build`) still takes one source file at a time.
For multi-document demos via CLI, the recommended pattern is:

1. keep the raw public source files in the demo folder
2. run `mcporb build` once per source (or pick the most representative one
   for a spot-check)
3. for a full multi-source build, use the Wizard GUI

## Suggested naming convention

- Folder name: theme-oriented and human-readable
- Orb internal name: lowercase kebab-case, e.g. `ai-governance`
- Consolidated build source: `Theme_Public_Corpus_v1.md` or `.pdf`
- Packaged Orb filename: `Theme_Public_Corpus_v1.orb`

See each demo folder for the concrete target names.
