//! Fase 5D — passkeys (WebAuthn/FIDO2) as an encrypted item type.
//!
//! A passkey is a P-256 keypair + metadata (`rpId`, `userHandle`, counter). It is
//! stored as a normal encrypted item (so it inherits the vault's zero-knowledge
//! protection); using it means signing the WebAuthn challenge (ES256) with the
//! stored private key. The platform/browser provider layers (Fase 3 extension,
//! Fase 5A browser extension) drive the ceremony; this is the core crypto.

use base64::Engine;
use p256::ecdsa::{signature::Signer, Signature, SigningKey};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::error::{CoreError, Result};

fn b64(b: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(b)
}
fn unb64(s: &str) -> Result<Vec<u8>> {
    base64::engine::general_purpose::STANDARD.decode(s).map_err(|e| CoreError::Invalid(format!("b64: {e}")))
}

/// A freshly created passkey: the item JSON to store (encrypted like any item)
/// and the public key (SEC1 uncompressed) to return to the relying party.
#[derive(uniffi::Record)]
pub struct NewPasskey {
    pub item_json: String,
    pub public_key: Vec<u8>,
}

/// A WebAuthn assertion produced by [`passkey_assert`]: the DER signature, the
/// public key (for the relying party), the post-assertion signature counter, and
/// the updated item JSON (counter bumped) the shell must re-encrypt and store.
#[derive(uniffi::Record)]
pub struct PasskeyAssertion {
    pub signature: Vec<u8>,
    pub public_key: Vec<u8>,
    pub counter: u32,
    pub updated_item_json: String,
}

#[derive(Serialize, Deserialize)]
struct PasskeyItem {
    #[serde(rename = "type")]
    type_: String,
    title: String,
    rp_id: String,
    user_handle: String,
    /// P-256 private scalar (32 bytes), base64. Encrypted at rest with the item.
    private_key_b64: String,
    /// SEC1 uncompressed public key (65 bytes), base64.
    public_key_b64: String,
    counter: u32,
}

/// Create a passkey for `rp_id`/`user_handle`. Returns the item to encrypt + the
/// public key for RP registration.
#[uniffi::export]
pub fn create_passkey(rp_id: String, user_handle: String) -> Result<NewPasskey> {
    let signing = SigningKey::random(&mut OsRng);
    let verifying = signing.verifying_key();
    let public_key = verifying.to_encoded_point(false).as_bytes().to_vec(); // SEC1 uncompressed
    let priv_bytes = Zeroizing::new(signing.to_bytes());

    let item = PasskeyItem {
        type_: "passkey".into(),
        title: rp_id.clone(),
        rp_id,
        user_handle,
        private_key_b64: b64(&priv_bytes),
        public_key_b64: b64(&public_key),
        counter: 0,
    };
    let item_json = serde_json::to_string(&item).map_err(|e| CoreError::Serde(e.to_string()))?;
    Ok(NewPasskey { item_json, public_key })
}

/// Sign a WebAuthn assertion message (`authenticatorData || clientDataHash`) with
/// the passkey's private key. Returns a DER-encoded ECDSA/P-256 signature.
#[uniffi::export]
pub fn passkey_sign(item_json: String, message: Vec<u8>) -> Result<Vec<u8>> {
    let item: PasskeyItem = serde_json::from_str(&item_json).map_err(|e| CoreError::Serde(e.to_string()))?;
    let priv_bytes = Zeroizing::new(unb64(&item.private_key_b64)?);
    let signing = SigningKey::from_slice(&priv_bytes).map_err(|e| CoreError::Crypto(format!("p256 key: {e}")))?;
    let sig: Signature = signing.sign(&message);
    Ok(sig.to_der().as_bytes().to_vec())
}

/// Sign an assertion **and** advance the passkey's signature counter — the live
/// flow. The relying party expects the counter to increase on every use; the
/// caller re-encrypts and stores `updated_item_json` so the next assertion counts
/// higher. The `message` must already embed the current counter in its
/// `authenticatorData` (the caller builds it before signing).
#[uniffi::export]
pub fn passkey_assert(item_json: String, message: Vec<u8>) -> Result<PasskeyAssertion> {
    let mut item: PasskeyItem = serde_json::from_str(&item_json).map_err(|e| CoreError::Serde(e.to_string()))?;
    let priv_bytes = Zeroizing::new(unb64(&item.private_key_b64)?);
    let signing = SigningKey::from_slice(&priv_bytes).map_err(|e| CoreError::Crypto(format!("p256 key: {e}")))?;
    let sig: Signature = signing.sign(&message);

    let public_key = unb64(&item.public_key_b64)?;
    item.counter = item.counter.saturating_add(1);
    let updated_item_json = serde_json::to_string(&item).map_err(|e| CoreError::Serde(e.to_string()))?;
    Ok(PasskeyAssertion {
        signature: sig.to_der().as_bytes().to_vec(),
        public_key,
        counter: item.counter,
        updated_item_json,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::{signature::Verifier, VerifyingKey};

    #[test]
    fn create_sign_verify() {
        let pk = create_passkey("example.com".into(), "user-123".into()).unwrap();
        let message = b"authenticatorData||clientDataHash";
        let der = passkey_sign(pk.item_json.clone(), message.to_vec()).unwrap();

        // The RP verifies with the returned public key.
        let vk = VerifyingKey::from_sec1_bytes(&pk.public_key).unwrap();
        let sig = Signature::from_der(&der).unwrap();
        assert!(vk.verify(message, &sig).is_ok());
        // A tampered message fails.
        assert!(vk.verify(b"tampered", &sig).is_err());
    }

    #[test]
    fn assert_signs_and_bumps_counter() {
        let pk = create_passkey("example.com".into(), "user-123".into()).unwrap();
        let msg = b"authenticatorData||clientDataHash".to_vec();

        let a1 = passkey_assert(pk.item_json.clone(), msg.clone()).unwrap();
        assert_eq!(a1.counter, 1);
        // The updated item carries the bumped counter; the next assertion counts higher.
        let a2 = passkey_assert(a1.updated_item_json.clone(), msg.clone()).unwrap();
        assert_eq!(a2.counter, 2);

        // Both signatures verify against the returned public key.
        let vk = VerifyingKey::from_sec1_bytes(&a1.public_key).unwrap();
        assert!(vk.verify(&msg, &Signature::from_der(&a1.signature).unwrap()).is_ok());
        assert_eq!(a1.public_key, pk.public_key);
    }

    #[test]
    fn item_is_a_passkey_type() {
        let pk = create_passkey("rp".into(), "u".into()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&pk.item_json).unwrap();
        assert_eq!(v["type"], "passkey");
        assert_eq!(v["rp_id"], "rp");
        assert_eq!(pk.public_key.len(), 65); // SEC1 uncompressed
    }
}
