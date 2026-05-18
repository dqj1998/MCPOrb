use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=MCPORB_EMBED_ASSETS_DIR");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set"));
    let dest = out_dir.join("embedded_orb.rs");

    let Some(assets_dir) = env::var_os("MCPORB_EMBED_ASSETS_DIR") else {
        write_stub(&dest);
        return;
    };

    let assets_dir = PathBuf::from(assets_dir);
    let manifest = assets_dir.join("orb_manifest.json");
    let documents = assets_dir.join("documents.postcard");
    let chunks = assets_dir.join("chunks.postcard");
    let index = assets_dir.join("bm25_index.postcard");

    for path in [&manifest, &documents, &chunks, &index] {
        if !path.exists() {
            panic!("missing embedded orb asset: {}", path.display());
        }
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let source = format!(
        "pub const HAS_EMBEDDED_ORB: bool = true;\n\
pub const EMBEDDED_MANIFEST_JSON: &[u8] = include_bytes!({manifest});\n\
pub const EMBEDDED_DOCUMENTS: &[u8] = include_bytes!({documents});\n\
pub const EMBEDDED_CHUNKS: &[u8] = include_bytes!({chunks});\n\
pub const EMBEDDED_INDEX: &[u8] = include_bytes!({index});\n",
        manifest = quoted_path(&manifest),
        documents = quoted_path(&documents),
        chunks = quoted_path(&chunks),
        index = quoted_path(&index),
    );

    fs::write(dest, source).expect("failed to write embedded_orb.rs");
}

fn write_stub(dest: &Path) {
    let source = "pub const HAS_EMBEDDED_ORB: bool = false;\n\
pub const EMBEDDED_MANIFEST_JSON: &[u8] = &[];\n\
pub const EMBEDDED_DOCUMENTS: &[u8] = &[];\n\
pub const EMBEDDED_CHUNKS: &[u8] = &[];\n\
pub const EMBEDDED_INDEX: &[u8] = &[];\n";
    fs::write(dest, source).expect("failed to write embedded_orb.rs");
}

fn quoted_path(path: &Path) -> String {
    format!("{:?}", path.to_string_lossy())
}