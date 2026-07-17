//! Fase 5B — post-quantum hybrid key wrapping (X25519 + ML-KEM-768).
//!
//! Only the **asymmetric** layer changes: wrapping a symmetric key (e.g. a
//! `collectionKey`) for a recipient. The at-rest layer (XChaCha20-Poly1305, 256-bit)
//! is already quantum-resistant and is untouched.
//!
//! The wrap is a **hybrid**: an X25519 ECDH and an ML-KEM-768 encapsulation are
//! combined through HKDF, so the wrap is safe as long as *either* KEM holds —
//! defeating harvest-now-decrypt-later. It rides the crypto-agility layer: the
//! wrapped blob starts with a version byte (`0x02` = hybrid), so it coexists with
//! the classical X25519-only wrap and can be migrated lazily.
//!
//! Wired into the live collection-wrap flow (Fase 5B): accounts carry an ML-KEM
//! keypair ([`crate::keypair`]), and [`crate::account::Session::wrap_collection_key_for_pq`]
//! produces a hybrid (v2) wrap that coexists with the classical HPKE (v1) wrap.

use hkdf::Hkdf;
use ml_kem::kem::{Decapsulate, Encapsulate};
use ml_kem::{EncodedSizeUser, KemCore, MlKem768};
use rand_core::OsRng;
use sha2::Sha256;
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::error::{CoreError, Result};
use crate::{envelope, keys::KEY_LEN};

/// Version marker for the hybrid wrap (distinct from the inner AEAD envelope's
/// own version, and from the classical X25519-only collection wrap = implicit v1).
pub const HYBRID_VERSION: u8 = 2;
const X_PUB_LEN: usize = 32;
/// ML-KEM-768 ciphertext length.
const MLKEM_CT_LEN: usize = 1088;
/// Smallest possible hybrid wrap: version + eph X pub + ML-KEM ct + a minimal
/// AEAD envelope (24-byte nonce + 16-byte tag over the 32-byte key).
const HYBRID_MIN_LEN: usize = 1 + X_PUB_LEN + MLKEM_CT_LEN + 24 + 16;
const HKDF_INFO: &[u8] = b"eve/pq-hybrid/v1";
const AAD: &[u8] = b"eve/pq-wrapped-key";

type MlDk = <MlKem768 as KemCore>::DecapsulationKey;
type MlEk = <MlKem768 as KemCore>::EncapsulationKey;

/// A recipient's post-quantum keypair material (encoded for storage).
pub struct MlkemKeypair {
    pub encapsulation_key: Vec<u8>, // public — publish alongside the X25519 pub
    pub decapsulation_key: Zeroizing<Vec<u8>>, // private — wrapped with the vault key
}

/// Generate an ML-KEM-768 keypair.
pub fn generate_mlkem_keypair() -> MlkemKeypair {
    let (dk, ek) = MlKem768::generate(&mut OsRng);
    MlkemKeypair {
        encapsulation_key: ek.as_bytes().to_vec(),
        decapsulation_key: Zeroizing::new(dk.as_bytes().to_vec()),
    }
}

/// Combine the two KEM shared secrets into a 32-byte wrapping key.
fn combine(ss_x: &[u8], ss_mlkem: &[u8]) -> Result<Zeroizing<[u8; KEY_LEN]>> {
    let mut ikm = Zeroizing::new(Vec::with_capacity(ss_x.len() + ss_mlkem.len()));
    ikm.extend_from_slice(ss_x);
    ikm.extend_from_slice(ss_mlkem);
    let hk = Hkdf::<Sha256>::new(None, &ikm);
    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    hk.expand(HKDF_INFO, out.as_mut()).map_err(|e| CoreError::Kdf(format!("hkdf: {e}")))?;
    Ok(out)
}

/// Whether `wrapped` looks like a hybrid (v2) wrap rather than a classical HPKE
/// (v1) collection wrap. The version byte plus the large minimum length make this
/// unambiguous: an HPKE wrap (encapped 32 + short ciphertext + sig) is far shorter
/// than `HYBRID_MIN_LEN`.
pub fn is_hybrid(wrapped: &[u8]) -> bool {
    !wrapped.is_empty() && wrapped[0] == HYBRID_VERSION && wrapped.len() >= HYBRID_MIN_LEN
}

/// Hybrid-wrap a 32-byte `key` for a recipient's (X25519 pub, ML-KEM ek).
/// Layout: version(1) || eph_x_pub(32) || mlkem_ct(1088) || AEAD envelope.
pub fn hybrid_wrap(recipient_x_pub: &[u8; 32], recipient_mlkem_ek: &[u8], key: &[u8; KEY_LEN]) -> Result<Vec<u8>> {
    // X25519: ephemeral ECDH.
    let mut eph_seed = Zeroizing::new([0u8; 32]);
    getrandom::getrandom(eph_seed.as_mut()).map_err(|e| CoreError::Random(e.to_string()))?;
    let eph_secret = StaticSecret::from(*eph_seed);
    let eph_pub = XPublicKey::from(&eph_secret);
    let recip_x = XPublicKey::from(*recipient_x_pub);
    let ss_x = eph_secret.diffie_hellman(&recip_x);

    // ML-KEM: encapsulate to the recipient's encapsulation key.
    let ek = MlEk::from_bytes(
        recipient_mlkem_ek
            .try_into()
            .map_err(|_| CoreError::Invalid("bad ML-KEM encapsulation key length".into()))?,
    );
    let (mlkem_ct, ss_mlkem) = ek.encapsulate(&mut OsRng).map_err(|_| CoreError::Crypto("ml-kem encapsulate".into()))?;

    let wrap_key = combine(ss_x.as_bytes(), &ss_mlkem)?;
    let aead = envelope::encrypt(&wrap_key, key.as_slice(), AAD)?;

    let mut out = Vec::with_capacity(1 + X_PUB_LEN + MLKEM_CT_LEN + aead.len());
    out.push(HYBRID_VERSION);
    out.extend_from_slice(eph_pub.as_bytes());
    out.extend_from_slice(&mlkem_ct);
    out.extend_from_slice(&aead);
    Ok(out)
}

/// Reverse of [`hybrid_wrap`], using the recipient's X25519 private key and
/// ML-KEM decapsulation key.
pub fn hybrid_unwrap(
    recipient_x_priv: &[u8; 32],
    recipient_mlkem_dk: &[u8],
    wrapped: &[u8],
) -> Result<Zeroizing<[u8; KEY_LEN]>> {
    let prefix = 1 + X_PUB_LEN + MLKEM_CT_LEN;
    if wrapped.len() < prefix {
        return Err(CoreError::InvalidEnvelope("hybrid wrap too short".into()));
    }
    if wrapped[0] != HYBRID_VERSION {
        return Err(CoreError::UnsupportedAlg { version: wrapped[0], alg: 0 });
    }
    let eph_pub: [u8; 32] = wrapped[1..1 + X_PUB_LEN].try_into().unwrap();
    let mlkem_ct = &wrapped[1 + X_PUB_LEN..prefix];
    let aead = &wrapped[prefix..];

    // X25519 ECDH with our private key.
    let x_secret = StaticSecret::from(*recipient_x_priv);
    let ss_x = x_secret.diffie_hellman(&XPublicKey::from(eph_pub));

    // ML-KEM decapsulate.
    let dk = MlDk::from_bytes(
        recipient_mlkem_dk
            .try_into()
            .map_err(|_| CoreError::Invalid("bad ML-KEM decapsulation key length".into()))?,
    );
    let ct = mlkem_ct
        .try_into()
        .map_err(|_| CoreError::Invalid("bad ML-KEM ciphertext length".into()))?;
    let ss_mlkem = dk.decapsulate(&ct).map_err(|_| CoreError::Crypto("ml-kem decapsulate".into()))?;

    let wrap_key = combine(ss_x.as_bytes(), &ss_mlkem)?;
    let plain = envelope::decrypt(&wrap_key, aead, AAD)?;
    if plain.len() != KEY_LEN {
        return Err(CoreError::Invalid("unwrapped key wrong length".into()));
    }
    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    out.copy_from_slice(&plain);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn x25519_keypair() -> ([u8; 32], [u8; 32]) {
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed).unwrap();
        let sk = StaticSecret::from(seed);
        let pk = XPublicKey::from(&sk);
        (sk.to_bytes(), pk.to_bytes())
    }

    #[test]
    fn hybrid_round_trip() {
        let (x_priv, x_pub) = x25519_keypair();
        let ml = generate_mlkem_keypair();
        let key = [7u8; KEY_LEN];

        let wrapped = hybrid_wrap(&x_pub, &ml.encapsulation_key, &key).unwrap();
        assert_eq!(wrapped[0], HYBRID_VERSION);
        let out = hybrid_unwrap(&x_priv, &ml.decapsulation_key, &wrapped).unwrap();
        assert_eq!(*out, key);
    }

    #[test]
    fn wrong_mlkem_key_fails() {
        // The PQ half genuinely contributes: a wrong ML-KEM key breaks the unwrap
        // even with the right X25519 key.
        let (x_priv, x_pub) = x25519_keypair();
        let ml = generate_mlkem_keypair();
        let other_ml = generate_mlkem_keypair();
        let wrapped = hybrid_wrap(&x_pub, &ml.encapsulation_key, &[9u8; KEY_LEN]).unwrap();
        assert!(hybrid_unwrap(&x_priv, &other_ml.decapsulation_key, &wrapped).is_err());
    }

    #[test]
    fn wrong_x25519_key_fails() {
        // ...and so does the classical half.
        let (_x_priv, x_pub) = x25519_keypair();
        let (other_x_priv, _) = x25519_keypair();
        let ml = generate_mlkem_keypair();
        let wrapped = hybrid_wrap(&x_pub, &ml.encapsulation_key, &[9u8; KEY_LEN]).unwrap();
        assert!(hybrid_unwrap(&other_x_priv, &ml.decapsulation_key, &wrapped).is_err());
    }

    #[test]
    fn rejects_wrong_version() {
        let (x_priv, _) = x25519_keypair();
        let ml = generate_mlkem_keypair();
        let bogus = vec![1u8; 1 + X_PUB_LEN + MLKEM_CT_LEN + 32];
        assert!(matches!(
            hybrid_unwrap(&x_priv, &ml.decapsulation_key, &bogus),
            Err(CoreError::UnsupportedAlg { .. })
        ));
    }
}
