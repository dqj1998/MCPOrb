# What's New in MCPOrb Runner 1.3.5

## Smarter Gateway Port Handling

The unified HTTP gateway is now resilient when your preferred port is occupied.

- **Automatic fallback** — If the configured port (default 5599) is busy, the app probes subsequent ports and shows a non-blocking note in the **HTTP** tab with the actual port in use, so MCP clients can reconnect immediately.
- **Reuse, don't proliferate** — Launching the app repeatedly no longer spawns a new gateway each time. The runner probes `127.0.0.1:<port>/mcp?token=<token>` with an `initialize` request; if the existing gateway answers HTTP 200, it is reused. This eliminates `EADDRINUSE` errors on quick restarts.
- **Accurate status after relaunch** — Even if the previous UI quit while the gateway was orphaned, `unified_gateway_status` now detects it via the token probe and reports `running: true` correctly.

## macOS Sandbox Reliability (App Review 2.4.5)

Orbs stored outside the sandbox container (e.g. a user-picked `~/Documents/MCPOrb` folder) now launch reliably on the Mac App Store build.

- The gateway detects when `zip_path` lives outside the sandbox container and streams the ZIP over **stdin** to the `mcporb-runtime` child (`--orb-zip-stdin` with an 8-byte little-endian length header). The child reads the ZIP first, then serves MCP on the same pipe — no direct file access required.
- Fixes the scenario where moving the library or leaving an adopted gateway previously surfaced as "failed to read Orb ZIP".

## Library Folder & Settings Robustness

- **Settings location** moved from the sandbox container to `~/.mcporb/` so preferences survive app updates and container resets.
- **Missing library detection** — If the chosen Orb library folder is moved, renamed or deleted, a banner offers **one-click re-select**. The `NSOpenPanel` is pre-navigated to the stale location and the stale security-scoped bookmark is kept until you pick a new folder.
- **Correct merges** — `merge_settings` now preserves `orb_library_bookmark`, gateway token and onboarding flag when the library path changes. Theme (Light / Dark / System) no longer resets unexpectedly.

---

*This is a reliability release: fewer restarts, fewer permission prompts, and sandbox-correct behavior for the Store build. No Store listing changes required except the version bump (1.3.5.0).*
