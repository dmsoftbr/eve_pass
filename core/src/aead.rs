//! Low-level AEAD primitive: XChaCha20-Poly1305 with an explicit 24-byte nonce.
//! Framing (version/alg header) lives in `envelope.rs`; this module only does
//! the raw seal/open so it can be unit-tested against known-answer vectors.

use crate::error::{CoreError, Result};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};

pub const KEY_LEN: usize = 32;
pub const NONCE_LEN: usize = 24;

/// Encrypt `plaintext` with `key` under `nonce`, binding `aad`. Output is
/// `ciphertext || 16-byte Poly1305 tag`.
pub fn seal(key: &[u8; KEY_LEN], nonce: &[u8; NONCE_LEN], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(key.into());
    cipher
        .encrypt(XNonce::from_slice(nonce), Payload { msg: plaintext, aad })
        .map_err(|_| CoreError::Decrypt)
}

/// Decrypt `ciphertext` (which includes the trailing tag). Returns
/// `CoreError::Decrypt` on any authentication failure — never panics.
pub fn open(key: &[u8; KEY_LEN], nonce: &[u8; NONCE_LEN], ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(key.into());
    cipher
        .decrypt(XNonce::from_slice(nonce), Payload { msg: ciphertext, aad })
        .map_err(|_| CoreError::Decrypt)
}

/// A fresh 24-byte nonce from the OS CSPRNG. XChaCha20's 192-bit nonce makes
/// random generation safe (no counter/reuse bookkeeping needed).
pub fn random_nonce() -> Result<[u8; NONCE_LEN]> {
    let mut n = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut n).map_err(|e| CoreError::Random(e.to_string()))?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    // draft-irtf-cfrg-xchacha-03 §A.3.1 — AEAD_XCHACHA20_POLY1305 test vector.
    #[test]
    fn xchacha20poly1305_known_answer() {
        let key: [u8; KEY_LEN] = hex::decode(
            "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f",
        )
        .unwrap()
        .try_into()
        .unwrap();
        let nonce: [u8; NONCE_LEN] =
            hex::decode("404142434445464748494a4b4c4d4e4f5051525354555657").unwrap().try_into().unwrap();
        let aad = hex::decode("50515253c0c1c2c3c4c5c6c7").unwrap();
        let plaintext = hex::decode(
            "4c616469657320616e642047656e746c656d656e206f662074686520636c6173\
             73206f66202739393a204966204920636f756c64206f6666657220796f75206f\
             6e6c79206f6e652074697020666f7220746865206675747572652c2073756e73\
             637265656e20776f756c642062652069742e",
        )
        .unwrap();
        // Expected AEAD output = ciphertext || 16-byte tag.
        let expected = format!(
            "{}{}",
            "bd6d179d3e83d43b9576579493c0e939572a1700252bfaccbed2902c21396cbb\
             731c7f1b0b4aa6440bf3a82f4eda7e39ae64c6708c54c216cb96b72e1213b452\
             2f8c9ba40db5d945b11b69b982c1bb9e3f3fac2bc369488f76b2383565d3fff9\
             21f9664c97637da9768812f615c68b13b52e"
                .replace(['\n', ' '], ""),
            "c0875924c1c7987947deafd8780acf49",
        );

        let out = seal(&key, &nonce, &plaintext, &aad).unwrap();
        assert_eq!(hex::encode(&out), expected);
        // And it round-trips.
        assert_eq!(open(&key, &nonce, &out, &aad).unwrap(), plaintext);
    }
}
