//! Error type for the whole core. No `unwrap`/`expect` on error paths — every
//! fallible operation returns `CoreError`. Exposed to foreign callers via UniFFI.

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum CoreError {
    /// AEAD authentication failed — wrong key/password, or tampered ciphertext.
    /// This is what a wrong master password surfaces as (never a panic).
    #[error("decryption failed (wrong password or corrupted data)")]
    Decrypt,

    /// A public-key / HPKE operation failed (bad key bytes, seal/open error).
    #[error("crypto operation failed: {0}")]
    Crypto(String),

    /// The envelope header was malformed or too short.
    #[error("invalid envelope: {0}")]
    InvalidEnvelope(String),

    /// The envelope declared a (version, alg_id) this build does not implement.
    #[error("unsupported crypto version {version}/alg {alg}")]
    UnsupportedAlg { version: u8, alg: u8 },

    /// Key derivation (Argon2id/HKDF) failed.
    #[error("key derivation failed: {0}")]
    Kdf(String),

    /// JSON (de)serialization of an item/model failed.
    #[error("serialization failed: {0}")]
    Serde(String),

    /// Caller passed invalid input (bad base64, wrong length, empty password…).
    #[error("invalid input: {0}")]
    Invalid(String),

    /// OS randomness was unavailable.
    #[error("randomness unavailable: {0}")]
    Random(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
