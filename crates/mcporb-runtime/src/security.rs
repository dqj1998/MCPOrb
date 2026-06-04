//! Local access-password gate and key derivation for packaged Orbs.
//!
//! See `plans/orb-password-access-plan.md`. The password layer is orthogonal to
//! the random URL token: the token isolates the local URL, the password
//! authorizes the user. Unlock state is process-global and lives only in memory.
//!
//! Key schedule (plan §2.2):
//!   Argon2id(password, salt, params)        -> 32-byte master_key
//!   HKDF-SHA256(master_key, "…-auth-key-v1") -> auth_key
//!   HKDF-SHA256(master_key, "…-asset-key-v1")-> asset_key
//!   HMAC-SHA256(auth_key, "mcporb-auth-v1")  -> auth_verifier (password-only mode)
//!
//! In asset-encryption mode there is **no** stored `auth_verifier` (plan §2.2,
//! S2): the `orb_assets.enc` AEAD tag authenticates the key, so a separate
//! verifier would only hand an offline attacker a smaller cracking oracle.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop};

type HmacSha256 = Hmac<Sha256>;

const AUTH_KEY_INFO: &[u8] = b"mcporb-auth-key-v1";
const ASSET_KEY_INFO: &[u8] = b"mcporb-asset-key-v1";
const AUTH_VERIFIER_MSG: &[u8] = b"mcporb-auth-v1";

// ── Errors ──────────────────────────────────────────────────────────────────

/// Authentication / unlock failure. The public-facing message is deliberately
/// generic (`Invalid password`) so it never leaks policy detail (plan §2.6).
#[derive(Debug, PartialEq, Eq)]
pub enum AuthError {
    /// Password did not match the stored verifier (or AEAD decryption failed).
    InvalidPassword,
    /// A protected resource was accessed while the Orb is locked.
    Locked,
    /// Key derivation itself failed (bad params, etc.). Internal, not user input.
    Crypto(String),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Map both wrong-password and locked to user-safe text. `Crypto` is
            // internal and should never be surfaced verbatim to a remote caller.
            AuthError::InvalidPassword => write!(f, "Invalid password"),
            AuthError::Locked => write!(f, "Orb is locked"),
            AuthError::Crypto(_) => write!(f, "Invalid password"),
        }
    }
}

impl std::error::Error for AuthError {}

// ── On-disk schema (orb_security.json) ───────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SecurityFile {
    #[allow(dead_code)]
    schema_version: u32,
    #[serde(default)]
    access_password: Option<AccessPasswordFile>,
    #[serde(default)]
    asset_encryption: Option<AssetEncryptionFile>,
}

#[derive(Debug, Deserialize)]
struct AccessPasswordFile {
    enabled: bool,
    kdf: String,
    kdf_params: KdfParamsFile,
    salt_b64: String,
    #[serde(default)]
    auth_verifier_b64: Option<String>,
    unlock_persistence: String,
    orb_identity_b64: String,
}

#[derive(Debug, Deserialize)]
struct KdfParamsFile {
    m_cost_kib: u32,
    t_cost: u32,
    p_cost: u32,
}

#[derive(Debug, Deserialize)]
struct AssetEncryptionFile {
    enabled: bool,
    algorithm: String,
    payload: String,
    nonce_b64: String,
    aad: String,
}

// ── Runtime config ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Argon2Params {
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnlockPersistence {
    EveryLaunch,
    RememberOnThisDevice,
}

#[derive(Debug, Clone)]
pub struct PasswordConfig {
    pub kdf_params: Argon2Params,
    pub salt: Vec<u8>,
    /// Present only in password-only mode; `None` when asset encryption is on
    /// (the AEAD tag authenticates the key instead — plan §2.2, S2).
    pub auth_verifier: Option<Vec<u8>>,
    /// Consumed by the keychain remember-on-device path (Phase 5).
    #[allow(dead_code)]
    pub unlock_persistence: UnlockPersistence,
    /// Stable per-Orb identity, used to key the OS keychain entry (Phase 5).
    #[allow(dead_code)]
    pub orb_identity: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetEncryptionAlgorithm {
    XChaCha20Poly1305,
}

#[derive(Debug, Clone)]
pub struct AssetEncryptionConfig {
    pub algorithm: AssetEncryptionAlgorithm,
    #[allow(dead_code)] // payload name is fixed ("orb_assets.enc"); kept for clarity/forward use
    pub payload: String,
    pub nonce: Vec<u8>,
    pub aad: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct SecurityConfig {
    pub password: Option<PasswordConfig>,
    pub asset_encryption: Option<AssetEncryptionConfig>,
}

impl SecurityConfig {
    /// No password, no encryption — the default for demo mode and any Orb
    /// packaged without `--password`.
    pub fn disabled() -> Self {
        SecurityConfig::default()
    }

    /// Parse an `orb_security.json` byte blob into a runtime config.
    ///
    /// Invalid JSON is an error. A well-formed file with `enabled = false`
    /// (or absent `access_password`) parses to a disabled config.
    pub fn from_bundle_json(bytes: &[u8]) -> Result<Self, AuthError> {
        let file: SecurityFile = serde_json::from_slice(bytes)
            .map_err(|e| AuthError::Crypto(format!("parse orb_security.json: {e}")))?;

        let password = match file.access_password {
            Some(p) if p.enabled => Some(parse_password(p)?),
            _ => None,
        };

        let asset_encryption = match file.asset_encryption {
            Some(a) if a.enabled => Some(parse_asset_encryption(a)?),
            _ => None,
        };

        Ok(SecurityConfig {
            password,
            asset_encryption,
        })
    }
}

fn b64(field: &str, s: &str) -> Result<Vec<u8>, AuthError> {
    B64.decode(s)
        .map_err(|e| AuthError::Crypto(format!("base64 decode {field}: {e}")))
}

fn parse_password(p: AccessPasswordFile) -> Result<PasswordConfig, AuthError> {
    if p.kdf != "argon2id" {
        return Err(AuthError::Crypto(format!("unsupported kdf: {}", p.kdf)));
    }
    let unlock_persistence = match p.unlock_persistence.as_str() {
        "every_launch" => UnlockPersistence::EveryLaunch,
        "remember_on_this_device" => UnlockPersistence::RememberOnThisDevice,
        other => {
            return Err(AuthError::Crypto(format!(
                "unknown unlock_persistence: {other}"
            )))
        }
    };
    Ok(PasswordConfig {
        kdf_params: Argon2Params {
            m_cost_kib: p.kdf_params.m_cost_kib,
            t_cost: p.kdf_params.t_cost,
            p_cost: p.kdf_params.p_cost,
        },
        salt: b64("salt_b64", &p.salt_b64)?,
        auth_verifier: p
            .auth_verifier_b64
            .as_deref()
            .map(|s| b64("auth_verifier_b64", s))
            .transpose()?,
        unlock_persistence,
        orb_identity: b64("orb_identity_b64", &p.orb_identity_b64)?,
    })
}

fn parse_asset_encryption(a: AssetEncryptionFile) -> Result<AssetEncryptionConfig, AuthError> {
    let algorithm = match a.algorithm.as_str() {
        "xchacha20poly1305" => AssetEncryptionAlgorithm::XChaCha20Poly1305,
        other => return Err(AuthError::Crypto(format!("unsupported algorithm: {other}"))),
    };
    Ok(AssetEncryptionConfig {
        algorithm,
        payload: a.payload,
        nonce: b64("nonce_b64", &a.nonce_b64)?,
        aad: a.aad.into_bytes(),
    })
}

// ── Derived key material ──────────────────────────────────────────────────────

/// Secret keys derived from the user password. Zeroized on drop so plaintext
/// key bytes do not linger in freed memory.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct DerivedKeys {
    /// Argon2id output. Wrapped/stored by the keychain in `remember` mode (Phase 5).
    pub master_key: [u8; 32],
    /// HKDF leg used for the auth verifier (password-only mode).
    auth_key: [u8; 32],
    /// HKDF leg used to AEAD-decrypt `orb_assets.enc` (Phase 4).
    pub asset_key: [u8; 32],
}

impl std::fmt::Debug for DerivedKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print key bytes.
        f.write_str("DerivedKeys(<redacted>)")
    }
}

// ── Process-global unlock state ───────────────────────────────────────────────

/// Holds the parsed config plus the live unlock flag. Shared via `Arc<OrbState>`
/// across the web server and the stdio MCP loop, so a single successful unlock
/// in any surface unlocks the whole process (plan §2.4).
pub struct SecurityState {
    config: SecurityConfig,
    /// Lock-free because `is_unlocked()` is on the gate hot path (every request).
    unlocked: AtomicBool,
    failed_attempts: Mutex<u32>,
}

impl SecurityState {
    pub fn new(config: SecurityConfig) -> Self {
        SecurityState {
            config,
            unlocked: AtomicBool::new(false),
            failed_attempts: Mutex::new(0),
        }
    }

    /// Read access to the parsed config (asset-encryption metadata, persistence
    /// policy).
    pub fn config(&self) -> &SecurityConfig {
        &self.config
    }

    /// True if this Orb requires a password to be entered.
    pub fn password_required(&self) -> bool {
        self.config.password.is_some()
    }

    /// True if the Orb is open for use: either no password is required, or one
    /// has been successfully entered this process lifetime.
    pub fn is_unlocked(&self) -> bool {
        !self.password_required() || self.unlocked.load(Ordering::Acquire)
    }

    /// Gate for protected resources.
    pub fn require_unlocked(&self) -> Result<(), AuthError> {
        if self.is_unlocked() {
            Ok(())
        } else {
            Err(AuthError::Locked)
        }
    }

    /// Mark the process unlocked. Called after a verified password (password-only
    /// mode) or after a successful asset decryption (encrypted mode, Phase 4).
    pub fn mark_unlocked(&self) {
        self.unlocked.store(true, Ordering::Release);
        if let Ok(mut n) = self.failed_attempts.lock() {
            *n = 0;
        }
    }

    /// Derive key material from a candidate password. Pure: does not touch unlock
    /// state. `Err(Crypto)` only on bad KDF params, never on a wrong password.
    pub fn derive_keys(&self, password: &str) -> Result<DerivedKeys, AuthError> {
        let pc = self
            .config
            .password
            .as_ref()
            .ok_or_else(|| AuthError::Crypto("no password configured".into()))?;
        derive_keys(password, &pc.salt, pc.kdf_params)
    }

    /// Verify a password and, in password-only mode, unlock the process.
    ///
    /// - Password-only mode (`auth_verifier` present): constant-time HMAC check;
    ///   on success marks unlocked and returns the derived keys.
    /// - Encrypted mode (`auth_verifier` absent): derivation succeeds but the
    ///   keys cannot be verified here — the caller must AEAD-decrypt the assets
    ///   and then call [`mark_unlocked`] (wired in Phase 4). Returns the keys
    ///   **without** unlocking.
    ///
    /// On a wrong password returns [`AuthError::InvalidPassword`] and records a
    /// failed attempt (see [`backoff_delay`]).
    pub fn verify_and_unlock(&self, password: &str) -> Result<DerivedKeys, AuthError> {
        let pc = self
            .config
            .password
            .as_ref()
            .ok_or_else(|| AuthError::Crypto("no password configured".into()))?;

        let keys = derive_keys(password, &pc.salt, pc.kdf_params)?;

        match pc.auth_verifier.as_deref() {
            Some(expected) => {
                if verify_auth(&keys.auth_key, expected) {
                    self.mark_unlocked();
                    Ok(keys)
                } else {
                    self.record_failure();
                    Err(AuthError::InvalidPassword)
                }
            }
            // Encrypted mode: defer verification to AEAD decryption (Phase 4).
            None => Ok(keys),
        }
    }

    /// Record a failed unlock attempt (drives [`backoff_delay`]). Public to the
    /// crate so the encrypted-mode unlock path (where the AEAD tag is the check)
    /// can record failures too.
    pub(crate) fn record_failure(&self) {
        if let Ok(mut n) = self.failed_attempts.lock() {
            *n = n.saturating_add(1);
        }
    }

    /// Throttle delay to apply before responding to the next attempt, based on
    /// accumulated failures (plan §2.6: 5→1s, 10→3s). No permanent lockout — a
    /// local offline tool must never lock the owner out.
    pub fn backoff_delay(&self) -> Duration {
        let n = self.failed_attempts.lock().map(|g| *g).unwrap_or(0);
        if n >= 10 {
            Duration::from_secs(3)
        } else if n >= 5 {
            Duration::from_secs(1)
        } else {
            Duration::ZERO
        }
    }
}

fn derive_keys(
    password: &str,
    salt: &[u8],
    params: Argon2Params,
) -> Result<DerivedKeys, AuthError> {
    let argon_params = Params::new(params.m_cost_kib, params.t_cost, params.p_cost, Some(32))
        .map_err(|e| AuthError::Crypto(format!("argon2 params: {e}")))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);

    let mut master_key = [0u8; 32];
    argon
        .hash_password_into(password.as_bytes(), salt, &mut master_key)
        .map_err(|e| AuthError::Crypto(format!("argon2: {e}")))?;

    let hk = Hkdf::<Sha256>::new(None, &master_key);
    let mut auth_key = [0u8; 32];
    let mut asset_key = [0u8; 32];
    hk.expand(AUTH_KEY_INFO, &mut auth_key)
        .map_err(|e| AuthError::Crypto(format!("hkdf auth: {e}")))?;
    hk.expand(ASSET_KEY_INFO, &mut asset_key)
        .map_err(|e| AuthError::Crypto(format!("hkdf asset: {e}")))?;

    Ok(DerivedKeys {
        master_key,
        auth_key,
        asset_key,
    })
}

/// Constant-time check that `auth_key` reproduces the stored verifier.
fn verify_auth(auth_key: &[u8; 32], expected: &[u8]) -> bool {
    let mut mac = match HmacSha256::new_from_slice(auth_key) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(AUTH_VERIFIER_MSG);
    mac.verify_slice(expected).is_ok()
}

/// Compute the `auth_verifier` for a given `auth_key`. Shared shape with the
/// packaging side (Phase 6) so both ends agree on the construction.
#[allow(dead_code)]
pub fn compute_auth_verifier(auth_key: &[u8; 32]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(auth_key).expect("hmac accepts any key length");
    mac.update(AUTH_VERIFIER_MSG);
    mac.finalize().into_bytes().to_vec()
}

/// Cheap Argon2 params so test suites stay fast. Production defaults are far
/// higher (plan §2.2: m=128 MiB, t=3) — that cost is data, not code.
#[cfg(test)]
pub(crate) const TEST_PARAMS: Argon2Params = Argon2Params {
    m_cost_kib: 64,
    t_cost: 1,
    p_cost: 1,
};

/// Build a password-only config whose verifier matches `password`, the way the
/// packaging side would (derive keys, store HMAC(auth_key)). Shared across the
/// crate's test modules (api, mcp_handler).
#[cfg(test)]
pub(crate) fn test_password_config(password: &str) -> SecurityConfig {
    let salt = b"0123456789abcdef".to_vec();
    let keys = derive_keys(password, &salt, TEST_PARAMS).unwrap();
    SecurityConfig {
        password: Some(PasswordConfig {
            kdf_params: TEST_PARAMS,
            salt,
            auth_verifier: Some(compute_auth_verifier(&keys.auth_key)),
            unlock_persistence: UnlockPersistence::EveryLaunch,
            orb_identity: vec![1, 2, 3, 4],
        }),
        asset_encryption: None,
    }
}

/// Emit an `orb_security.json` (password-only) string whose verifier matches
/// `password`. Mirrors what the packaging side will write (Phase 6); used by the
/// loader round-trip test until that real serializer exists.
#[cfg(test)]
pub(crate) fn test_security_json(password: &str) -> String {
    let salt: &[u8] = b"0123456789abcdef";
    let keys = derive_keys(password, salt, TEST_PARAMS).unwrap();
    let verifier = compute_auth_verifier(&keys.auth_key);
    format!(
        r#"{{"schema_version":1,"access_password":{{"enabled":true,"kdf":"argon2id","kdf_params":{{"m_cost_kib":{},"t_cost":{},"p_cost":{}}},"salt_b64":"{}","auth_verifier_b64":"{}","unlock_persistence":"every_launch","orb_identity_b64":"AQID"}}}}"#,
        TEST_PARAMS.m_cost_kib,
        TEST_PARAMS.t_cost,
        TEST_PARAMS.p_cost,
        B64.encode(salt),
        B64.encode(verifier),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::TEST_PARAMS;

    fn password_only_config(password: &str) -> SecurityConfig {
        super::test_password_config(password)
    }

    #[test]
    fn disabled_config_is_always_unlocked() {
        // Contract: an Orb with no password requires no unlock and gates pass.
        let s = SecurityState::new(SecurityConfig::disabled());
        assert!(!s.password_required());
        assert!(s.is_unlocked());
        assert!(s.require_unlocked().is_ok());
    }

    #[test]
    fn correct_password_unlocks() {
        // Contract: the right password flips the process to unlocked.
        let s = SecurityState::new(password_only_config("hunter2-strong"));
        assert!(s.password_required());
        assert!(!s.is_unlocked());
        assert!(s.require_unlocked().is_err());

        let keys = s.verify_and_unlock("hunter2-strong").unwrap();
        assert_eq!(keys.master_key.len(), 32);
        assert!(s.is_unlocked());
        assert!(s.require_unlocked().is_ok());
    }

    #[test]
    fn wrong_password_does_not_unlock() {
        // Contract: a wrong password is rejected and leaves the Orb locked.
        let s = SecurityState::new(password_only_config("correct-horse"));
        let err = s.verify_and_unlock("battery-staple").unwrap_err();
        assert_eq!(err, AuthError::InvalidPassword);
        assert!(!s.is_unlocked());
        assert!(s.require_unlocked().is_err());
    }

    #[test]
    fn error_message_never_leaks_detail() {
        // Contract: user-facing text is the generic "Invalid password" (§2.6).
        assert_eq!(AuthError::InvalidPassword.to_string(), "Invalid password");
        assert_eq!(
            AuthError::Crypto("internal salt mismatch".into()).to_string(),
            "Invalid password"
        );
    }

    #[test]
    fn backoff_grows_with_failures_then_caps() {
        // Contract: repeated failures add delay but never permanently lock out.
        let s = SecurityState::new(password_only_config("pw-aaaa-bbbb"));
        assert_eq!(s.backoff_delay(), Duration::ZERO);
        for _ in 0..5 {
            let _ = s.verify_and_unlock("nope-nope-nope");
        }
        assert_eq!(s.backoff_delay(), Duration::from_secs(1));
        for _ in 0..5 {
            let _ = s.verify_and_unlock("nope-nope-nope");
        }
        assert_eq!(s.backoff_delay(), Duration::from_secs(3));
        // A correct password still works after many failures (no lockout).
        assert!(s.verify_and_unlock("pw-aaaa-bbbb").is_ok());
        assert_eq!(s.backoff_delay(), Duration::ZERO);
    }

    #[test]
    fn parse_disabled_when_flag_false() {
        // Contract: enabled=false parses to a disabled (no-password) config.
        let json = br#"{
            "schema_version": 1,
            "access_password": { "enabled": false, "kdf": "argon2id",
                "kdf_params": {"m_cost_kib":64,"t_cost":1,"p_cost":1},
                "salt_b64":"AAAA","unlock_persistence":"every_launch",
                "orb_identity_b64":"AQID" }
        }"#;
        let cfg = SecurityConfig::from_bundle_json(json).unwrap();
        assert!(cfg.password.is_none());
    }

    #[test]
    fn parse_enabled_password_only() {
        // Contract: a valid enabled file round-trips into a usable PasswordConfig.
        let salt = b"0123456789abcdef".to_vec();
        let keys = derive_keys("file-pw-strong", &salt, TEST_PARAMS).unwrap();
        let json = format!(
            r#"{{
                "schema_version": 1,
                "access_password": {{
                    "enabled": true, "kdf": "argon2id",
                    "kdf_params": {{"m_cost_kib":64,"t_cost":1,"p_cost":1}},
                    "salt_b64": "{}",
                    "auth_verifier_b64": "{}",
                    "unlock_persistence": "remember_on_this_device",
                    "orb_identity_b64": "AQIDBA=="
                }}
            }}"#,
            B64.encode(&salt),
            B64.encode(compute_auth_verifier(&keys.auth_key)),
        );
        let cfg = SecurityConfig::from_bundle_json(json.as_bytes()).unwrap();
        let pc = cfg.password.as_ref().expect("password present");
        assert_eq!(pc.unlock_persistence, UnlockPersistence::RememberOnThisDevice);
        assert!(pc.auth_verifier.is_some());

        let s = SecurityState::new(cfg);
        assert!(s.verify_and_unlock("file-pw-strong").is_ok());
    }

    #[test]
    fn parse_rejects_invalid_json() {
        // Contract: malformed JSON is an error, not a silent disabled config.
        assert!(SecurityConfig::from_bundle_json(b"not json").is_err());
    }

    #[test]
    fn encrypted_mode_derives_without_unlocking() {
        // Contract: with no verifier (encrypted mode), verify_and_unlock returns
        // keys but does NOT unlock — Phase 4 unlocks only after AEAD succeeds.
        let salt = b"0123456789abcdef".to_vec();
        let cfg = SecurityConfig {
            password: Some(PasswordConfig {
                kdf_params: TEST_PARAMS,
                salt,
                auth_verifier: None,
                unlock_persistence: UnlockPersistence::EveryLaunch,
                orb_identity: vec![9, 9],
            }),
            asset_encryption: None,
        };
        let s = SecurityState::new(cfg);
        let keys = s.verify_and_unlock("any-password").unwrap();
        assert_eq!(keys.asset_key.len(), 32);
        assert!(!s.is_unlocked(), "encrypted mode must not self-unlock");
    }
}
