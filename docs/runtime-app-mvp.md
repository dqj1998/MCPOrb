# MCPOrb Runtime App MVP Notes

## Plan audit

The Runner Upgrade Plan is directionally sound: Runtime should become an installed app that imports Orb ZIPs, provides native search, and exposes MCP STDIO/HTTP entry points while Builder focuses on producing and publishing ZIPs. The following adjustments are needed before treating the plan as store-ready:

- Keep the existing no-UI `mcporb-runtime` as the MCP execution binary. The new desktop Runtime App must live in separate crates and call app-facing APIs instead of absorbing the legacy runtime entry point.
- Freeze Orb ZIP v1 constants in shared code, not only in prose. Runtime, Builder, and Store need the same file allowlist, size limits, manifest fields, and compatibility rules.
- Resolve the encrypted ZIP manifest conflict. Existing encrypted packaging may emit only `orb_security.json` plus `orb_assets.enc`, while Runtime App import and Store catalog need a public `orb_manifest.json` for display and version checks.
- Treat HTTP MCP as a separate release gate. Mac App Store `network.server` approval remains unresolved, so MVP 1 should avoid enabling HTTP server features in the MAS build.
- STDIO config must be executable on day one. The Runtime App now generates config against the existing no-UI runtime using `--orb-zip <managed zip> --stdio-only`.
- Store and Builder work remains outside this repository slice when write operations are limited to `MCPOrb/`.

## Implemented in this repository

- `mcporb-runtime-core` manifest schema now includes ZIP v1 metadata: `display_name`, `runtime_min_version`, `builder_version`, `created_at`, `assets_sha256`, and `encrypted`.
- `mcporb-runtime-app-core` provides isolated Runtime App APIs:
  - safe Orb ZIP validation with path traversal, allowlist, file count, per-file size, and total size checks;
  - ZIP and asset SHA256 calculation;
  - local registry load/save/import with managed ZIP copies;
  - plaintext Orb search through `mcporb-runtime-core`;
  - STDIO MCP config snippets for Claude Desktop, Cursor, and VS Code.
- `mcporb-runtime-app` provides the Tauri desktop shell:
  - Search, Library, Import, MCP Config, Store, Running, and Settings tabs;
  - Store/HTTP tabs are explicit placeholders for later MVP gates;
  - deep link scheme registration for `mcporb://`;
  - Mac sandbox entitlements for selected-file import without enabling `network.server` yet.
- `mcporb-runtime` remains independently buildable and gains `--orb-zip <path>` for STDIO execution of imported ZIPs.

## Store readiness status

MVP 1 within `MCPOrb/` is implemented and compile-validated. Mac/Windows Store submission still needs repository-external work from the plan:

- Builder must export ZIP v1 with shared constants and public manifest for encrypted Orbs.
- Store/licensing-site must expose `/api/client/v1/*`, canonical ZIP catalog/downloads, and stop new derived runnable artifacts.
- Runtime MVP 2/3 require Store API and auth endpoints that are outside the permitted write scope.
- Runtime MVP 4 HTTP MCP should wait for the Mac App Store `network.server` decision; direct-download builds can enable it separately.
- Final store submission requires production icons, localized store listing text, screenshots, privacy answers, signing identities, and CI packaging jobs.

## Validation commands

```bash
cargo test -p mcporb-runtime-app-core
cargo check -p mcporb-runtime-app
cargo check -p mcporb-runtime --no-default-features
```
