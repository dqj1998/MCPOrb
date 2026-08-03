Password-protected artifact download flow evidence
==================================================

Date: 2026-07-14

Changed files
-------------
- `crates/mcporb-runtime-app/frontend/app.js`
- `crates/mcporb-runtime-app/frontend/index.html`
- `crates/mcporb-runtime-app/src/main.rs`

Implemented contract
--------------------
- `storeDownloadArtifact(artifactId, hasPassword)` branches on boolean/string password state.
- Password-protected artifacts open `#store-password-dialog`, clear input/status, focus password input, and store `state.pendingDownloadArtifactId`.
- Non-password artifacts call `invoke('store_download_artifact', { artifactId, token: null })` directly.
- Submit calls `store_verify_download_password` and then `store_download_artifact` with the returned token.
- Cancel hides the dialog and clears the pending artifact ID.
- Detail view artifact buttons bind click listeners via `data-artifact-id` / `data-artifact-password` and call `storeDownloadArtifact(artifact.id, artifact.has_password)` without inline string injection.
- Backend `store_download_artifact` accepts `token: Option<String>`, downloads to `{download_dir}/store/{artifactId}.zip`, and returns a path/size success string.

Verification
------------
- Executed: `cargo check --package mcporb-runtime-app`
- Result: passed (`Finished dev profile ... target(s) in 7.51s`).

LSP diagnostics
---------------
- Attempted `lsp_diagnostics` on changed Rust/JS/HTML files.
- Rust LSP unavailable: `rust-analyzer` missing from stable toolchain.
- JS LSP unavailable: `typescript-language-server` not installed.
- HTML/Biome LSP unavailable: `biome` not installed.
