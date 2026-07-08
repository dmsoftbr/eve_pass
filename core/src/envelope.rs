//! Self-describing crypto envelope — the crypto-agility layer.
//!
//! Every ciphertext stored or transmitted is:
//! ```text
//! version (1 byte) || alg_id (1 byte) || nonce (24 bytes) || ciphertext+tag
//! ```
//! `decrypt` reads `version`/`alg_id` and dispatches to the right primitive set.
//! Because the nonce travels inside the envelope, the database stores the whole
//! envelope in one `ciphertext` column — there is no separate `nonce` column.
//!
//! v1/alg1 = { Argon2id, HKDF-SHA256, XChaCha20-Poly1305, X25519, Ed25519 }.
//! A v2 dispatch arm is stubbed to prove the version-routing path works, so a
//! post-quantum/Secret-Key suite can be added later without a format rewrite.

use crate::aead::{self, KEY_LEN, NONCE_LEN};
use crate::error::{CoreError, Result};

pub const VERSION_1: u8 = 1;
pub const ALG_1: u8 = 1;
/// Reserved for the next crypto suite. Dispatched-but-unimplemented on purpose.
pub const VERSION_2: u8 = 2;

const HEADER_LEN: usize = 2; // version + alg_id
const PREFIX_LEN: usize = HEADER_LEN + NONCE_LEN;

/// Seal `plaintext` under `key`, producing a v1 envelope. `aad` is authenticated
/// but not stored; callers must supply the same `aad` to `decrypt`.
pub fn encrypt(key: &[u8; KEY_LEN], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    let nonce = aead::random_nonce()?;
    let ct = aead::seal(key, &nonce, plaintext, aad)?;
    let mut out = Vec::with_capacity(PREFIX_LEN + ct.len());
    out.push(VERSION_1);
    out.push(ALG_1);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Read the envelope header and dispatch to the matching primitive set.
pub fn decrypt(key: &[u8; KEY_LEN], envelope: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    if envelope.len() < HEADER_LEN {
        return Err(CoreError::InvalidEnvelope("shorter than header".into()));
    }
    let version = envelope[0];
    let alg = envelope[1];
    match (version, alg) {
        (VERSION_1, ALG_1) => decrypt_v1(key, envelope, aad),
        // ── v2 dispatch stub ──────────────────────────────────────────────
        // Proves version routing without a second suite implemented yet.
        // Returning UnsupportedAlg (not InvalidEnvelope) confirms the header
        // was parsed and routed here, not rejected as garbage.
        (VERSION_2, alg) => Err(CoreError::UnsupportedAlg { version: VERSION_2, alg }),
        (version, alg) => Err(CoreError::UnsupportedAlg { version, alg }),
    }
}

fn decrypt_v1(key: &[u8; KEY_LEN], envelope: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    if envelope.len() < PREFIX_LEN {
        return Err(CoreError::InvalidEnvelope("v1 envelope missing nonce".into()));
    }
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&envelope[HEADER_LEN..PREFIX_LEN]);
    let ciphertext = &envelope[PREFIX_LEN..];
    aead::open(key, &nonce, ciphertext, aad)
}

/// The (version, alg_id) an envelope declares, without decrypting it.
#[allow(dead_code)] // used by tests + shells (sync/inspection) in later phases
pub fn header(envelope: &[u8]) -> Result<(u8, u8)> {
    if envelope.len() < HEADER_LEN {
        return Err(CoreError::InvalidEnvelope("shorter than header".into()));
    }
    Ok((envelope[0], envelope[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_empty_aad() {
        let key = [7u8; KEY_LEN];
        let env = encrypt(&key, b"hello vault", b"").unwrap();
        assert_eq!(header(&env).unwrap(), (VERSION_1, ALG_1));
        assert_eq!(decrypt(&key, &env, b"").unwrap(), b"hello vault");
    }

    #[test]
    fn round_trip_with_aad() {
        let key = [9u8; KEY_LEN];
        let env = encrypt(&key, b"secret", b"item:123").unwrap();
        assert_eq!(decrypt(&key, &env, b"item:123").unwrap(), b"secret");
        // Wrong AAD must fail authentication.
        assert!(matches!(decrypt(&key, &env, b"item:999"), Err(CoreError::Decrypt)));
    }

    #[test]
    fn wrong_key_fails_without_panic() {
        let env = encrypt(&[1u8; KEY_LEN], b"data", b"").unwrap();
        assert!(matches!(decrypt(&[2u8; KEY_LEN], &env, b""), Err(CoreError::Decrypt)));
    }

    #[test]
    fn v2_header_dispatches_to_stub() {
        // Hand-craft a v2 envelope; decrypt must route to the v2 arm (unsupported),
        // NOT try to open it as v1 and NOT reject it as an invalid envelope.
        let mut env = vec![VERSION_2, ALG_1];
        env.extend_from_slice(&[0u8; NONCE_LEN]);
        env.extend_from_slice(b"whatever");
        match decrypt(&[0u8; KEY_LEN], &env, b"") {
            Err(CoreError::UnsupportedAlg { version, alg }) => {
                assert_eq!((version, alg), (VERSION_2, ALG_1));
            }
            other => panic!("expected v2 dispatch, got {other:?}"),
        }
    }

    #[test]
    fn truncated_envelope_is_invalid_not_panic() {
        assert!(matches!(decrypt(&[0u8; KEY_LEN], &[1], b""), Err(CoreError::InvalidEnvelope(_))));
        assert!(matches!(decrypt(&[0u8; KEY_LEN], &[VERSION_1, ALG_1, 0, 0], b""), Err(CoreError::InvalidEnvelope(_))));
    }
}
