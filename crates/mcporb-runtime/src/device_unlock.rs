//! Remember-on-device unlock via the OS keychain / credential store (Phase 5).
//!
//! See `plans/orb-password-access-plan.md` §2.5. For `remember_on_this_device`
//! Orbs we persist the derived `master_key` directly in the OS keychain, keyed
//! by the Orb's `orb_identity` (the simplified single-store model, S1 — no
//! disk-side wrapped blob). On the next launch the runtime recalls it and
//! unlocks without the password (and without re-running Argon2).
//!
//! The plaintext password is never stored. `orb_identity` scopes the entry so
//! one Orb's remembered state can never unlock a different Orb.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// Keychain service namespace for all MCPOrb remember-on-device entries.
const SERVICE: &str = "com.mcporb.orb-unlock";

fn account(orb_identity: &[u8]) -> String {
    B64.encode(orb_identity)
}

fn entry(orb_identity: &[u8]) -> Option<keyring::Entry> {
    keyring::Entry::new(SERVICE, &account(orb_identity)).ok()
}

/// Persist the `master_key` for this Orb so future launches auto-unlock.
/// Best-effort: returns the keyring error so the caller can log it, but a
/// failure to remember must never block a successful unlock.
pub fn remember(orb_identity: &[u8], master_key: &[u8; 32]) -> Result<(), String> {
    let e = keyring::Entry::new(SERVICE, &account(orb_identity)).map_err(|e| e.to_string())?;
    e.set_secret(master_key).map_err(|e| e.to_string())
}

/// Recall a previously-remembered `master_key`, if any. Returns `None` on a
/// missing entry, a read error, or an unexpected length (treated as absent so
/// the caller falls back to the password flow).
pub fn recall(orb_identity: &[u8]) -> Option<[u8; 32]> {
    let bytes = entry(orb_identity)?.get_secret().ok()?;
    if bytes.len() == 32 {
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        Some(key)
    } else {
        None
    }
}

/// Remove this Orb's remembered key. Used on migration / stale-or-invalid entry
/// / policy change away from remember-on-device. Silent and best-effort.
pub fn forget(orb_identity: &[u8]) {
    if let Some(e) = entry(orb_identity) {
        let _ = e.delete_credential();
    }
}
