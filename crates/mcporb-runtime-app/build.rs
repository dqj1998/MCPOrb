use std::fs;

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

    tauri_build::build()
}
