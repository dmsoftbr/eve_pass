//! evepass-core — the pure cryptographic + vault core shared by every EVEPass
//! platform. It performs KDF, AEAD, key derivation, item (de)cryption, password
//! generation and TOTP. It has **no networking**: shells pass only ciphertext
//! to/from here. Foreign bindings are generated via UniFFI.

mod aead;
mod account;
mod collections;
mod envelope;
mod error;
mod folder;
mod generator;
mod health;
mod item;
mod kdf;
mod keypair;
mod keys;
mod login;
mod mobile;
mod passkey;
mod pq;
mod recovery;
mod totp;

// ── Public API surface (also the UniFFI-exported surface) ──────────────────
pub use account::{
    auth_key_for_login, create_account, unlock, unlock_with_recovery, Blob, NewAccount,
    PasswordChange, PasswordReset, Session,
};
pub use collections::{public_key_fingerprint, MemberRow, NewCollection};
pub use error::CoreError;
pub use generator::GenOptions;
pub use health::{password_score, sha1_hex};
pub use kdf::KdfParams;
pub use login::{begin_login, LoginContext};
pub use mobile::{
    extract_credential, match_credentials, session_from_vault_key, Credential, ItemMatch, MatchItem,
};
pub use passkey::{create_passkey, passkey_sign, NewPasskey};
pub use totp::TotpCode;

use error::Result;

/// Generate a password from the given options (stateless; no session needed).
#[uniffi::export]
pub fn generate_password(opts: GenOptions) -> Result<String> {
    generator::generate(&opts)
}

/// Current TOTP code for an `otpauth://` URI.
#[uniffi::export]
pub fn totp_now(otpauth_uri: String) -> Result<TotpCode> {
    totp::totp_now(&otpauth_uri)
}

/// Tune Argon2id memory cost to ~`target_ms` on this device.
#[uniffi::export]
pub fn calibrate_kdf(target_ms: u32) -> KdfParams {
    kdf::calibrate_kdf(target_ms)
}

uniffi::setup_scaffolding!();
