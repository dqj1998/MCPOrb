use mcporb_core::builder::{build_orb, OrbBuildConfig};
use mcporb_core::chunker::ChunkerConfig;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_markdown_ingestion_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/synthetic.md");

    let result = build_orb(
        &fixture,
        OrbBuildConfig {
            name: "test-orb".to_string(),
            description: "Test".to_string(),
            output_dir: tmp.path().to_path_buf(),
            chunker: ChunkerConfig::default(),
        },
    )
    .expect("build should succeed");

    assert!(!result.chunks.is_empty(), "should produce chunks");
    assert!(result.manifest.chunk_count > 0);

    // Verify roundtrip: deserialize chunks.postcard
    let bytes = std::fs::read(tmp.path().join("chunks.postcard")).unwrap();
    let chunks: Vec<mcporb_core::Chunk> = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(
        chunks.len(),
        result.chunks.len(),
        "postcard roundtrip must match"
    );

    // Verify manifest.json exists and parses
    let manifest_json =
        std::fs::read_to_string(tmp.path().join("orb_manifest.json")).unwrap();
    let manifest: mcporb_core::OrbManifest = serde_json::from_str(&manifest_json).unwrap();
    assert_eq!(manifest.name, "test-orb");
    assert_eq!(manifest.chunk_count, result.chunks.len());
}
