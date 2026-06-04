//! Decryption of an asset-encrypted Orb's embedded payload (`orb_assets.enc`).
//!
//! See `plans/orb-password-access-plan.md` §2.3 / §4.4. The packaged `.orb`
//! embeds the *encrypted* asset zip instead of the plaintext asset files. On a
//! successful unlock the runtime derives `asset_key` and decrypts the payload
//! in memory back into the original asset zip, which is then parsed normally.
//!
//! The AEAD tag is the password check in encrypted mode: a wrong password yields
//! a different `asset_key`, decryption fails, and we report a generic invalid
//! password (no separate verifier — plan §2.2, S2).

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};

use crate::security::{AssetEncryptionAlgorithm, AssetEncryptionConfig, AuthError};

/// Decrypt the `orb_assets.enc` ciphertext into the plaintext asset zip bytes.
///
/// Returns [`AuthError::InvalidPassword`] when the AEAD tag does not verify
/// (wrong key / tampered payload) — deliberately indistinguishable from a wrong
/// password so we leak nothing about which it was.
pub fn decrypt_asset_blob(
    cfg: &AssetEncryptionConfig,
    asset_key: &[u8; 32],
    ciphertext: &[u8],
) -> Result<Vec<u8>, AuthError> {
    match cfg.algorithm {
        AssetEncryptionAlgorithm::XChaCha20Poly1305 => {}
    }

    if cfg.nonce.len() != 24 {
        return Err(AuthError::Crypto(format!(
            "xchacha20poly1305 nonce must be 24 bytes, got {}",
            cfg.nonce.len()
        )));
    }

    let cipher = XChaCha20Poly1305::new(Key::from_slice(asset_key));
    let nonce = XNonce::from_slice(&cfg.nonce);

    cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad: &cfg.aad,
            },
        )
        // Tag failure here is the wrong-password signal in encrypted mode.
        .map_err(|_| AuthError::InvalidPassword)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chacha20poly1305::aead::Aead;

    fn encrypt(key: &[u8; 32], nonce: &[u8; 24], aad: &[u8], msg: &[u8]) -> Vec<u8> {
        let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
        cipher
            .encrypt(XNonce::from_slice(nonce), Payload { msg, aad })
            .unwrap()
    }

    fn cfg(nonce: Vec<u8>, aad: &[u8]) -> AssetEncryptionConfig {
        AssetEncryptionConfig {
            algorithm: AssetEncryptionAlgorithm::XChaCha20Poly1305,
            payload: "orb_assets.enc".to_string(),
            nonce,
            aad: aad.to_vec(),
        }
    }

    #[test]
    fn round_trip_decrypts() {
        // Contract: correct key + nonce + aad recovers the exact plaintext.
        let key = [3u8; 32];
        let nonce = [7u8; 24];
        let aad = b"mcporb-assets-v1";
        let plain = b"the original asset zip bytes";
        let ct = encrypt(&key, &nonce, aad, plain);
        let out = decrypt_asset_blob(&cfg(nonce.to_vec(), aad), &key, &ct).unwrap();
        assert_eq!(out, plain);
    }

    #[test]
    fn wrong_key_is_invalid_password() {
        // Contract: a wrong asset_key (i.e. wrong password) fails as InvalidPassword.
        let nonce = [7u8; 24];
        let aad = b"mcporb-assets-v1";
        let ct = encrypt(&[3u8; 32], &nonce, aad, b"data");
        let err = decrypt_asset_blob(&cfg(nonce.to_vec(), aad), &[9u8; 32], &ct).unwrap_err();
        assert_eq!(err, AuthError::InvalidPassword);
    }

    #[test]
    fn tampered_ciphertext_is_invalid_password() {
        // Contract: any tamper (here, the AAD differs) fails closed.
        let key = [3u8; 32];
        let nonce = [7u8; 24];
        let ct = encrypt(&key, &nonce, b"aad-A", b"data");
        let err = decrypt_asset_blob(&cfg(nonce.to_vec(), b"aad-B"), &key, &ct).unwrap_err();
        assert_eq!(err, AuthError::InvalidPassword);
    }

    #[test]
    fn bad_nonce_length_is_crypto_error() {
        let err = decrypt_asset_blob(&cfg(vec![0u8; 12], b"aad"), &[0u8; 32], b"x").unwrap_err();
        assert!(matches!(err, AuthError::Crypto(_)));
    }
}
