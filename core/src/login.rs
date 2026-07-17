//! Two-step login that runs the expensive Argon2id pass exactly once.
//!
//! The Fase 0 `unlock` re-derived the master key from the password. On desktop
//! the prelogin dance needs the `authKey` *before* the profile (and its
//! `wrapped_vault_key`) can be downloaded, so login is split:
//!
//! 1. [`begin_login`] derives `masterKey → encKey` + `authKey` once and returns
//!    a [`LoginContext`] that holds `encKey` (in Rust, zeroized on drop).
//! 2. The shell signs in with `authKey`, downloads the profile, then calls
//!    [`LoginContext::complete`] with the `wrapped_vault_key` to get a `Session`.
//!
//! This type is deliberately **not** UniFFI-exported: it is held as Rust state
//! by the Tauri backend. Mobile (Fase 3) drives the same core primitives.

use std::sync::Arc;

use base64::Engine;
use zeroize::Zeroizing;

use crate::account::Session;
use crate::error::Result;
use crate::kdf::{self, KdfParams};
use crate::keys::{self, Key};

/// Transient login state between `begin_login` and `complete`. Holds `encKey`
/// only — never the password or master key beyond derivation.
pub struct LoginContext {
    enc_key: Key,
    auth_key_b64: String,
}

/// Run Argon2id once; derive `encKey` (kept) and `authKey` (returned for the
/// server sign-in).
pub fn begin_login(password: &str, salt: &[u8], params: &KdfParams) -> Result<LoginContext> {
    let master = Zeroizing::new(kdf::derive_master_key(password, salt, params)?);
    let (enc, auth) = keys::derive_enc_auth(&master)?;
    let auth_key_b64 = base64::engine::general_purpose::STANDARD.encode(auth.as_slice());
    Ok(LoginContext { enc_key: enc, auth_key_b64 })
}

/// Fase 5C — two-step login for an account with a Secret Key enabled: the Secret
/// Key salts the HKDF so both `encKey` and `authKey` differ from the no-secret
/// path. The device must hold the Secret Key.
pub fn begin_login_with_secret(
    password: &str,
    salt: &[u8],
    params: &KdfParams,
    secret_key: &[u8],
) -> Result<LoginContext> {
    let master = Zeroizing::new(kdf::derive_master_key(password, salt, params)?);
    let (enc, auth) = keys::derive_enc_auth_with_secret(&master, secret_key)?;
    let auth_key_b64 = base64::engine::general_purpose::STANDARD.encode(auth.as_slice());
    Ok(LoginContext { enc_key: enc, auth_key_b64 })
}

impl LoginContext {
    /// The base64 `authKey` to hand to `signInWithPassword`.
    pub fn auth_key_b64(&self) -> &str {
        &self.auth_key_b64
    }

    /// Unwrap the vault key with the held `encKey` and produce an unlocked
    /// `Session`. `encKey` is dropped (zeroized) with the context afterwards.
    pub fn complete(&self, wrapped_vault_key: &[u8]) -> Result<Arc<Session>> {
        let vault_key = keys::unwrap_key(&self.enc_key, wrapped_vault_key)?;
        Ok(Session::from_vault_key(vault_key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::create_account;

    #[test]
    fn begin_then_complete_matches_direct_unlock() {
        let acct = create_account("desktop-pass".into()).unwrap();
        let ctx = begin_login("desktop-pass", &acct.kdf_salt, &acct.kdf_params).unwrap();
        // authKey from begin_login equals the one signup registered.
        assert_eq!(ctx.auth_key_b64(), acct.auth_key_b64);
        // complete unlocks and can decrypt an item.
        let session = ctx.complete(&acct.wrapped_vault_key).unwrap();
        let blob = session.encrypt_item(r#"{"type":"login","title":"T"}"#.into()).unwrap();
        assert!(session.decrypt_item(blob).is_ok());
    }

    #[test]
    fn wrong_password_fails_at_complete() {
        let acct = create_account("right".into()).unwrap();
        let ctx = begin_login("wrong", &acct.kdf_salt, &acct.kdf_params).unwrap();
        assert!(ctx.complete(&acct.wrapped_vault_key).is_err());
    }
}
