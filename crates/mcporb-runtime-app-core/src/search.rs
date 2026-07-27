use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read, Seek};
use std::path::Path;

use anyhow::{Context, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use mcporb_runtime_core::format::Capability;
use mcporb_runtime_core::{
    Bm25Index, Chunk, DenseRuntime, Document, FlatVectorIndex, HnswIndex, OrbManifest,
    SearchMethodRequest, SearchRequest, SearchRuntime, TfIdfIndex, TrigramIndex,
};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const AUTH_KEY_INFO: &[u8] = b"mcporb-auth-key-v1";
const ASSET_KEY_INFO: &[u8] = b"mcporb-asset-key-v1";
const AUTH_VERIFIER_MSG: &[u8] = b"mcporb-auth-v1";
const DEVICE_UNLOCK_SERVICE: &str = "com.mcporb.orb-unlock";

#[derive(Debug, Clone, Deserialize)]
struct SecurityFile {
    #[serde(default)]
    access_password: Option<AccessPasswordFile>,
    #[serde(default)]
    asset_encryption: Option<AssetEncryptionFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct AccessPasswordFile {
    enabled: bool,
    kdf: String,
    kdf_params: KdfParamsFile,
    salt_b64: String,
    #[serde(default)]
    auth_verifier_b64: Option<String>,
    orb_identity_b64: String,
}

#[derive(Debug, Clone, Deserialize)]
struct KdfParamsFile {
    m_cost_kib: u32,
    t_cost: u32,
    p_cost: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct AssetEncryptionFile {
    enabled: bool,
    algorithm: String,
    payload: String,
    nonce_b64: String,
    aad: String,
}

#[derive(Debug, Clone, Copy)]
struct DerivedKeys {
    auth_key: [u8; 32],
    asset_key: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub chunk_id: u32,
    pub score: f32,
    pub method: String,
    pub text: String,
    pub document_title: String,
    pub source_path: String,
    pub page: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub orb_name: String,
    pub active_plan: String,
    pub hits: Vec<SearchHit>,
}

struct LoadedKnowledge {
    manifest: OrbManifest,
    documents: Vec<Document>,
    chunks: Vec<Chunk>,
    search: SearchRuntime,
}

pub fn search_zip(
    zip_path: &Path,
    query: &str,
    method: Option<&str>,
    top_k: Option<usize>,
) -> Result<SearchResponse> {
    search_zip_internal(zip_path, query, method, top_k, None)
}

pub fn search_zip_with_password(
    zip_path: &Path,
    query: &str,
    method: Option<&str>,
    top_k: Option<usize>,
    password: &str,
) -> Result<SearchResponse> {
    search_zip_internal(zip_path, query, method, top_k, Some(password))
}

fn search_zip_internal(
    zip_path: &Path,
    query: &str,
    method: Option<&str>,
    top_k: Option<usize>,
    password: Option<&str>,
) -> Result<SearchResponse> {
    let query = query.trim();
    if query.is_empty() {
        anyhow::bail!("query cannot be empty");
    }
    let knowledge = load_knowledge(zip_path, password)?;
    let response = knowledge.search.search(&SearchRequest {
        query: query.to_string(),
        top_k: top_k.unwrap_or(8).clamp(1, 50),
        method: method
            .map(SearchMethodRequest::from_str)
            .unwrap_or(SearchMethodRequest::Auto),
        query_vector: None,
        explain: false,
    })?;

    let documents = knowledge
        .documents
        .iter()
        .map(|doc| (doc.id, doc))
        .collect::<HashMap<_, _>>();
    let chunks = knowledge
        .chunks
        .iter()
        .map(|chunk| (chunk.id, chunk))
        .collect::<HashMap<_, _>>();

    let hits = response
        .hits
        .into_iter()
        .filter_map(|hit| {
            let chunk = chunks.get(&hit.chunk_id)?;
            let doc = documents.get(&chunk.document_id)?;
            Some(SearchHit {
                chunk_id: hit.chunk_id,
                score: hit.score,
                method: hit.method.to_string(),
                text: chunk.text.clone(),
                document_title: doc.title.clone(),
                source_path: doc.source_path.clone(),
                page: chunk.page,
            })
        })
        .collect();

    Ok(SearchResponse {
        orb_name: knowledge
            .manifest
            .display_name
            .clone()
            .unwrap_or_else(|| knowledge.manifest.name.clone()),
        active_plan: response.active_plan.to_string(),
        hits,
    })
}

fn load_knowledge(zip_path: &Path, password: Option<&str>) -> Result<LoadedKnowledge> {
    let bytes = fs::read(zip_path).with_context(|| format!("read {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    if archive.by_name("orb_assets.enc").is_ok() {
        let security = read_security_file(&mut archive)?;
        let access_password = security
            .access_password
            .ok_or_else(|| anyhow::anyhow!("password required to search this Orb"))?;
        if !access_password.enabled {
            anyhow::bail!("password required to search this Orb");
        }
        let keys = resolve_derived_keys(&access_password, password)?;
        if let Some(verifier_b64) = access_password.auth_verifier_b64.as_deref() {
            let expected = B64.decode(verifier_b64).context("decode auth verifier")?;
            if !verify_auth(&keys.auth_key, &expected) {
                anyhow::bail!("Invalid password");
            }
        }
        let asset_encryption = security
            .asset_encryption
            .ok_or_else(|| anyhow::anyhow!("encrypted Orb is missing asset encryption metadata"))?;
        let encrypted_blob = read_bundle_asset(&mut archive, "orb_assets.enc")?;
        let zip_bytes = decrypt_asset_blob(&asset_encryption, &keys.asset_key, &encrypted_blob)?;
        let mut decrypted_archive = zip::ZipArchive::new(Cursor::new(zip_bytes))?;
        return read_knowledge_from_archive(&mut decrypted_archive);
    }

    // Non-encrypted password-protected orbs store knowledge in plaintext
    // in the ZIP archive; no password is needed to search them.

    read_knowledge_from_archive(&mut archive)
}

fn read_security_file<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> Result<SecurityFile> {
    let security_json = read_bundle_asset(archive, "orb_security.json")?;
    Ok(serde_json::from_slice(&security_json).context("parse orb_security.json")?)
}

fn resolve_derived_keys(access_password: &AccessPasswordFile, password: Option<&str>) -> Result<DerivedKeys> {
    if let Some(password) = password {
        return derive_keys(access_password, password);
    }

    // Try keychain — the master key was stored during import (remember_orb_password).
    if let Some(master_key) = recall_device_unlock(&access_password.orb_identity_b64) {
        return Ok(derive_from_master(master_key));
    }

    anyhow::bail!("password required to search this Orb")
}

fn derive_keys(access_password: &AccessPasswordFile, password: &str) -> Result<DerivedKeys> {
    if access_password.kdf != "argon2id" {
        anyhow::bail!("unsupported kdf: {}", access_password.kdf);
    }
    let salt = B64
        .decode(&access_password.salt_b64)
        .context("decode salt_b64")?;
    let params = Params::new(
        access_password.kdf_params.m_cost_kib,
        access_password.kdf_params.t_cost,
        access_password.kdf_params.p_cost,
        Some(32),
    )
    .map_err(|error| anyhow::anyhow!("argon2 params: {error}"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut master_key = [0u8; 32];
    argon
        .hash_password_into(password.as_bytes(), &salt, &mut master_key)
        .map_err(|error| anyhow::anyhow!("argon2 derive: {error}"))?;
    Ok(derive_from_master(master_key))
}

fn derive_from_master(master_key: [u8; 32]) -> DerivedKeys {
    let hk = Hkdf::<Sha256>::new(None, &master_key);
    let mut auth_key = [0u8; 32];
    hk.expand(AUTH_KEY_INFO, &mut auth_key).expect("hkdf auth leg");
    let mut asset_key = [0u8; 32];
    hk.expand(ASSET_KEY_INFO, &mut asset_key).expect("hkdf asset leg");
    DerivedKeys { auth_key, asset_key }
}

fn verify_auth(auth_key: &[u8; 32], expected: &[u8]) -> bool {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(auth_key).expect("hmac accepts any key length");
    mac.update(AUTH_VERIFIER_MSG);
    mac.verify_slice(expected).is_ok()
}

fn recall_device_unlock(orb_identity_b64: &str) -> Option<[u8; 32]> {
    let orb_identity = B64.decode(orb_identity_b64).ok()?;
    let entry = keyring::Entry::new(DEVICE_UNLOCK_SERVICE, &B64.encode(orb_identity)).ok()?;
    let secret = entry.get_secret().ok()?;
    if secret.len() != 32 {
        return None;
    }
    let mut master_key = [0u8; 32];
    master_key.copy_from_slice(&secret);
    Some(master_key)
}

fn decrypt_asset_blob(
    cfg: &AssetEncryptionFile,
    asset_key: &[u8; 32],
    ciphertext: &[u8],
) -> Result<Vec<u8>> {
    if !cfg.enabled {
        anyhow::bail!("asset encryption disabled");
    }
    if cfg.algorithm != "xchacha20poly1305" {
        anyhow::bail!("unsupported algorithm: {}", cfg.algorithm);
    }
    if cfg.payload != "orb_assets.enc" {
        anyhow::bail!("unsupported payload: {}", cfg.payload);
    }
    let nonce = B64.decode(&cfg.nonce_b64).context("decode nonce_b64")?;
    if nonce.len() != 24 {
        anyhow::bail!("xchacha20poly1305 nonce must be 24 bytes, got {}", nonce.len());
    }
    let cipher = XChaCha20Poly1305::new(Key::from_slice(asset_key));
    let nonce = XNonce::from_slice(&nonce);
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad: cfg.aad.as_bytes(),
            },
        )
        .map_err(|_| anyhow::anyhow!("Invalid password"))
}

fn read_knowledge_from_archive<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> Result<LoadedKnowledge> {
    let manifest_json = read_bundle_asset(archive, "orb_manifest.json")?;
    let documents_bytes = read_bundle_asset(archive, "documents.postcard")?;
    let chunks_bytes = read_bundle_asset(archive, "chunks.postcard")?;
    let bm25_bytes = read_bundle_asset(archive, "bm25_index.postcard")?;
    let tfidf_bytes = read_optional_bundle_asset(archive, "tfidf_index.postcard")?;
    let trigram_bytes = read_optional_bundle_asset(archive, "trigram_index.postcard")?;
    let vector_bytes = read_optional_bundle_asset(archive, "vector_store.postcard")?;
    let hnsw_bytes = read_optional_bundle_asset(archive, "hnsw_index.postcard")?;

    let manifest: OrbManifest = serde_json::from_slice(&manifest_json)?;
    let documents: Vec<Document> = postcard::from_bytes(&documents_bytes)?;
    let chunks: Vec<Chunk> = postcard::from_bytes(&chunks_bytes)?;
    let bm25: Bm25Index = postcard::from_bytes(&bm25_bytes)?;
    let tfidf = load_optional_index::<TfIdfIndex>(&manifest, Capability::TfIdf, tfidf_bytes.as_deref())?;
    let trigram = load_optional_index::<TrigramIndex>(&manifest, Capability::Trigram, trigram_bytes.as_deref())?;
    let vector = load_optional_index::<FlatVectorIndex>(&manifest, Capability::FlatVector, vector_bytes.as_deref())?;
    let hnsw = load_optional_index::<HnswIndex>(&manifest, Capability::Hnsw, hnsw_bytes.as_deref())?;

    Ok(LoadedKnowledge {
        search: SearchRuntime {
            bm25,
            tfidf,
            trigram,
            dense: DenseRuntime::from_assets(vector, hnsw)?,
            dense_tier: manifest.selected_retrieval_plan.clone(),
        },
        manifest,
        documents,
        chunks,
    })
}

fn read_bundle_asset<R: Read + Seek>(archive: &mut zip::ZipArchive<R>, name: &str) -> Result<Vec<u8>> {
    let mut file = archive.by_name(name)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn read_optional_bundle_asset<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> Result<Option<Vec<u8>>> {
    match archive.by_name(name) {
        Ok(mut file) => {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Ok(Some(bytes))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn load_optional_index<T>(
    manifest: &OrbManifest,
    capability: Capability,
    bytes: Option<&[u8]>,
) -> Result<Option<T>>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let capability_enabled = manifest
        .enabled_capabilities
        .iter()
        .any(|value| *value == capability);

    match (capability_enabled, bytes) {
        (true, Some(bytes)) => Ok(Some(postcard::from_bytes(bytes)?)),
        (true, None) => anyhow::bail!("missing asset for enabled capability {:?}", capability),
        (false, Some(bytes)) => Ok(Some(postcard::from_bytes(bytes)?)),
        (false, None) => Ok(None),
    }
}
