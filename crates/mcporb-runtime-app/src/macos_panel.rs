//! Native NSOpenPanel wrapper for picking the Orb library folder.
//!
//! `tauri-plugin-dialog` (via `rfd`) reduces the panel result to a `PathBuf`,
//! discarding the security-scoped `NSURL` that NSOpenPanel returns. A bookmark
//! created from a path-derived URL carries no attached sandbox extension, so
//! resolving it later fails with "startAccessingSecurityScopedResource failed".
//! This module runs NSOpenPanel directly and creates the bookmark from the
//! panel's URL (toll-free bridged to `CFURLRef`) while it is still alive.

use std::path::PathBuf;

use objc2::rc::autoreleasepool;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSModalResponseOK, NSOpenPanel};
use objc2_foundation::{NSString, NSURL};

use crate::macos_access;

/// Result of a library-folder pick: the chosen path, its security-scoped
/// bookmark (persistable), and a live access guard for the session.
pub struct PickedLibrary {
    pub path: PathBuf,
    pub bookmark: String,
    pub guard: macos_access::AccessGuard,
}

/// Shows NSOpenPanel for a folder and creates a security-scoped bookmark from
/// the panel's URL, re-resolving it immediately so a bad bookmark surfaces
/// here instead of later when the user tries to apply the change.
///
/// Returns `Ok(None)` when the user cancels.
///
/// # Safety
///
/// Must be called on the main thread.
pub unsafe fn pick_library_folder(
    suggested: Option<PathBuf>,
) -> Result<Option<PickedLibrary>, String> {
    autoreleasepool(|_| {
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| "folder picker must run on the main thread".to_string())?;
        let panel = NSOpenPanel::openPanel(mtm);
        panel.setCanChooseFiles(false);
        panel.setCanChooseDirectories(true);
        panel.setAllowsMultipleSelection(false);
        panel.setCanCreateDirectories(true);
        if let Some(dir) = suggested {
            let url = NSURL::fileURLWithPath(&NSString::from_str(&dir.to_string_lossy()));
            panel.setDirectoryURL(Some(&url));
        }
        if panel.runModal() != NSModalResponseOK {
            return Ok(None);
        }
        let Some(url) = panel.URL() else {
            return Ok(None);
        };

        // Only the panel's own URL carries the security-scoped extension;
        // bookmark from it, then re-resolve to validate and get a live guard.
        let bookmark =
            macos_access::create_bookmark_from_url((&*url as *const NSURL).cast())?;
        let resolved = macos_access::resolve_bookmark(&bookmark)?;
        let guard = resolved.guard.ok_or_else(|| {
            "The picked folder did not grant sandbox access; please try picking it again.".to_string()
        })?;
        Ok(Some(PickedLibrary {
            path: resolved.path,
            bookmark,
            guard,
        }))
    })
}
