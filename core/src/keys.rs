//! Key hierarchy: HKDF-SHA256 domain separation over the Argon2id `masterKey`,
//! plus AEAD wrap/unwrap of the random `vaultKey`.
//!
//! ```text
//! masterKey ─HKDF("eve/enc")─▶ encKey   (stays on device)
//!           └HKDF("eve/auth")─▶ authKey  (sent to Supabase as the "password")
//! vaultKey (random 256-bit) ──encrypts──▶ every item
//! wrapped_vault_key = AEAD(encKey, vaultKey)   (stored server-side)
//! ```
//! Changing the master password only re-wraps `vaultKey`; items are never
//! re-encrypted.

use crate::envelope;
use crate::error::{CoreError, Result};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

pub const KEY_LEN: usize = 32;
pub type Key = Zeroizing<[u8; KEY_LEN]>;

const INFO_ENC: &[u8] = b"eve/enc";
const INFO_AUTH: &[u8] = b"eve/auth";
const INFO_RECOVERY: &[u8] = b"eve/recovery";

/// AAD binding the vault-key wrap, so a wrapped vault key can't be swapped in
/// for some other AEAD blob.
const AAD_VAULT_KEY: &[u8] = b"eve/wrapped-vault-key";

/// HKDF-SHA256 expand of `ikm` into 32 bytes under `info`, with an optional salt.
fn hkdf32_salted(ikm: &[u8], salt: Option<&[u8]>, info: &[u8]) -> Result<Key> {
    let hk = Hkdf::<Sha256>::new(salt, ikm);
    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    hk.expand(info, out.as_mut())
        .map_err(|e| CoreError::Kdf(format!("hkdf: {e}")))?;
    Ok(out)
}

fn hkdf32(ikm: &[u8], info: &[u8]) -> Result<Key> {
    hkdf32_salted(ikm, None, info)
}

/// Derive `encKey` (device) and `authKey` (server) from the master key.
pub fn derive_enc_auth(master_key: &[u8; KEY_LEN]) -> Result<(Key, Key)> {
    let enc = hkdf32(master_key, INFO_ENC)?;
    let auth = hkdf32(master_key, INFO_AUTH)?;
    Ok((enc, auth))
}

/// Fase 5C — Secret-Key derivation (2SKD): derive `encKey`/`authKey` with a
/// 128-bit **Secret Key** as the HKDF salt. It never leaves the device/emergency
/// kit, so a server breach + weak master password is not enough to brute-force
/// the vault offline. This rides the params-version part of the agility layer:
/// enabling it on an account just re-derives + re-wraps the vault key (items are
/// not re-encrypted). Wired into the live opt-in flow via
/// [`crate::account::Session::enable_secret_key`] + [`crate::account::unlock_with_secret`].
pub fn derive_enc_auth_with_secret(master_key: &[u8; KEY_LEN], secret_key: &[u8]) -> Result<(Key, Key)> {
    let enc = hkdf32_salted(master_key, Some(secret_key), INFO_ENC)?;
    let auth = hkdf32_salted(master_key, Some(secret_key), INFO_AUTH)?;
    Ok((enc, auth))
}

/// Derive the wrapping key for the recovery path from the high-entropy recovery
/// code bytes (already 128-bit; no Argon2 needed).
pub fn derive_recovery_key(recovery_entropy: &[u8]) -> Result<Key> {
    hkdf32(recovery_entropy, INFO_RECOVERY)
}

/// A fresh random 256-bit key (vault key, collection key, …).
pub fn random_key() -> Result<Key> {
    let mut k = Zeroizing::new([0u8; KEY_LEN]);
    getrandom::getrandom(k.as_mut()).map_err(|e| CoreError::Random(e.to_string()))?;
    Ok(k)
}

/// `AEAD(wrapping_key, key)` → envelope, for storing a key at rest.
pub fn wrap_key(wrapping_key: &[u8; KEY_LEN], key: &[u8; KEY_LEN]) -> Result<Vec<u8>> {
    envelope::encrypt(wrapping_key, key.as_slice(), AAD_VAULT_KEY)
}

/// Reverse of [`wrap_key`]. A wrong wrapping key surfaces as `CoreError::Decrypt`.
pub fn unwrap_key(wrapping_key: &[u8; KEY_LEN], wrapped: &[u8]) -> Result<Key> {
    let plain = envelope::decrypt(wrapping_key, wrapped, AAD_VAULT_KEY)?;
    if plain.len() != KEY_LEN {
        return Err(CoreError::Invalid("unwrapped key has wrong length".into()));
    }
    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    out.copy_from_slice(&plain);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enc_and_auth_differ_and_are_stable() {
        let mk = [3u8; KEY_LEN];
        let (enc1, auth1) = derive_enc_auth(&mk).unwrap();
        let (enc2, auth2) = derive_enc_auth(&mk).unwrap();
        assert_eq!(*enc1, *enc2);
        assert_eq!(*auth1, *auth2);
        assert_ne!(*enc1, *auth1, "domain separation must produce distinct keys");
    }

    #[test]
    fn wrap_unwrap_round_trip() {
        let enc = random_key().unwrap();
        let vault = random_key().unwrap();
        let wrapped = wrap_key(&enc, &vault).unwrap();
        let unwrapped = unwrap_key(&enc, &wrapped).unwrap();
        assert_eq!(*vault, *unwrapped);
    }

    #[test]
    fn wrong_wrapping_key_fails() {
        let vault = random_key().unwrap();
        let wrapped = wrap_key(&[1u8; KEY_LEN], &vault).unwrap();
        assert!(matches!(unwrap_key(&[2u8; KEY_LEN], &wrapped), Err(CoreError::Decrypt)));
    }

    #[test]
    fn secret_key_2skd_gates_the_vault_key() {
        let mk = [4u8; KEY_LEN];
        let sk1 = [0xABu8; 16];
        let sk2 = [0xCDu8; 16];

        let (enc_plain, _) = derive_enc_auth(&mk).unwrap();
        let (enc1, _) = derive_enc_auth_with_secret(&mk, &sk1).unwrap();
        let (enc1b, _) = derive_enc_auth_with_secret(&mk, &sk1).unwrap();
        let (enc2, _) = derive_enc_auth_with_secret(&mk, &sk2).unwrap();

        // With a Secret Key the keys differ from the no-secret path, are
        // deterministic per key, and differ across secret keys.
        assert_ne!(*enc_plain, *enc1);
        assert_eq!(*enc1, *enc1b);
        assert_ne!(*enc1, *enc2);

        // The vault key wrapped under sk1's encKey can't be unwrapped without it:
        // a server breach + master password but no Secret Key stays locked out.
        let vault = random_key().unwrap();
        let wrapped = wrap_key(&enc1, &vault).unwrap();
        assert!(unwrap_key(&enc2, &wrapped).is_err()); // wrong secret key
        assert!(unwrap_key(&enc_plain, &wrapped).is_err()); // no secret key
        assert_eq!(*unwrap_key(&enc1, &wrapped).unwrap(), *vault); // correct
    }

    // RFC 5869 Appendix A.1 — HKDF-SHA256 Test Case 1. Exercises the underlying
    // primitive directly (with the RFC's salt) to prove our wiring is correct.
    #[test]
    fn rfc5869_hkdf_sha256_case1() {
        let ikm = [0x0bu8; 22];
        let salt = hex::decode("000102030405060708090a0b0c").unwrap();
        let info = hex::decode("f0f1f2f3f4f5f6f7f8f9").unwrap();
        let hk = Hkdf::<Sha256>::new(Some(&salt), &ikm);
        let mut okm = [0u8; 42];
        hk.expand(&info, &mut okm).unwrap();
        assert_eq!(
            hex::encode(okm),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
        );
    }
}
