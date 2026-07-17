//! Account lifecycle and the unlocked `Session`.
//!
//! The core is pure: these functions take inputs, do crypto, and return
//! bytes/records. Networking (Supabase Auth/REST/Realtime) is the shell's job —
//! it passes only ciphertext to/from here. A `Session` holds the `vaultKey` in
//! memory (zeroized on drop) and is the only thing that can decrypt items.

use std::sync::Arc;

use base64::Engine;
use zeroize::Zeroizing;

use crate::error::Result;
use crate::kdf::{self, KdfParams};
use crate::keys::{self, Key};
use crate::{item, keypair, recovery};

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Everything the shell must persist after signup. `recovery_code` is shown to
/// the user exactly once.
#[derive(uniffi::Record)]
pub struct NewAccount {
    pub kdf_salt: Vec<u8>,
    pub kdf_params: KdfParams,
    /// Base64 `authKey` — sent to Supabase Auth as the account "password".
    pub auth_key_b64: String,
    pub wrapped_vault_key: Vec<u8>,
    pub wrapped_vault_key_recovery: Vec<u8>,
    /// Emergency kit — displayed once, never stored by us.
    pub recovery_code: String,
    pub public_key: Vec<u8>,
    pub signing_public_key: Vec<u8>,
    /// Fase 5B — ML-KEM-768 encapsulation (public) key, to publish for PQ sharing.
    pub mlkem_public_key: Vec<u8>,
    pub wrapped_private_keys: Vec<u8>,
}

/// Result of a master-password change. `wrapped_vault_key` and the new
/// `authKey` change; items are **not** re-encrypted. The recovery wrap is
/// independent of the password, so it is intentionally untouched here.
#[derive(uniffi::Record)]
pub struct PasswordChange {
    pub auth_key_b64: String,
    pub wrapped_vault_key: Vec<u8>,
}

/// One item ciphertext (the full self-describing envelope).
#[derive(uniffi::Record)]
pub struct Blob {
    pub envelope: Vec<u8>,
}

/// Result of a password reset (recovery flow, Fase 4 §9): re-wraps the vault key
/// under a new password AND rotates the recovery code. Collection access is
/// preserved because the X25519/Ed25519 keys are wrapped with the (unchanged)
/// vault key.
#[derive(uniffi::Record)]
pub struct PasswordReset {
    pub auth_key_b64: String,
    pub wrapped_vault_key: Vec<u8>,
    pub wrapped_vault_key_recovery: Vec<u8>,
    pub recovery_code: String,
}

/// Fase 5C — result of enabling a Secret Key (2SKD) on an existing account. The
/// `secret_key` is shown once (emergency kit) and stored on-device only; the new
/// `auth_key_b64` and `wrapped_vault_key` must replace the server-side ones. The
/// recovery wrap is untouched (recovery bypasses the Secret Key by design).
#[derive(uniffi::Record)]
pub struct SecretKeyEnabled {
    /// 128-bit Secret Key. Never sent to the server; kept on the device + kit.
    pub secret_key: Vec<u8>,
    pub auth_key_b64: String,
    pub wrapped_vault_key: Vec<u8>,
}

/// The user's asymmetric keys, unwrapped into the session for sharing.
pub(crate) struct SessionKeys {
    pub(crate) x25519: x25519_dalek::StaticSecret,
    pub(crate) ed25519: ed25519_dalek::SigningKey,
    /// Fase 5B — ML-KEM decapsulation key, empty on accounts created before 5B.
    pub(crate) mlkem_dk: Vec<u8>,
}

/// An unlocked vault. Holds the `vaultKey`; drop it (via `lock`) to relock.
/// From Fase 4 it also holds the user's keypair and the collection keys it has
/// unwrapped (via HPKE), enabling shared-collection crypto.
#[derive(uniffi::Object)]
pub struct Session {
    pub(crate) vault_key: Key,
    pub(crate) keys: std::sync::Mutex<Option<SessionKeys>>,
    pub(crate) collection_keys: std::sync::Mutex<std::collections::HashMap<String, Key>>,
}

impl Session {
    /// Construct a session directly from an unwrapped vault key. Used by the
    /// two-step desktop login (`login.rs`) after `encKey` unwraps the vault key.
    pub(crate) fn from_vault_key(vault_key: Key) -> Arc<Session> {
        Arc::new(Session {
            vault_key,
            keys: std::sync::Mutex::new(None),
            collection_keys: std::sync::Mutex::new(std::collections::HashMap::new()),
        })
    }
}

/// Create a brand-new account: derive keys, generate the vault key, keypair, and
/// recovery code, and wrap everything for storage. Uses default (strong) KDF
/// params; callers wanting device-tuned cost can pass them to a future variant.
#[uniffi::export]
pub fn create_account(password: String) -> Result<NewAccount> {
    let params = KdfParams::default();
    let salt = kdf::random_salt()?;
    let master = Zeroizing::new(kdf::derive_master_key(&password, &salt, &params)?);
    let (enc, auth) = keys::derive_enc_auth(&master)?;

    let vault_key = keys::random_key()?;
    let wrapped_vault_key = keys::wrap_key(&enc, &vault_key)?;

    let (recovery_entropy, recovery_code) = recovery::generate()?;
    let recovery_key = keys::derive_recovery_key(recovery_entropy.as_slice())?;
    let wrapped_vault_key_recovery = keys::wrap_key(&recovery_key, &vault_key)?;

    let kp = keypair::generate(&vault_key)?;

    Ok(NewAccount {
        kdf_salt: salt.to_vec(),
        kdf_params: params,
        auth_key_b64: b64(auth.as_slice()),
        wrapped_vault_key,
        wrapped_vault_key_recovery,
        recovery_code,
        public_key: kp.public_key.to_vec(),
        signing_public_key: kp.signing_public_key.to_vec(),
        mlkem_public_key: kp.mlkem_public_key,
        wrapped_private_keys: kp.wrapped_private_keys,
    })
}

/// Derive the base64 `authKey` for `signInWithPassword`, without unlocking.
/// Used by the shell in the prelogin dance (salt comes from `login_params`).
#[uniffi::export]
pub fn auth_key_for_login(password: String, salt: Vec<u8>, params: KdfParams) -> Result<String> {
    let master = Zeroizing::new(kdf::derive_master_key(&password, &salt, &params)?);
    let (_enc, auth) = keys::derive_enc_auth(&master)?;
    Ok(b64(auth.as_slice()))
}

/// Unlock the vault with the master password: derive `encKey`, unwrap the vault
/// key, and hold it in a `Session`. Wrong password → `CoreError::Decrypt`.
#[uniffi::export]
pub fn unlock(
    password: String,
    salt: Vec<u8>,
    params: KdfParams,
    wrapped_vault_key: Vec<u8>,
) -> Result<Arc<Session>> {
    let master = Zeroizing::new(kdf::derive_master_key(&password, &salt, &params)?);
    let (enc, _auth) = keys::derive_enc_auth(&master)?;
    let vault_key = keys::unwrap_key(&enc, &wrapped_vault_key)?;
    Ok(Session::from_vault_key(vault_key))
}

/// Fase 5C — derive the base64 `authKey` for an account that has a Secret Key
/// enabled. The Secret Key salts the HKDF, so the server "password" differs from
/// the no-secret derivation. Used by the prelogin dance on a device that holds
/// the Secret Key.
#[uniffi::export]
pub fn auth_key_for_login_with_secret(
    password: String,
    salt: Vec<u8>,
    params: KdfParams,
    secret_key: Vec<u8>,
) -> Result<String> {
    let master = Zeroizing::new(kdf::derive_master_key(&password, &salt, &params)?);
    let (_enc, auth) = keys::derive_enc_auth_with_secret(&master, &secret_key)?;
    Ok(b64(auth.as_slice()))
}

/// Fase 5C — unlock an account that has a Secret Key enabled. Needs the master
/// password AND the on-device Secret Key; a server breach + master password
/// without the Secret Key cannot unwrap the vault key.
#[uniffi::export]
pub fn unlock_with_secret(
    password: String,
    salt: Vec<u8>,
    params: KdfParams,
    secret_key: Vec<u8>,
    wrapped_vault_key: Vec<u8>,
) -> Result<Arc<Session>> {
    let master = Zeroizing::new(kdf::derive_master_key(&password, &salt, &params)?);
    let (enc, _auth) = keys::derive_enc_auth_with_secret(&master, &secret_key)?;
    let vault_key = keys::unwrap_key(&enc, &wrapped_vault_key)?;
    Ok(Session::from_vault_key(vault_key))
}

/// Unlock via the recovery code (bypasses the master password). Used to reset a
/// forgotten password.
#[uniffi::export]
pub fn unlock_with_recovery(
    recovery_code: String,
    wrapped_vault_key_recovery: Vec<u8>,
) -> Result<Arc<Session>> {
    let entropy = recovery::parse(&recovery_code)?;
    let recovery_key = keys::derive_recovery_key(entropy.as_ref())?;
    let vault_key = keys::unwrap_key(&recovery_key, &wrapped_vault_key_recovery)?;
    Ok(Session::from_vault_key(vault_key))
}

#[uniffi::export]
impl Session {
    /// "unlocked" — a live session is, by definition, unlocked.
    pub fn status(&self) -> String {
        "unlocked".into()
    }

    /// Export the raw vault key for the mobile biometric path (Fase 3).
    /// ⚠️ The **only** legitimate caller is the native enclave module, which
    /// stores it in the Keychain/Keystore under biometric control. It must never
    /// be surfaced to the React Native JS layer. See `mobile.rs`.
    pub fn export_vault_key(&self) -> Vec<u8> {
        self.vault_key.to_vec()
    }

    /// Encrypt an item's JSON (§4.4) into a storable envelope.
    pub fn encrypt_item(&self, item_json: String) -> Result<Blob> {
        let envelope = item::encrypt(&self.vault_key, &item_json)?;
        Ok(Blob { envelope })
    }

    /// Decrypt an item envelope back to its JSON string.
    pub fn decrypt_item(&self, blob: Blob) -> Result<String> {
        item::decrypt(&self.vault_key, &blob.envelope)
    }

    /// Encrypt a folder's JSON (`{name, parent_id}`) into a storable envelope.
    pub fn encrypt_folder(&self, folder_json: String) -> Result<Blob> {
        let envelope = crate::folder::encrypt(&self.vault_key, &folder_json)?;
        Ok(Blob { envelope })
    }

    /// Decrypt a folder envelope back to its JSON string.
    pub fn decrypt_folder(&self, blob: Blob) -> Result<String> {
        crate::folder::decrypt(&self.vault_key, &blob.envelope)
    }

    /// Re-wrap the vault key under a new master password. Returns the new
    /// `authKey` + `wrapped_vault_key`; items stay as-is (never re-encrypted).
    pub fn change_password(
        &self,
        new_password: String,
        salt: Vec<u8>,
        params: KdfParams,
    ) -> Result<PasswordChange> {
        let master = Zeroizing::new(kdf::derive_master_key(&new_password, &salt, &params)?);
        let (enc, auth) = keys::derive_enc_auth(&master)?;
        let wrapped_vault_key = keys::wrap_key(&enc, &self.vault_key)?;
        Ok(PasswordChange { auth_key_b64: b64(auth.as_slice()), wrapped_vault_key })
    }

    /// Fase 5C — enable a Secret Key (2SKD) on this unlocked account. Generates a
    /// fresh 128-bit Secret Key, re-derives `encKey`/`authKey` with it as the HKDF
    /// salt, and re-wraps the (unchanged) vault key. The master password stays the
    /// same; items are **not** re-encrypted. Returns the Secret Key to show once +
    /// the new `authKey`/`wrapped_vault_key` for the shell to persist server-side.
    pub fn enable_secret_key(
        &self,
        password: String,
        salt: Vec<u8>,
        params: KdfParams,
    ) -> Result<SecretKeyEnabled> {
        let mut secret = Zeroizing::new([0u8; 16]);
        getrandom::getrandom(secret.as_mut())
            .map_err(|e| crate::error::CoreError::Random(e.to_string()))?;
        let master = Zeroizing::new(kdf::derive_master_key(&password, &salt, &params)?);
        let (enc, auth) = keys::derive_enc_auth_with_secret(&master, secret.as_ref())?;
        let wrapped_vault_key = keys::wrap_key(&enc, &self.vault_key)?;
        Ok(SecretKeyEnabled {
            secret_key: secret.to_vec(),
            auth_key_b64: b64(auth.as_slice()),
            wrapped_vault_key,
        })
    }

    /// Set a new password and rotate the recovery code (used after
    /// `unlock_with_recovery`). Items are not re-encrypted; collection access is
    /// preserved (asymmetric keys are wrapped with the unchanged vault key).
    pub fn reset_password(
        &self,
        new_password: String,
        salt: Vec<u8>,
        params: KdfParams,
    ) -> Result<PasswordReset> {
        let master = Zeroizing::new(kdf::derive_master_key(&new_password, &salt, &params)?);
        let (enc, auth) = keys::derive_enc_auth(&master)?;
        let wrapped_vault_key = keys::wrap_key(&enc, &self.vault_key)?;

        let (entropy, recovery_code) = recovery::generate()?;
        let recovery_key = keys::derive_recovery_key(entropy.as_slice())?;
        let wrapped_vault_key_recovery = keys::wrap_key(&recovery_key, &self.vault_key)?;

        Ok(PasswordReset {
            auth_key_b64: b64(auth.as_slice()),
            wrapped_vault_key,
            wrapped_vault_key_recovery,
            recovery_code,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CoreError;

    #[test]
    fn signup_then_unlock_round_trips_item() {
        let acct = create_account("hunter2-master".into()).unwrap();
        let session = unlock(
            "hunter2-master".into(),
            acct.kdf_salt.clone(),
            acct.kdf_params.clone(),
            acct.wrapped_vault_key.clone(),
        )
        .unwrap();

        let json = r#"{"type":"login","title":"GH","username":"me","password":"p"}"#;
        let blob = session.encrypt_item(json.into()).unwrap();
        let back = session.decrypt_item(blob).unwrap();
        // normalize() fills defaults (folders/tags/…); the supplied fields must
        // survive the round trip unchanged.
        let b: serde_json::Value = serde_json::from_str(&back).unwrap();
        assert_eq!(b["type"], "login");
        assert_eq!(b["title"], "GH");
        assert_eq!(b["username"], "me");
        assert_eq!(b["password"], "p");
    }

    #[test]
    fn wrong_password_unlock_fails_cleanly() {
        let acct = create_account("right-password".into()).unwrap();
        let r = unlock(
            "wrong-password".into(),
            acct.kdf_salt.clone(),
            acct.kdf_params.clone(),
            acct.wrapped_vault_key.clone(),
        );
        assert!(matches!(r, Err(CoreError::Decrypt)));
    }

    #[test]
    fn recovery_unlocks_when_password_forgotten() {
        let acct = create_account("forgotten".into()).unwrap();
        let session = unlock_with_recovery(
            acct.recovery_code.clone(),
            acct.wrapped_vault_key_recovery.clone(),
        )
        .unwrap();
        assert_eq!(session.status(), "unlocked");
    }

    #[test]
    fn secret_key_opt_in_gates_unlock_and_changes_auth_key() {
        let acct = create_account("master-pw".into()).unwrap();
        let session = unlock(
            "master-pw".into(),
            acct.kdf_salt.clone(),
            acct.kdf_params.clone(),
            acct.wrapped_vault_key.clone(),
        )
        .unwrap();
        let item = session.encrypt_item(r#"{"type":"login","title":"Bank"}"#.into()).unwrap();

        // Enable the Secret Key on the live session.
        let en = session
            .enable_secret_key("master-pw".into(), acct.kdf_salt.clone(), acct.kdf_params.clone())
            .unwrap();
        assert_eq!(en.secret_key.len(), 16);
        assert_ne!(en.auth_key_b64, acct.auth_key_b64, "authKey must change with the Secret Key");

        // Unlock with password + Secret Key + the new wrapped key: works, item decrypts.
        let s2 = unlock_with_secret(
            "master-pw".into(),
            acct.kdf_salt.clone(),
            acct.kdf_params.clone(),
            en.secret_key.clone(),
            en.wrapped_vault_key.clone(),
        )
        .unwrap();
        assert!(s2.decrypt_item(item).is_ok());

        // Server breach + master password but NO Secret Key: the new wrapped key
        // stays locked (plain unlock fails, and a wrong secret fails too).
        assert!(unlock(
            "master-pw".into(),
            acct.kdf_salt.clone(),
            acct.kdf_params.clone(),
            en.wrapped_vault_key.clone(),
        )
        .is_err());
        assert!(unlock_with_secret(
            "master-pw".into(),
            acct.kdf_salt.clone(),
            acct.kdf_params.clone(),
            vec![0u8; 16],
            en.wrapped_vault_key.clone(),
        )
        .is_err());

        // The prelogin authKey derivation matches enable's output.
        let ak = auth_key_for_login_with_secret(
            "master-pw".into(),
            acct.kdf_salt.clone(),
            acct.kdf_params.clone(),
            en.secret_key.clone(),
        )
        .unwrap();
        assert_eq!(ak, en.auth_key_b64);
    }

    #[test]
    fn change_password_rotates_auth_key_but_keeps_items() {
        let acct = create_account("old-pass".into()).unwrap();
        let session = unlock(
            "old-pass".into(),
            acct.kdf_salt.clone(),
            acct.kdf_params.clone(),
            acct.wrapped_vault_key.clone(),
        )
        .unwrap();
        let blob = session
            .encrypt_item(r#"{"type":"login","title":"X"}"#.into())
            .unwrap();

        let change = session
            .change_password("new-pass".into(), acct.kdf_salt.clone(), acct.kdf_params.clone())
            .unwrap();

        // Old auth key differs from the new one.
        assert_ne!(change.auth_key_b64, acct.auth_key_b64);

        // Old password no longer unlocks the new wrapped vault key.
        assert!(unlock(
            "old-pass".into(),
            acct.kdf_salt.clone(),
            acct.kdf_params.clone(),
            change.wrapped_vault_key.clone(),
        )
        .is_err());

        // New password unlocks it, and the pre-existing item still decrypts.
        let session2 = unlock(
            "new-pass".into(),
            acct.kdf_salt.clone(),
            acct.kdf_params.clone(),
            change.wrapped_vault_key.clone(),
        )
        .unwrap();
        assert!(session2.decrypt_item(blob).is_ok());
    }
}
