use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use mcporb_runtime_app_core::password::{inspect_orb_security, remember_orb_password, verify_orb_password};
use mcporb_runtime_app_core::search::search_zip;
use mcporb_runtime_app_core::zip_import::validate_zip_path;
use mcporb_runtime_core::format::{Capability, Chunk, Document, OrbManifest, RetrievalPlanKind};
use mcporb_runtime_core::build_bm25_index;
use sha2::Sha256;
use tempfile::tempdir;

const AUTH_KEY_INFO: &[u8] = b"mcporb-auth-key-v1";
const ASSET_KEY_INFO: &[u8] = b"mcporb-asset-key-v1";
const AUTH_VERIFIER_MSG: &[u8] = b"mcporb-auth-v1";

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
struct SecurityMaterial {
    security_json: String,
    asset_key: [u8; 32],
}

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
    hk.expand(ASSET_KEY_INFO, &mut asset_key).expect("hkdf asset");
    (auth_key, asset_key)
}

fn auth_verifier(auth_key: &[u8; 32]) -> Vec<u8> {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(auth_key).expect("hmac");
    mac.update(AUTH_VERIFIER_MSG);
    mac.finalize().into_bytes().to_vec()
}

fn make_security_json(
    password: &str,
    unlock_persistence: &str,
    include_asset_encryption: bool,
    orb_identity_b64: &str,
) -> SecurityMaterial {
    let salt = b"0123456789abcdef";
    let (auth_key, asset_key) = derive_keys(password, salt);
    let verifier = auth_verifier(&auth_key);
    let nonce = [7u8; 24];

    let security_json = if include_asset_encryption {
        format!(
            "{{\"schema_version\":1,\"access_password\":{{\"enabled\":true,\"kdf\":\"argon2id\",\"kdf_params\":{{\"m_cost_kib\":32768,\"t_cost\":2,\"p_cost\":1}},\"salt_b64\":\"{}\",\"auth_verifier_b64\":\"{}\",\"unlock_persistence\":\"{}\",\"orb_identity_b64\":\"{}\"}},\"asset_encryption\":{{\"enabled\":true,\"algorithm\":\"xchacha20poly1305\",\"payload\":\"orb_assets.enc\",\"nonce_b64\":\"{}\",\"aad\":\"mcporb-orb-assets-v1\"}}}}",
            B64.encode(salt),
            B64.encode(verifier),
            unlock_persistence,
            orb_identity_b64,
            B64.encode(nonce),
        )
    } else {
        format!(
            "{{\"schema_version\":1,\"access_password\":{{\"enabled\":true,\"kdf\":\"argon2id\",\"kdf_params\":{{\"m_cost_kib\":32768,\"t_cost\":2,\"p_cost\":1}},\"salt_b64\":\"{}\",\"auth_verifier_b64\":\"{}\",\"unlock_persistence\":\"{}\",\"orb_identity_b64\":\"{}\"}}}}",
            B64.encode(salt),
            B64.encode(verifier),
            unlock_persistence,
            orb_identity_b64,
        )
    };

    SecurityMaterial {
        security_json,
        asset_key,
    }
}

fn make_knowledge_zip_bytes() -> Vec<u8> {
    let manifest = OrbManifest {
        name: "acceptance-orb".to_string(),
        display_name: Some("Acceptance Orb".to_string()),
        version: "0.1.0".to_string(),
        description: "password acceptance test orb".to_string(),
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
    };

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
    let mut zip = zip::ZipWriter::new(cursor);
    let opts = zip::write::SimpleFileOptions::default();

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

fn encrypt_inner_assets_zip(asset_key: &[u8; 32], inner_zip: &[u8]) -> Vec<u8> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(asset_key));
    let nonce = XNonce::from_slice(&[7u8; 24]);
    cipher
        .encrypt(
            nonce,
            Payload {
                msg: inner_zip,
                aad: b"mcporb-orb-assets-v1",
            },
        )
        .expect("encrypt orb_assets.enc")
}

fn write_orb_zip(path: &Path, unlock_persistence: &str, encrypted_assets: bool, password: &str, orb_identity_b64: &str) -> PathBuf {
    let security = make_security_json(password, unlock_persistence, encrypted_assets, orb_identity_b64);
    let knowledge = make_knowledge_zip_bytes();

    let cursor = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    let opts = zip::write::SimpleFileOptions::default();

    if encrypted_assets {
        zip.start_file("orb_security.json", opts).unwrap();
        zip.write_all(security.security_json.as_bytes()).unwrap();

        let encrypted_blob = encrypt_inner_assets_zip(&security.asset_key, &knowledge);
        zip.start_file("orb_assets.enc", opts).unwrap();
        zip.write_all(&encrypted_blob).unwrap();
    } else {
        let mut inner = zip::ZipArchive::new(Cursor::new(knowledge)).unwrap();
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
        zip.write_all(security.security_json.as_bytes()).unwrap();
    }

    let bytes = zip.finish().unwrap().into_inner();
    fs::write(path, bytes).unwrap();
    path.to_path_buf()
}

#[test]
fn detects_password_persistence_modes() {
    let dir = tempdir().unwrap();
    let every = write_orb_zip(&dir.path().join("every.zip"), "every_launch", false, "open-sesame", "AQIDBA==");
    let remember = write_orb_zip(
        &dir.path().join("remember.zip"),
        "remember_on_this_device",
        false,
        "open-sesame",
        "AQIEBA==",
    );

    let every_report = validate_zip_path(&every).unwrap();
    assert!(every_report.password_protected);
    assert_eq!(every_report.password_persistence.as_deref(), Some("every_launch"));

    let remember_report = validate_zip_path(&remember).unwrap();
    assert!(remember_report.password_protected);
    assert_eq!(
        remember_report.password_persistence.as_deref(),
        Some("remember_on_this_device")
    );

    let security = inspect_orb_security(&remember).unwrap();
    assert!(security.password_protected);
    assert_eq!(
        security.password_persistence.as_deref(),
        Some("remember_on_this_device")
    );
}

#[test]
fn verifies_password_for_every_launch_orb() {
    let dir = tempdir().unwrap();
    let orb = write_orb_zip(&dir.path().join("every.zip"), "every_launch", false, "open-sesame", "AQIEAQ==");

    assert!(!verify_orb_password(&orb, "wrong-pass").unwrap());
    assert!(verify_orb_password(&orb, "open-sesame").unwrap());
}

#[test]
fn remembers_password_for_every_launch_orb() {
    let dir = tempdir().unwrap();
    // A fresh identity per run so the device-unlock keyring entry is never
    // pre-populated by an earlier run of this test (the keychain persists).
    let identity = B64.encode(format!("every-{}", fresh_id()).as_bytes());
    let orb = write_orb_zip(&dir.path().join("every.zip"), "every_launch", false, "open-sesame", &identity);

    let info_after = inspect_orb_security(&orb).unwrap();
    assert!(!info_after.device_remembered);

    // Password is entered once at import and remembered on the device; the
    // legacy every-launch flag no longer blocks device unlock.
    remember_orb_password(&orb, "open-sesame").expect("remember should succeed");
    let info = inspect_orb_security(&orb).unwrap();
    assert!(info.device_remembered);
}

fn fresh_id() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

#[test]
fn encrypted_search_fails_without_password() {
    let dir = tempdir().unwrap();
    let orb = write_orb_zip(
        &dir.path().join("enc.zip"),
        "every_launch",
        true,
        "open-sesame",
        "AQICBQ==",
    );

    let locked = search_zip(&orb, "guarded", Some("bm25"), Some(3));
    assert!(locked.is_err());
    assert!(locked
        .unwrap_err()
        .to_string()
        .contains("password required to search this Orb"));
}
