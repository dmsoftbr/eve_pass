//! Folder model + folder-level encrypt/decrypt. Folders are encrypted rows too
//! (so empty folders and, later, shared folders work). The plaintext is just
//! `{name, parent_id}` — the server never sees folder names or the tree shape.

use crate::envelope;
use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};

/// AAD binding folder envelopes (separates them from item/key blobs).
const AAD_FOLDER: &[u8] = b"eve/folder";

#[derive(Debug, Serialize, Deserialize)]
pub struct Folder {
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<String>,
}

fn normalize(folder_json: &str) -> Result<Vec<u8>> {
    let folder: Folder = serde_json::from_str(folder_json).map_err(|e| CoreError::Serde(e.to_string()))?;
    if folder.name.trim().is_empty() {
        return Err(CoreError::Invalid("folder.name must not be empty".into()));
    }
    serde_json::to_vec(&folder).map_err(|e| CoreError::Serde(e.to_string()))
}

pub fn encrypt(vault_key: &[u8; 32], folder_json: &str) -> Result<Vec<u8>> {
    let canonical = normalize(folder_json)?;
    envelope::encrypt(vault_key, &canonical, AAD_FOLDER)
}

pub fn decrypt(vault_key: &[u8; 32], envelope_bytes: &[u8]) -> Result<String> {
    let plain = envelope::decrypt(vault_key, envelope_bytes, AAD_FOLDER)?;
    String::from_utf8(plain).map_err(|e| CoreError::Serde(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let vk = [3u8; 32];
        let env = encrypt(&vk, r#"{"name":"Clientes","parent_id":null}"#).unwrap();
        let back = decrypt(&vk, &env).unwrap();
        let f: Folder = serde_json::from_str(&back).unwrap();
        assert_eq!(f.name, "Clientes");
        assert!(f.parent_id.is_none());
    }

    #[test]
    fn empty_name_rejected() {
        assert!(encrypt(&[3u8; 32], r#"{"name":""}"#).is_err());
    }

    #[test]
    fn item_and_folder_aad_do_not_cross() {
        // An item envelope must not decrypt as a folder (different AAD).
        let vk = [3u8; 32];
        let item_env = crate::item::encrypt(&vk, r#"{"type":"login","title":"X"}"#).unwrap();
        assert!(decrypt(&vk, &item_env).is_err());
    }
}
