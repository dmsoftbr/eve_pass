//! The item model (plaintext, before encryption) and item-level encrypt/decrypt.
//!
//! Organization data (`folders`, `tags`) lives **inside** the encrypted blob —
//! the server only ever sees an opaque envelope. The client decrypts everything
//! and rebuilds tree/tags/smart-views in memory.

use crate::envelope;
use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};

/// AAD binding item envelopes (separates them from key-wrap blobs).
const AAD_ITEM: &[u8] = b"eve/item";

#[derive(Debug, Serialize, Deserialize)]
pub struct Item {
    #[serde(rename = "type")]
    pub type_: String,
    pub title: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub totp: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub folders: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub custom_fields: Vec<CustomField>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CustomField {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub hidden: bool,
}

/// Validate the JSON is a well-formed item and normalize it (drops unknown
/// fields, fills defaults). Returns the canonical JSON that will be encrypted.
fn normalize(item_json: &str) -> Result<Vec<u8>> {
    let item: Item = serde_json::from_str(item_json).map_err(|e| CoreError::Serde(e.to_string()))?;
    if item.type_.trim().is_empty() {
        return Err(CoreError::Invalid("item.type must not be empty".into()));
    }
    if item.title.trim().is_empty() {
        return Err(CoreError::Invalid("item.title must not be empty".into()));
    }
    serde_json::to_vec(&item).map_err(|e| CoreError::Serde(e.to_string()))
}

/// Encrypt an item's JSON with the vault key, producing a storable envelope.
pub fn encrypt(vault_key: &[u8; 32], item_json: &str) -> Result<Vec<u8>> {
    let canonical = normalize(item_json)?;
    envelope::encrypt(vault_key, &canonical, AAD_ITEM)
}

/// Decrypt an item envelope back to its JSON string.
pub fn decrypt(vault_key: &[u8; 32], envelope_bytes: &[u8]) -> Result<String> {
    let plain = envelope::decrypt(vault_key, envelope_bytes, AAD_ITEM)?;
    String::from_utf8(plain).map_err(|e| CoreError::Serde(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "type":"login","title":"Servidor Datasul","username":"diogo.admin",
        "password":"s3cr3t","url":"datasul.cliente.com","totp":"",
        "notes":"","folders":["a","b"],"tags":["crítico"],
        "custom_fields":[{"name":"porta","value":"22","hidden":false}]
    }"#;

    #[test]
    fn round_trip_preserves_content() {
        let vk = [4u8; 32];
        let env = encrypt(&vk, SAMPLE).unwrap();
        let back = decrypt(&vk, &env).unwrap();
        let a: serde_json::Value = serde_json::from_str(SAMPLE).unwrap();
        let b: serde_json::Value = serde_json::from_str(&back).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn missing_title_rejected() {
        let vk = [4u8; 32];
        assert!(matches!(encrypt(&vk, r#"{"type":"login","title":""}"#), Err(CoreError::Invalid(_))));
    }

    #[test]
    fn wrong_key_cannot_decrypt() {
        let env = encrypt(&[1u8; 32], SAMPLE).unwrap();
        assert!(matches!(decrypt(&[2u8; 32], &env), Err(CoreError::Decrypt)));
    }
}
