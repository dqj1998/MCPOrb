use std::fs;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// tauri-utils `external_binaries()` rewrites every `externalBin` path to
/// `{path}-{target_triple}{.exe on Windows}`; cargo never produces those
/// suffixed files, and `tauri_build::build()` aborts if they are missing.
/// Sync them from the plain cargo outputs, keyed off the same `TARGET` env
/// var tauri-build reads, so the naming matches on every platform.
fn sync_external_binaries() {
    let target_triple = std::env::var("TARGET").expect("TARGET env var not set");
    let ext = if target_triple.contains("windows") {
        ".exe"
    } else {
        ""
    };
    for name in ["mcporb-runtime", "mcporb-gateway-stdio", "mcporb-gateway-http"] {
        let src = format!("../../target/release/{name}{ext}");
        let dst = format!("../../target/release/{name}-{target_triple}{ext}");
        println!("cargo:rerun-if-changed={src}");
        let bytes = fs::read(&src).unwrap_or_else(|e| {
            panic!(
                "external binary not found at {src} ({e}); build it first with `cargo build --release -p {name}`"
            )
        });
        let in_sync = fs::read(&dst).map(|d| d == bytes).unwrap_or(false);
        if !in_sync {
            fs::copy(&src, &dst)
                .unwrap_or_else(|e| panic!("failed to write external binary {dst}: {e}"));
        }
        // The suffixed copy is what tauri-bundler puts into the .app, and the
        // macOS bundler propagates its mode as-is; a 644 sidecar would not
        // launch. Force +x regardless of the source's mode.
        #[cfg(unix)]
        fs::set_permissions(&dst, fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|e| panic!("failed to chmod +x {dst}: {e}"));
    }
}

fn main() {
    let source = if cfg!(feature = "direct-download") {
        "entitlements-direct.plist"
    } else {
        "entitlements-mas.plist"
    };

    let target = "entitlements.plist";
    let needs_copy = match fs::read_to_string(target) {
        Ok(existing) => match fs::read_to_string(source) {
            Ok(desired) => existing != desired,
            Err(_) => true,
        },
        Err(_) => true,
    };

    if needs_copy {
        fs::copy(source, target).expect("failed to copy entitlements");
    }

    sync_external_binaries();

    tauri_build::build()
}
