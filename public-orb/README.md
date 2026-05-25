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

## Current builder limitation

Today, `mcporb build` accepts a single Markdown or PDF source file.

So for multi-document public demos, the recommended directory pattern is:

1. keep the raw public source files in the demo folder
2. curate a canonical single-source build document for the current CLI
3. package the resulting Orb back into the same demo folder

## Suggested naming convention

- Folder name: theme-oriented and human-readable
- Orb internal name: lowercase kebab-case, e.g. `ai-governance`
- Consolidated build source: `Theme_Public_Corpus_v1.md` or `.pdf`
- Packaged Orb filename: `Theme_Public_Corpus_v1.orb`

See each demo folder for the concrete target names.
