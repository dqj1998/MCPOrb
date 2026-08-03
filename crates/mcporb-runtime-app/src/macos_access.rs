// macOS App Sandbox security-scoped bookmark helpers.
//
// Sandboxed apps lose access to user-selected folders after relaunch; a
// security-scoped bookmark is the only way to regain it. The public
// CoreFoundation C API (CFURL.h) exposes everything needed, so no third-party
// dependency is required. This module is only compiled on macOS (the `mod`
// declaration in main.rs is gated with #[cfg(target_os = "macos")]).

use std::ffi::{c_void, CStr, CString};
use std::os::raw::c_char;
use std::path::{Path, PathBuf};

use base64::Engine;

const K_CFURL_POSIX_PATH_STYLE: u32 = 0;
const K_CFSTRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_CFURL_BOOKMARK_CREATION_WITH_SECURITY_SCOPE: u32 = 1 << 0;
const K_CFURL_BOOKMARK_RESOLUTION_WITH_SECURITY_SCOPE: u32 = 1 << 0;

#[repr(C)]
struct __CFURL(c_void);
#[repr(C)]
struct __CFData(c_void);

type CFURLRef = *const __CFURL;
type CFDataRef = *const __CFData;
type CFAllocatorRef = *const c_void;
type CFIndex = isize;
type Boolean = u8;
type CFStringRef = *const c_void;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFURLCreateWithFileSystemPath(
        allocator: CFAllocatorRef,
        file_path: CFStringRef,
        path_style: u32,
        is_directory: Boolean,
    ) -> CFURLRef;
    fn CFURLGetFileSystemRepresentation(
        url: CFURLRef,
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
    fn CFStringCreateWithCString(
        allocator: CFAllocatorRef,
        c_str: *const c_char,
        encoding: u32,
    ) -> CFStringRef;
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

impl Drop for AccessGuard {
    fn drop(&mut self) {
        unsafe {
            CFURLStopAccessingSecurityScopedResource(self.url);
            CFRelease(self.url.cast());
        }
    }
}

/// Creates a security-scoped bookmark for `path` (the app must currently hold
/// access to it, e.g. from a folder-open dialog) and returns it base64-encoded
/// for persistence.
pub fn create_bookmark(path: &Path) -> Result<String, String> {
    unsafe {
        let url = url_from_path(path)?;
        let bookmark = CFURLCreateBookmarkData(
            std::ptr::null(),
            url,
            K_CFURL_BOOKMARK_CREATION_WITH_SECURITY_SCOPE,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
        );
        CFRelease(url.cast());
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

unsafe fn url_from_path(path: &Path) -> Result<CFURLRef, String> {
    let c_path = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| "path contains a NUL byte".to_string())?;
    let cf_string =
        CFStringCreateWithCString(std::ptr::null(), c_path.as_ptr(), K_CFSTRING_ENCODING_UTF8);
    if cf_string.is_null() {
        return Err("CFStringCreateWithCString failed".to_string());
    }
    let url = CFURLCreateWithFileSystemPath(
        std::ptr::null(),
        cf_string,
        K_CFURL_POSIX_PATH_STYLE,
        1,
    );
    CFRelease(cf_string.cast());
    if url.is_null() {
        return Err("CFURLCreateWithFileSystemPath failed".to_string());
    }
    Ok(url)
}

unsafe fn path_from_url(url: CFURLRef) -> Result<PathBuf, String> {
    let mut buffer = [0 as c_char; 4096];
    if CFURLGetFileSystemRepresentation(url, buffer.as_mut_ptr(), buffer.len() as CFIndex) == 0 {
        return Err("CFURLGetFileSystemRepresentation failed".to_string());
    }
    let c_str = CStr::from_ptr(buffer.as_ptr());
    Ok(PathBuf::from(c_str.to_string_lossy().into_owned()))
}
