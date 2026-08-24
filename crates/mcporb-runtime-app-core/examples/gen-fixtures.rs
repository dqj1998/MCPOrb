//! Generates the versioned E2E fixtures under `UI-test/fixtures/v1/`.
//!
//! This mirrors `tests/password_acceptance.rs` so the produced Orb zips are
//! byte-for-byte valid against the real `validate_zip_path` / `build_orb` /
//! `verify_orb_password` code paths. Run with:
//!
//! ```sh
//! cargo run -p mcporb-runtime-app-core --example gen-fixtures
//! ```
//!
//! Fixtures produced:
//!   * plaintext-orb.zip   — valid, unprotected (`acceptance-orb`)
//!   * protected-orb.zip   — valid, password `test-orb-password`, `every_launch`
//!   * invalid-orb.zip     — manifest only, missing required postcards
//!   * traversal-orb.zip    — entry `../traversal.txt` (must be rejected)

use std::fs;
use std::io::{Cursor, Write};
use std::path::PathBuf;

use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use mcporb_runtime_app_core::password::verify_orb_password;
use mcporb_runtime_app_core::zip_import::validate_zip_path;
use mcporb_runtime_core::build_bm25_index;
use mcporb_runtime_core::format::{Capability, Chunk, Document, OrbManifest, RetrievalPlanKind};
use sha2::Sha256;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

const AUTH_KEY_INFO: &[u8] = b"mcporb-auth-key-v1";
const AUTH_VERIFIER_MSG: &[u8] = b"mcporb-auth-v1";
type HmacSha256 = Hmac<Sha256>;

fn derive_keys(password: &str, salt: &[u8]) -> ([u8; 32], [u8; 32]) {
    let params = Params::new(32 * 1024, 2, 1, Some(32)).expect("argon2 params");
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut master_key = [0u8; 32];
    argon
        .hash_password_into(password.as_bytes(), salt, &mut master_key)
        .expect("derive master key");
    let hk = Hkdf::<Sha256>::new(None, &master_key);
    let mut auth_key = [0u8; 32];
    hk.expand(AUTH_KEY_INFO, &mut auth_key).expect("hkdf auth");
    let mut asset_key = [0u8; 32];
    hk.expand(b"mcporb-asset-key-v1", &mut asset_key).expect("hkdf asset");
    (auth_key, asset_key)
}

fn auth_verifier(auth_key: &[u8; 32]) -> Vec<u8> {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(auth_key).expect("hmac");
    mac.update(AUTH_VERIFIER_MSG);
    mac.finalize().into_bytes().to_vec()
}

fn make_security_json(password: &str, persistence: &str, orb_identity_b64: &str) -> String {
    let salt = b"0123456789abcdef";
    let (auth_key, _asset_key) = derive_keys(password, salt);
    let verifier = auth_verifier(&auth_key);
    format!(
        "{{\"schema_version\":1,\"access_password\":{{\"enabled\":true,\"kdf\":\"argon2id\",\"kdf_params\":{{\"m_cost_kib\":32768,\"t_cost\":2,\"p_cost\":1}},\"salt_b64\":\"{}\",\"auth_verifier_b64\":\"{}\",\"unlock_persistence\":\"{}\",\"orb_identity_b64\":\"{}\"}}}}",
        B64.encode(salt),
        B64.encode(verifier),
        persistence,
        orb_identity_b64,
    )
}

fn build_manifest(name: &str) -> OrbManifest {
    OrbManifest {
        name: name.to_string(),
        display_name: Some(format!("{name} fixture")),
        version: "0.1.0".to_string(),
        description: "E2E fixture orb".to_string(),
        orb_format_version: "0.2".to_string(),
        runtime_min_version: None,
        builder_version: None,
        mcp_protocol_version: "2024-11-05".to_string(),
        build_time: "2026-07-24T00:00:00Z".to_string(),
        created_at: None,
        source_documents: vec!["doc.md".to_string()],
        chunk_count: 1,
        index_format_version: "0.2".to_string(),
        binary_size_target_mb: 20,
        assets_sha256: None,
        encrypted: false,
        selected_retrieval_plan: RetrievalPlanKind::Bm25Only,
        enabled_capabilities: vec![Capability::Bm25],
        embedding_dim: None,
        embedding_model: None,
        embedding_model_tar_sha256: None,
        trigram_min_df: None,
        planning_rationale: vec![],
    }
}

fn make_knowledge_zip_bytes(name: &str) -> Vec<u8> {
    let manifest = build_manifest(name);

    let documents = vec![Document {
        id: 0,
        title: "Doc".to_string(),
        source_path: "doc.md".to_string(),
        page_count: Some(1),
        sections: vec![],
    }];

    let chunks = vec![Chunk {
        id: 0,
        document_id: 0,
        section_id: None,
        page: Some(1),
        text: "alpha bravo guarded content".to_string(),
        token_count: 4,
    }];

    let bm25 = build_bm25_index(&chunks);

    let cursor = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(cursor);
    let opts = SimpleFileOptions::default();

    zip.start_file("orb_manifest.json", opts).unwrap();
    zip.write_all(&serde_json::to_vec(&manifest).unwrap()).unwrap();
    zip.start_file("documents.postcard", opts).unwrap();
    zip.write_all(&postcard::to_allocvec(&documents).unwrap()).unwrap();
    zip.start_file("chunks.postcard", opts).unwrap();
    zip.write_all(&postcard::to_allocvec(&chunks).unwrap()).unwrap();
    zip.start_file("bm25_index.postcard", opts).unwrap();
    zip.write_all(&postcard::to_allocvec(&bm25).unwrap()).unwrap();

    zip.finish().unwrap().into_inner()
}

fn write_bytes(path: &std::path::Path, bytes: &[u8]) {
    fs::write(path, bytes).expect("write fixture");
}

fn main() {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../UI-test/fixtures/v1");
    fs::create_dir_all(&out_dir).expect("create fixtures dir");

    // 1) plaintext orb
    let plaintext_path = out_dir.join("plaintext-orb.zip");
    write_bytes(&plaintext_path, &make_knowledge_zip_bytes("acceptance-orb"));
    assert!(
        validate_zip_path(&plaintext_path).is_ok(),
        "plaintext orb must validate"
    );

    // 2) protected orb (password: test-orb-password, every_launch)
    let protected_path = out_dir.join("protected-orb.zip");
    {
        let security = make_security_json("test-orb-password", "every_launch", "AQIDBA==");
        let knowledge = make_knowledge_zip_bytes("protected-orb");
        let cursor = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(cursor);
        let opts = SimpleFileOptions::default();
        let mut inner = zip::ZipArchive::new(Cursor::new(&knowledge)).unwrap();
        for name in [
            "orb_manifest.json",
            "documents.postcard",
            "chunks.postcard",
            "bm25_index.postcard",
        ] {
            let mut f = inner.by_name(name).unwrap();
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut f, &mut bytes).unwrap();
            zip.start_file(name, opts).unwrap();
            zip.write_all(&bytes).unwrap();
        }
        zip.start_file("orb_security.json", opts).unwrap();
        zip.write_all(security.as_bytes()).unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        write_bytes(&protected_path, &bytes);
    }
    assert!(
        validate_zip_path(&protected_path).is_ok(),
        "protected orb must validate"
    );
    assert!(
        verify_orb_password(&protected_path, "test-orb-password").unwrap(),
        "correct password must verify"
    );
    assert!(
        !verify_orb_password(&protected_path, "wrong").unwrap(),
        "wrong password must fail"
    );

    // 3) invalid orb (manifest only, missing required postcards)
    let invalid_path = out_dir.join("invalid-orb.zip");
    {
        let cursor = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(cursor);
        let opts = SimpleFileOptions::default();
        zip.start_file("orb_manifest.json", opts).unwrap();
        zip.write_all(&serde_json::to_vec(&build_manifest("bad")).unwrap())
            .unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        write_bytes(&invalid_path, &bytes);
    }
    assert!(
        validate_zip_path(&invalid_path).is_err(),
        "invalid orb must fail validation"
    );

    // 4) traversal orb
    let traversal_path = out_dir.join("traversal-orb.zip");
    {
        let cursor = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(cursor);
        let opts = SimpleFileOptions::default();
        zip.start_file("../traversal.txt", opts).unwrap();
        zip.write_all(b"hacked").unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        write_bytes(&traversal_path, &bytes);
    }
    assert!(
        validate_zip_path(&traversal_path).is_err(),
        "traversal entry must be rejected"
    );

    println!("fixtures generated at {:?}", out_dir);
}
