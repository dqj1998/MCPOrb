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
use serde::Deserialize;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const AUTH_KEY_INFO: &[u8] = b"mcporb-auth-key-v1";
const ASSET_KEY_INFO: &[u8] = b"mcporb-asset-key-v1";
const AUTH_VERIFIER_MSG: &[u8] = b"mcporb-auth-v1";
const DEVICE_UNLOCK_SERVICE: &str = "com.mcporb.orb-unlock";

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct SecurityFile {
    #[allow(dead_code)]
    schema_version: u32,
    #[serde(default)]
    access_password: Option<AccessPasswordFile>,
    #[serde(default)]
    asset_encryption: Option<AssetEncryptionFile>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct AccessPasswordFile {
    enabled: bool,
    kdf: String,
    kdf_params: KdfParamsFile,
    salt_b64: String,
    #[serde(default)]
    auth_verifier_b64: Option<String>,
    #[serde(default)]
    unlock_persistence: Option<String>,
    orb_identity_b64: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct KdfParamsFile {
    m_cost_kib: u32,
    t_cost: u32,
    p_cost: u32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct AssetEncryptionFile {
    enabled: bool,
    algorithm: String,
    payload: String,
    nonce_b64: String,
    aad: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct OrbSecurityInfo {
    pub password_protected: bool,
    pub password_persistence: Option<String>,
    pub encrypted_assets: bool,
    pub device_remembered: bool,
}

#[derive(Debug, Clone, Copy)]
struct DerivedKeys {
    master_key: [u8; 32],
    auth_key: [u8; 32],
    asset_key: [u8; 32],
}

pub fn inspect_orb_security(zip_path: &Path) -> Result<OrbSecurityInfo> {
    let bytes = fs::read(zip_path).with_context(|| format!("read Orb ZIP {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    inspect_orb_security_archive(&mut archive)
}

pub fn verify_orb_password(zip_path: &Path, password: &str) -> Result<bool> {
    let bytes = fs::read(zip_path).with_context(|| format!("read Orb ZIP {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    verify_orb_password_archive(&mut archive, password)
}

pub fn remember_orb_password(zip_path: &Path, password: &str) -> Result<()> {
    let bytes = fs::read(zip_path).with_context(|| format!("read Orb ZIP {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    remember_orb_password_archive(&mut archive, password)
}

fn inspect_orb_security_archive<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> Result<OrbSecurityInfo> {
    let mut names = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        names.push(file.name().to_string());
    }
    let encrypted_assets = names.iter().any(|name| name == "orb_assets.enc");
    let (password_protected, password_persistence, device_remembered) = if names.iter().any(|name| name == "orb_security.json") {
        let security_json = read_file(archive, "orb_security.json")?;
        let security: SecurityFile = serde_json::from_slice(&security_json)
            .context("parse orb_security.json")?;
        match security.access_password {
            Some(access_password) if access_password.enabled => (
                true,
                access_password.unlock_persistence,
                device_unlock_remembered(&access_password.orb_identity_b64),
            ),
            _ => (false, None, false),
        }
    } else {
        (false, None, false)
    };

    Ok(OrbSecurityInfo {
        password_protected,
        password_persistence,
        encrypted_assets,
        device_remembered,
    })
}

fn verify_orb_password_archive<R: Read + Seek>(archive: &mut zip::ZipArchive<R>, password: &str) -> Result<bool> {
    let security_json = match read_file(archive, "orb_security.json") {
        Ok(bytes) => bytes,
        Err(_) => return Ok(false),
    };
    let security: SecurityFile = serde_json::from_slice(&security_json)
        .context("parse orb_security.json")?;
    let access_password = match security.access_password {
        Some(access_password) if access_password.enabled => access_password,
        _ => return Ok(false),
    };
    if access_password.kdf != "argon2id" {
        return Ok(false);
    }
    let keys = derive_keys(&access_password, password)?;

    if let Some(expected) = access_password.auth_verifier_b64.as_deref() {
        let expected = B64.decode(expected).context("decode auth verifier")?;
        return Ok(verify_auth(&keys.auth_key, &expected));
    }

    let asset_encryption = match security.asset_encryption {
        Some(asset_encryption) if asset_encryption.enabled => asset_encryption,
        _ => return Ok(false),
    };
    let ciphertext = match read_file(archive, "orb_assets.enc") {
        Ok(bytes) => bytes,
        Err(_) => return Ok(false),
    };
    Ok(decrypt_asset_blob(&asset_encryption, &keys.asset_key, &ciphertext).is_ok())
}

fn remember_orb_password_archive<R: Read + Seek>(archive: &mut zip::ZipArchive<R>, password: &str) -> Result<()> {
    let security_json = read_file(archive, "orb_security.json")?;
    let security: SecurityFile = serde_json::from_slice(&security_json)
        .context("parse orb_security.json")?;
    let access_password = match security.access_password {
        Some(access_password) if access_password.enabled => access_password,
        _ => anyhow::bail!("Orb is not password-protected"),
    };

    let keys = derive_keys(&access_password, password)?;

    let verified = if let Some(expected) = access_password.auth_verifier_b64.as_deref() {
        let expected = B64.decode(expected).context("decode auth verifier")?;
        verify_auth(&keys.auth_key, &expected)
    } else if let Some(asset_encryption) = security.asset_encryption.as_ref() {
        let ciphertext = read_file(archive, "orb_assets.enc")?;
        decrypt_asset_blob(asset_encryption, &keys.asset_key, &ciphertext).is_ok()
    } else {
        false
    };

    if !verified {
        anyhow::bail!("Invalid password");
    }

    let orb_identity = B64
        .decode(&access_password.orb_identity_b64)
        .context("decode orb_identity_b64")?;
    let entry = keyring::Entry::new(DEVICE_UNLOCK_SERVICE, &B64.encode(orb_identity))
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    entry
        .set_secret(&keys.master_key)
        .map_err(|error| anyhow::anyhow!(error.to_string()))
}

fn derive_keys(access_password: &AccessPasswordFile, password: &str) -> Result<DerivedKeys> {
    let salt = B64
        .decode(&access_password.salt_b64)
        .context("decode orb_security.json salt")?;
    let params = Params::new(
        access_password.kdf_params.m_cost_kib,
        access_password.kdf_params.t_cost,
        access_password.kdf_params.p_cost,
        Some(32),
    )
    .map_err(|e| anyhow::anyhow!("argon2 params: {e}"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut master_key = [0u8; 32];
    argon
        .hash_password_into(password.as_bytes(), &salt, &mut master_key)
        .map_err(|e| anyhow::anyhow!("argon2 derive: {e}"))?;
    Ok(derive_from_master(master_key))
}

fn derive_from_master(master_key: [u8; 32]) -> DerivedKeys {
    let hk = Hkdf::<Sha256>::new(None, &master_key);
    let mut auth_key = [0u8; 32];
    hk.expand(AUTH_KEY_INFO, &mut auth_key)
        .expect("hkdf auth leg");
    let mut asset_key = [0u8; 32];
    hk.expand(ASSET_KEY_INFO, &mut asset_key)
        .expect("hkdf asset leg");
    DerivedKeys { master_key, auth_key, asset_key }
}

fn verify_auth(auth_key: &[u8; 32], expected: &[u8]) -> bool {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(auth_key).expect("hmac accepts any key length");
    mac.update(AUTH_VERIFIER_MSG);
    mac.verify_slice(expected).is_ok()
}

fn device_unlock_remembered(orb_identity_b64: &str) -> bool {
    let Ok(orb_identity) = B64.decode(orb_identity_b64) else {
        return false;
    };
    let Ok(entry) = keyring::Entry::new(DEVICE_UNLOCK_SERVICE, &B64.encode(orb_identity)) else {
        return false;
    };
    matches!(entry.get_secret(), Ok(secret) if secret.len() == 32)
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
    cipher
        .decrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: ciphertext,
                aad: cfg.aad.as_bytes(),
            },
        )
        .map_err(|_| anyhow::anyhow!("Invalid password"))
}

fn read_file<R: Read + Seek>(archive: &mut zip::ZipArchive<R>, name: &str) -> Result<Vec<u8>> {
    let mut file = archive.by_name(name)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(security_json: Option<&str>) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("orb_manifest.json", opts).unwrap();
        zip.write_all(br#"{"name":"x","version":"1.0.0","display_name":"X","description":"d","orb_format_version":"1","mcp_protocol_version":"2024-11-05","build_time":"now","source_documents":[],"chunk_count":0,"index_format_version":"0.2","binary_size_target_mb":20,"encrypted":false,"selected_retrieval_plan":"bm25_only","enabled_capabilities":["bm25"],"planning_rationale":[]}"#).unwrap();
        zip.start_file("documents.postcard", opts).unwrap();
        zip.write_all(b"d").unwrap();
        zip.start_file("chunks.postcard", opts).unwrap();
        zip.write_all(b"c").unwrap();
        zip.start_file("bm25_index.postcard", opts).unwrap();
        zip.write_all(b"b").unwrap();
        if let Some(security_json) = security_json {
            zip.start_file("orb_security.json", opts).unwrap();
            zip.write_all(security_json.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn detects_plaintext_orb() {
        let zip = make_zip(None);
        let mut archive = zip::ZipArchive::new(Cursor::new(zip)).unwrap();
        let info = inspect_orb_security_archive(&mut archive).unwrap();
        assert!(!info.password_protected);
        assert!(!info.encrypted_assets);
        assert!(!info.device_remembered);
    }

    #[test]
    fn verifies_password_only_orb() {
        let password = "hunter2-strong";
        let salt = b"0123456789abcdef";
        let params = Params::new(64, 1, 1, Some(32)).unwrap();
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut master_key = [0u8; 32];
        argon.hash_password_into(password.as_bytes(), salt, &mut master_key).unwrap();
        let keys = derive_from_master(master_key);
        let mut mac = <HmacSha256 as Mac>::new_from_slice(&keys.auth_key).unwrap();
        mac.update(AUTH_VERIFIER_MSG);
        let verifier = B64.encode(mac.finalize().into_bytes());
        let json = format!(r#"{{"schema_version":1,"access_password":{{"enabled":true,"kdf":"argon2id","kdf_params":{{"m_cost_kib":64,"t_cost":1,"p_cost":1}},"salt_b64":"{}","auth_verifier_b64":"{}","unlock_persistence":"every_launch","orb_identity_b64":"AQID"}}}}"#, B64.encode(salt), verifier);
        let zip = make_zip(Some(&json));
        let mut archive = zip::ZipArchive::new(Cursor::new(zip)).unwrap();
        assert!(verify_orb_password_archive(&mut archive, password).unwrap());
        let mut archive = zip::ZipArchive::new(Cursor::new(make_zip(Some(&json)))).unwrap();
        assert!(!verify_orb_password_archive(&mut archive, "wrong").unwrap());
    }
}
