//! Canonical macOS App Sandbox security-scoped bookmark helpers.
//!
//! Shared by the Runner GUI (`mcporb-runtime-app`), the gateway core (via
//! `mcporb-runtime-app-core`, which re-exports this as `macos_access`), and the
//! runtime child (`mcporb-runtime`). Previously each of those crates carried
//! its own byte-identical copy; a one-bit error in the bookmark-resolution
//! constant then had to be fixed in three places at once. Keeping a single
//! canonical implementation here removes that divergence class.
//!
//! The crate compiles to nothing on non-macOS targets, so it is safe to depend
//! on unconditionally (consumers gate their use with `cfg(target_os = "macos")`).

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;
