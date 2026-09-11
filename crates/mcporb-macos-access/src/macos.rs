// macOS App Sandbox security-scoped bookmark helpers.
//
// Sandboxed apps lose access to user-selected folders after relaunch; a
// security-scoped bookmark is the only way to regain it. The public
// CoreFoundation C API (CFURL.h) exposes everything needed, so no third-party
// dependency is required.

use std::ffi::{c_void, CStr};
use std::os::raw::c_char;
use std::path::{Path, PathBuf};

use base64::Engine;

// These MUST match Apple's CFURL.h. Getting the RESOLUTION value wrong (e.g.
// using the CREATION bit 1<<11) resolves a bookmark WITHOUT its security scope,
// so CFURLStartAccessingSecurityScopedResource always returns false regardless
// of entitlements or how fresh the bookmark is. `constants_match_sdk_header`
// below asserts these against the installed SDK's CFURL.h so a wrong bit can
// never be reintroduced silently.
const K_CFURL_BOOKMARK_CREATION_WITH_SECURITY_SCOPE: u32 = 1 << 11; // kCFURLBookmarkCreationWithSecurityScope = 2048
const K_CFURL_BOOKMARK_RESOLUTION_WITH_SECURITY_SCOPE: u32 = 1 << 10; // kCFURLBookmarkResolutionWithSecurityScope = 1024
const K_CFURL_BOOKMARK_RESOLUTION_WITHOUT_UI_MODAL_PROMPTS: u32 = 1 << 8; // kCFURLBookmarkResolutionWithoutUIMask = 256

#[repr(C)]
struct __CFURL(c_void);
#[repr(C)]
struct __CFData(c_void);

type CFURLRef = *const __CFURL;
type CFDataRef = *const __CFData;
type CFAllocatorRef = *const c_void;
type CFIndex = isize;
type Boolean = u8;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFURLGetFileSystemRepresentation(
        url: CFURLRef,
        resolve_against_base: Boolean,
        buffer: *mut c_char,
        buffer_len: CFIndex,
    ) -> Boolean;
    fn CFURLCreateBookmarkData(
        allocator: CFAllocatorRef,
        url: CFURLRef,
        options: u32,
        resource_properties_to_include: *const c_void,
        relative_to_url: CFURLRef,
        error: *mut *const c_void,
    ) -> CFDataRef;
    fn CFURLCreateByResolvingBookmarkData(
        allocator: CFAllocatorRef,
        bookmark_data: CFDataRef,
        options: u32,
        relative_to_url: CFURLRef,
        resource_properties_to_return: *const c_void,
        is_stale: *mut Boolean,
        error: *mut *const c_void,
    ) -> CFURLRef;
    fn CFURLStartAccessingSecurityScopedResource(url: CFURLRef) -> Boolean;
    fn CFURLStopAccessingSecurityScopedResource(url: CFURLRef);
    fn CFDataCreate(allocator: CFAllocatorRef, bytes: *const u8, length: CFIndex) -> CFDataRef;
    fn CFDataGetBytePtr(data: CFDataRef) -> *const u8;
    fn CFDataGetLength(data: CFDataRef) -> CFIndex;
    fn CFRelease(cf: *const c_void);
}

/// Holds a security-scoped URL and stops access when dropped. Keep it alive
/// for as long as the resolved folder must remain readable.
pub struct AccessGuard {
    url: CFURLRef,
}

// Safety: the guard is only ever passed around and stopped on drop; the
// underlying CFURL is immutable and CoreFoundation's start/stop access calls
// are thread-safe, so sharing the guard across threads is sound.
unsafe impl Send for AccessGuard {}
unsafe impl Sync for AccessGuard {}

impl Drop for AccessGuard {
    fn drop(&mut self) {
        unsafe {
            CFURLStopAccessingSecurityScopedResource(self.url);
            CFRelease(self.url.cast());
        }
    }
}

/// Creates a security-scoped bookmark from an existing CFURL and returns it
/// base64-encoded for persistence.
///
/// The URL MUST carry an attached security-scoped extension — i.e. the
/// toll-free-bridged `NSURL` returned by `NSOpenPanel`. A URL rebuilt from a
/// plain path string (the old `create_bookmark(&Path)` approach) has no
/// attached extension, so the resulting bookmark contains no usable security
/// scope: resolving it later succeeds but `CFURLStartAccessingSecurityScopedResource`
/// returns false ("startAccessingSecurityScopedResource failed").
///
/// # Safety
///
/// `url` must be a valid CFURLRef (or toll-free-bridged NSURL pointer).
pub unsafe fn create_bookmark_from_url(url: *const c_void) -> Result<String, String> {
    let bookmark = CFURLCreateBookmarkData(
        std::ptr::null(),
        url as CFURLRef,
        K_CFURL_BOOKMARK_CREATION_WITH_SECURITY_SCOPE,
        std::ptr::null(),
        std::ptr::null(),
        std::ptr::null_mut(),
    );
    if bookmark.is_null() {
        return Err("CFURLCreateBookmarkData returned null".to_string());
    }
    let len = CFDataGetLength(bookmark) as usize;
    let ptr = CFDataGetBytePtr(bookmark);
    let bytes = std::slice::from_raw_parts(ptr, len);
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    CFRelease(bookmark.cast());
    Ok(encoded)
}

/// Result of resolving a persisted security-scoped bookmark.
pub struct ResolvedBookmark {
    /// Folder the bookmark points at.
    pub path: PathBuf,
    /// Live access handle. `None` when access could not be granted this
    /// session (stale bookmark after app update/reinstall).
    pub guard: Option<AccessGuard>,
    /// Rebuilt base64 bookmark from the resolved URL when the stored one was
    /// stale — Apple's documented recovery. Persisting it makes the NEXT launch
    /// start healthy.
    pub refreshed: Option<String>,
}

/// Resolves a persisted base64 bookmark back to a folder path and starts
/// security-scoped access to it.
pub fn resolve_bookmark(encoded: &str) -> Result<ResolvedBookmark, String> {
    unsafe {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| format!("invalid bookmark data: {e}"))?;
        let bookmark = CFDataCreate(std::ptr::null(), bytes.as_ptr(), bytes.len() as CFIndex);
        if bookmark.is_null() {
            return Err("CFDataCreate failed".to_string());
        }
        let mut is_stale: Boolean = 0;
        // kCFURLBookmarkResolutionWithoutUIModalPrompts: resolving with only
        // the security-scope flag makes CoreFoundation attempt a consent UI
        // when the extension needs renewal and fail headless. The silent-renewal
        // flag is what Apple's samples use for relaunch restoration.
        let options = K_CFURL_BOOKMARK_RESOLUTION_WITH_SECURITY_SCOPE
            | K_CFURL_BOOKMARK_RESOLUTION_WITHOUT_UI_MODAL_PROMPTS;
        let url = CFURLCreateByResolvingBookmarkData(
            std::ptr::null(),
            bookmark,
            options,
            std::ptr::null(),
            std::ptr::null(),
            &mut is_stale,
            std::ptr::null_mut(),
        );
        CFRelease(bookmark.cast());
        if url.is_null() {
            return Err(
                "bookmark could not be resolved (folder may have been moved or deleted)"
                    .to_string(),
            );
        }
        if is_stale != 0 {
            tracing::warn!("resolved security-scoped bookmark is stale");
        }
        let path = path_from_url(url)?;

        // Start access FIRST. Creating a fresh security-scoped bookmark
        // (CFURLCreateBookmarkData with the security-scope option) requires the
        // resolved URL to be actively accessed — building it before starting
        // access always returns null. The URL must be accessed before any I/O
        // regardless.
        let access_ok = CFURLStartAccessingSecurityScopedResource(url) != 0;

        // Stale recovery: rebuild the bookmark from the resolved URL while
        // access is held, so the caller can persist a non-stale copy for the
        // next launch. Only possible once access has actually started.
        let refreshed = if is_stale != 0 && access_ok {
            match create_bookmark_from_url(url.cast()) {
                Ok(fresh) => Some(fresh),
                Err(error) => {
                    tracing::warn!(
                        %error,
                        "failed to refresh stale Orb library bookmark"
                    );
                    None
                }
            }
        } else {
            None
        };

        let guard = if access_ok {
            Some(AccessGuard { url })
        } else {
            tracing::warn!(
                "startAccessingSecurityScopedResource failed; folder not accessible this \
                 session — re-select the Orb library folder in Settings to restore access"
            );
            // No guard owns the URL; release it here.
            CFRelease(url.cast());
            None
        };
        Ok(ResolvedBookmark {
            path,
            guard,
            refreshed,
        })
    }
}

/// Resolve a security-scoped bookmark, start access, and read a file.
///
/// Returns `(guard, bytes)` — the caller MUST keep the `AccessGuard` alive
/// for as long as `bytes` is used. Dropping the guard revokes the sandbox
/// extension, making further I/O on the resolved path fail with EPERM.
pub fn read_file_via_bookmark(
    encoded_bookmark: &str,
    path: &Path,
) -> Result<(AccessGuard, Vec<u8>), String> {
    let resolved = resolve_bookmark(encoded_bookmark)?;
    let guard = resolved.guard.ok_or_else(|| {
        "security-scoped bookmark did not grant access (may be stale after app update)".to_string()
    })?;
    let bytes =
        std::fs::read(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    Ok((guard, bytes))
}

unsafe fn path_from_url(url: CFURLRef) -> Result<PathBuf, String> {
    let mut buffer = [0 as c_char; 4096];
    if CFURLGetFileSystemRepresentation(url, 1, buffer.as_mut_ptr(), buffer.len() as CFIndex) == 0 {
        return Err("CFURLGetFileSystemRepresentation failed".to_string());
    }
    let c_str = CStr::from_ptr(buffer.as_ptr());
    Ok(PathBuf::from(c_str.to_string_lossy().into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_file_via_bookmark_rejects_invalid_bookmark() {
        let result = read_file_via_bookmark("not-a-valid-bookmark", Path::new("/tmp/nonexistent"));
        match result {
            Ok(_) => panic!("expected error for invalid bookmark"),
            Err(err) => {
                assert!(
                    err.contains("invalid bookmark data")
                        || err.contains("bookmark could not be resolved"),
                    "unexpected error: {err}"
                );
            }
        }
    }

    #[test]
    fn read_file_via_bookmark_rejects_empty_bookmark() {
        let result = read_file_via_bookmark("", Path::new("/tmp/nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn resolve_bookmark_rejects_garbage_data() {
        let result = resolve_bookmark("!!!not-base64!!!");
        assert!(result.is_err());
    }

    /// Guards against the exact regression that broke Runner 1.5.0–1.5.2: the
    /// bookmark-resolution constant was `1 << 11` (the CREATION value) instead
    /// of `1 << 10`, so every resolve dropped the security scope and access
    /// always failed. Parse the installed SDK's CFURL.h and assert our FFI
    /// constants match Apple's authoritative values.
    #[test]
    fn constants_match_sdk_header() {
        let Some(header) = read_cfurl_header() else {
            eprintln!(
                "skipping constants_match_sdk_header: CFURL.h not found \
                 (xcrun/SDK unavailable in this environment)"
            );
            return;
        };

        let cases = [
            (
                "kCFURLBookmarkCreationWithSecurityScope",
                K_CFURL_BOOKMARK_CREATION_WITH_SECURITY_SCOPE,
            ),
            (
                "kCFURLBookmarkResolutionWithSecurityScope",
                K_CFURL_BOOKMARK_RESOLUTION_WITH_SECURITY_SCOPE,
            ),
            (
                "kCFURLBookmarkResolutionWithoutUIMask",
                K_CFURL_BOOKMARK_RESOLUTION_WITHOUT_UI_MODAL_PROMPTS,
            ),
        ];

        for (name, ours) in cases {
            let sdk = sdk_value_for(&header, name).unwrap_or_else(|| {
                panic!("could not find/parse `{name}` in CFURL.h")
            });
            assert_eq!(
                ours, sdk,
                "{name}: our FFI constant is {ours} but the SDK header says {sdk} \
                 — fix the constant to match Apple's CFURL.h"
            );
        }
    }

    fn read_cfurl_header() -> Option<String> {
        let sdk_path = std::process::Command::new("xcrun")
            .args(["--show-sdk-path"])
            .output()
            .ok()
            .filter(|o| o.status.success())?;
        let sdk = String::from_utf8(sdk_path.stdout).ok()?;
        let path = Path::new(sdk.trim())
            .join("System/Library/Frameworks/CoreFoundation.framework/Headers/CFURL.h");
        std::fs::read_to_string(path).ok()
    }

    /// Find the enum line for `name` and evaluate its `( 1UL << N )` / `( 1 << N )`
    /// / decimal value, ignoring the trailing `// comment` and API_AVAILABLE cruft.
    fn sdk_value_for(header: &str, name: &str) -> Option<u32> {
        let line = header
            .lines()
            .find(|l| l.trim_start().starts_with(name) && l.contains('='))?;
        // RHS after the first '=', with any line comment stripped.
        let rhs = line.split_once('=')?.1;
        let rhs = rhs.split("//").next().unwrap_or(rhs);
        // Prefer the content inside the first (...) group, e.g. "( 1UL << 11 )".
        let inner = match (rhs.find('('), rhs.find(')')) {
            (Some(a), Some(b)) if b > a => &rhs[a + 1..b],
            _ => rhs,
        };
        let inner = inner.trim().trim_end_matches(',').trim();
        if let Some((lhs, shift)) = inner.split_once("<<") {
            let base: u32 = lhs.trim().trim_end_matches(['U', 'L']).trim().parse().ok()?;
            let shift: u32 = shift.trim().trim_end_matches(['U', 'L']).trim().parse().ok()?;
            Some(base << shift)
        } else {
            inner.trim_end_matches(['U', 'L']).trim().parse().ok()
        }
    }
}
