//! Asymmetric keypair generated at signup for future sharing (Fase 4 / HPKE).
//! In Fase 0 we only generate, wrap, and store it — nothing signs or exchanges
//! yet. Private keys are serialized and encrypted with the `vaultKey`.

use crate::envelope;
use crate::error::{CoreError, Result};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};
use zeroize::Zeroizing;

/// AAD binding the wrapped private-keys blob.
const AAD_PRIVATE_KEYS: &[u8] = b"eve/wrapped-private-keys";

pub struct KeyPair {
    /// X25519 public key (32 bytes) — stored in the clear in `profiles`.
    pub public_key: [u8; 32],
    /// Ed25519 public key (32 bytes) — stored in the clear in `profiles`.
    pub signing_public_key: [u8; 32],
    /// Fase 5B — ML-KEM-768 encapsulation (public) key, published alongside the
    /// X25519 pub so senders can hybrid-wrap collection keys for this recipient.
    pub mlkem_public_key: Vec<u8>,
    /// `AEAD(vaultKey, {x25519, ed25519, mlkem seeds/keys})` — stored in `profiles`.
    pub wrapped_private_keys: Vec<u8>,
}

/// The private halves, wrapped with the vault key. `mlkem` is optional so blobs
/// written before Fase 5B still deserialize (older accounts have no PQ key).
#[derive(Serialize, Deserialize, zeroize::ZeroizeOnDrop)]
struct PrivateKeys {
    x25519: [u8; 32],
    ed25519: [u8; 32],
    /// ML-KEM-768 decapsulation (private) key bytes. Empty on pre-5B accounts.
    #[serde(default)]
    mlkem: Vec<u8>,
}

/// The unwrapped private keys, for loading into a `Session`.
pub struct PrivateKeyMaterial {
    pub x25519: StaticSecret,
    pub ed25519: SigningKey,
    /// ML-KEM decapsulation key, empty if this account predates Fase 5B.
    pub mlkem_dk: Vec<u8>,
}

/// Generate a fresh X25519 + Ed25519 + ML-KEM-768 keypair and wrap the private
/// halves with the vault key.
pub fn generate(vault_key: &[u8; 32]) -> Result<KeyPair> {
    let mut x_seed = Zeroizing::new([0u8; 32]);
    let mut ed_seed = Zeroizing::new([0u8; 32]);
    getrandom::getrandom(x_seed.as_mut()).map_err(|e| CoreError::Random(e.to_string()))?;
    getrandom::getrandom(ed_seed.as_mut()).map_err(|e| CoreError::Random(e.to_string()))?;

    let x_secret = StaticSecret::from(*x_seed);
    let x_public = XPublicKey::from(&x_secret);
    let signing = SigningKey::from_bytes(&ed_seed);
    let verifying = signing.verifying_key();

    let mlkem = crate::pq::generate_mlkem_keypair();

    let priv_keys = PrivateKeys {
        x25519: *x_seed,
        ed25519: *ed_seed,
        mlkem: mlkem.decapsulation_key.to_vec(),
    };
    let json = serde_json::to_vec(&priv_keys).map_err(|e| CoreError::Serde(e.to_string()))?;
    let json = Zeroizing::new(json);
    let wrapped = envelope::encrypt(vault_key, &json, AAD_PRIVATE_KEYS)?;

    Ok(KeyPair {
        public_key: x_public.to_bytes(),
        signing_public_key: verifying.to_bytes(),
        mlkem_public_key: mlkem.encapsulation_key,
        wrapped_private_keys: wrapped,
    })
}

/// Decrypt the wrapped private keys with the vault key (used once sharing lands;
/// exercised here to prove the blob round-trips).
#[allow(dead_code)]
pub fn unwrap_private_keys(vault_key: &[u8; 32], wrapped: &[u8]) -> Result<()> {
    let plain = envelope::decrypt(vault_key, wrapped, AAD_PRIVATE_KEYS)?;
    let _keys: PrivateKeys = serde_json::from_slice(&plain).map_err(|e| CoreError::Serde(e.to_string()))?;
    Ok(())
}

/// Reconstruct the X25519 secret + Ed25519 signing key + ML-KEM decapsulation
/// key from the wrapped blob. Used at unlock to enable collection sharing
/// (HPKE v1 and hybrid PQ v2).
pub fn open_private_keys(vault_key: &[u8; 32], wrapped: &[u8]) -> Result<PrivateKeyMaterial> {
    let plain = envelope::decrypt(vault_key, wrapped, AAD_PRIVATE_KEYS)?;
    let plain = Zeroizing::new(plain);
    let keys: PrivateKeys = serde_json::from_slice(&plain).map_err(|e| CoreError::Serde(e.to_string()))?;
    Ok(PrivateKeyMaterial {
        x25519: StaticSecret::from(keys.x25519),
        ed25519: SigningKey::from_bytes(&keys.ed25519),
        mlkem_dk: keys.mlkem.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_and_unwrap() {
        let vault = [5u8; 32];
        let kp = generate(&vault).unwrap();
        assert_eq!(kp.public_key.len(), 32);
        assert_eq!(kp.signing_public_key.len(), 32);
        // Correct vault key unwraps; wrong one fails.
        assert!(unwrap_private_keys(&vault, &kp.wrapped_private_keys).is_ok());
        assert!(unwrap_private_keys(&[6u8; 32], &kp.wrapped_private_keys).is_err());
    }

    fn h32(s: &str) -> [u8; 32] {
        hex::decode(s).unwrap().try_into().unwrap()
    }

    // RFC 7748 §6.1 — X25519 Diffie-Hellman test vector.
    #[test]
    fn rfc7748_x25519_known_answer() {
        let a_secret = StaticSecret::from(h32("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a"));
        let b_secret = StaticSecret::from(h32("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb"));
        let a_pub = "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a";
        let b_pub = "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f";
        let shared = "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742";

        assert_eq!(hex::encode(XPublicKey::from(&a_secret).to_bytes()), a_pub);
        assert_eq!(hex::encode(XPublicKey::from(&b_secret).to_bytes()), b_pub);
        // Both sides derive the same shared secret.
        assert_eq!(hex::encode(a_secret.diffie_hellman(&XPublicKey::from(h32(b_pub))).to_bytes()), shared);
        assert_eq!(hex::encode(b_secret.diffie_hellman(&XPublicKey::from(h32(a_pub))).to_bytes()), shared);
    }

    // RFC 8032 §7.1 — Ed25519 TEST 1 (empty message).
    #[test]
    fn rfc8032_ed25519_test1() {
        use ed25519_dalek::{Signer, Verifier, VerifyingKey};
        let seed = h32("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
        let pub_exp = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
        let sig_exp = "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b";

        let signing = SigningKey::from_bytes(&seed);
        assert_eq!(hex::encode(signing.verifying_key().to_bytes()), pub_exp);

        let sig = signing.sign(b"");
        assert_eq!(hex::encode(sig.to_bytes()), sig_exp);

        let vk = VerifyingKey::from_bytes(&h32(pub_exp)).unwrap();
        assert!(vk.verify(b"", &sig).is_ok());
    }
}
