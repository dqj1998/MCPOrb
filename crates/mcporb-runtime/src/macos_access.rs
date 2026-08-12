// macOS App Sandbox security-scoped bookmark helpers.
//
// Sandboxed apps lose access to user-selected folders after relaunch; a
// security-scoped bookmark is the only way to regain it. The public
// CoreFoundation C API (CFURL.h) exposes everything needed, so no third-party
// dependency is required. This module is only compiled on macOS (the `mod`
// declaration in main.rs is gated with #[cfg(target_os = "macos")]).

use std::ffi::{c_void, CStr};
use std::os::raw::c_char;
use std::path::PathBuf;

use base64::Engine;

const K_CFURL_BOOKMARK_CREATION_WITH_SECURITY_SCOPE: u32 = 1 << 11; // kCFURLBookmarkCreationWithSecurityScope = 2048
const K_CFURL_BOOKMARK_RESOLUTION_WITH_SECURITY_SCOPE: u32 = 1 << 11; // kCFURLBookmarkResolutionWithSecurityScope = 2048

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
#[allow(dead_code)] // mirrored from mcporb-runtime-app; runtime only resolves
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

/// Resolves a persisted base64 bookmark back to a folder path and starts
/// security-scoped access to it. Returns the path and a guard that must stay
/// alive while the folder is in use.
pub fn resolve_bookmark(encoded: &str) -> Result<(PathBuf, AccessGuard), String> {
    unsafe {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| format!("invalid bookmark data: {e}"))?;
        let bookmark = CFDataCreate(std::ptr::null(), bytes.as_ptr(), bytes.len() as CFIndex);
        if bookmark.is_null() {
            return Err("CFDataCreate failed".to_string());
        }
        let mut is_stale: Boolean = 0;
        let url = CFURLCreateByResolvingBookmarkData(
            std::ptr::null(),
            bookmark,
            K_CFURL_BOOKMARK_RESOLUTION_WITH_SECURITY_SCOPE,
            std::ptr::null(),
            std::ptr::null(),
            &mut is_stale,
            std::ptr::null_mut(),
        );
        CFRelease(bookmark.cast());
        if url.is_null() {
            return Err(
                "bookmark could not be resolved (folder may have been moved or deleted)".to_string(),
            );
        }
        if is_stale != 0 {
            tracing::warn!("resolved security-scoped bookmark is stale");
        }
        let path = path_from_url(url)?;
        if CFURLStartAccessingSecurityScopedResource(url) == 0 {
            CFRelease(url.cast());
            return Err("startAccessingSecurityScopedResource failed".to_string());
        }
        Ok((path, AccessGuard { url }))
    }
}

unsafe fn path_from_url(url: CFURLRef) -> Result<PathBuf, String> {
    let mut buffer = [0 as c_char; 4096];
    if CFURLGetFileSystemRepresentation(url, 1, buffer.as_mut_ptr(), buffer.len() as CFIndex) == 0 {
        return Err("CFURLGetFileSystemRepresentation failed".to_string());
    }
    let c_str = CStr::from_ptr(buffer.as_ptr());
    Ok(PathBuf::from(c_str.to_string_lossy().into_owned()))
}
