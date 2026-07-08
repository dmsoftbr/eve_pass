//! Argon2id master-key derivation + calibration.
//!
//! The single expensive pass in the whole system. Everything downstream
//! (`encKey`, `authKey`) is cheap HKDF over the resulting `masterKey`.

use crate::error::{CoreError, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::Zeroize;

/// KDF parameters, stored in `profiles.kdf_params` (and public `login_params`)
/// so any device can reproduce the derivation. `m` is memory cost in **KiB**.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct KdfParams {
    pub alg: String,
    pub m: u32,
    pub t: u32,
    pub p: u32,
}

/// Memory cost in KiB for the 256 MiB default.
pub const DEFAULT_M_KIB: u32 = 256 * 1024;
pub const DEFAULT_T: u32 = 3;
pub const DEFAULT_P: u32 = 4;
pub const MASTER_KEY_LEN: usize = 32;
pub const SALT_LEN: usize = 16;

impl Default for KdfParams {
    fn default() -> Self {
        KdfParams { alg: "argon2id".into(), m: DEFAULT_M_KIB, t: DEFAULT_T, p: DEFAULT_P }
    }
}

impl KdfParams {
    fn to_argon2(&self) -> Result<Argon2<'static>> {
        if self.alg != "argon2id" {
            return Err(CoreError::Kdf(format!("unsupported KDF alg: {}", self.alg)));
        }
        let params = Params::new(self.m, self.t, self.p, Some(MASTER_KEY_LEN))
            .map_err(|e| CoreError::Kdf(e.to_string()))?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }
}

/// A random 16-byte salt (not secret; stored in the profile / login_params).
pub fn random_salt() -> Result<[u8; SALT_LEN]> {
    let mut s = [0u8; SALT_LEN];
    getrandom::getrandom(&mut s).map_err(|e| CoreError::Random(e.to_string()))?;
    Ok(s)
}

/// `masterKey = Argon2id(password, salt, params)`.
pub fn derive_master_key(password: &str, salt: &[u8], params: &KdfParams) -> Result<[u8; MASTER_KEY_LEN]> {
    if password.is_empty() {
        return Err(CoreError::Invalid("password must not be empty".into()));
    }
    if salt.len() < 8 {
        return Err(CoreError::Invalid("salt too short".into()));
    }
    let argon = params.to_argon2()?;
    let mut out = [0u8; MASTER_KEY_LEN];
    argon
        .hash_password_into(password.as_bytes(), salt, &mut out)
        .map_err(|e| CoreError::Kdf(e.to_string()))?;
    Ok(out)
}

/// Pick `m` (memory cost) so one derivation lands near `target_ms` on this
/// device, keeping `t`/`p` at defaults. Returns params to persist at signup.
pub fn calibrate_kdf(target_ms: u32) -> KdfParams {
    use std::time::Instant;
    let salt = [0x42u8; SALT_LEN];
    let password = "calibration-probe";
    // Probe cost — measure one cheap pass, then scale memory linearly (Argon2id
    // runtime is ~linear in memory at fixed t/p) and clamp to sane bounds.
    let probe = KdfParams { alg: "argon2id".into(), m: 64 * 1024, t: DEFAULT_T, p: DEFAULT_P };
    let start = Instant::now();
    let mut scratch = derive_master_key(password, &salt, &probe).unwrap_or([0u8; MASTER_KEY_LEN]);
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    scratch.zeroize();

    let m = if elapsed <= 0.0 {
        DEFAULT_M_KIB
    } else {
        let scaled = (probe.m as f64) * (target_ms as f64) / elapsed;
        (scaled as u32).clamp(32 * 1024, 1024 * 1024) // 32 MiB .. 1 GiB
    };
    KdfParams { alg: "argon2id".into(), m, t: DEFAULT_T, p: DEFAULT_P }
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 9106 §5.3 Argon2id reference test vector (password/salt/secret/AD all
    // fixed byte patterns, m=32, t=3, p=4). Verifies our Argon2id wiring against
    // the spec, independent of our own default params.
    #[test]
    fn rfc9106_argon2id_vector() {
        use argon2::ParamsBuilder;
        let password = [0x01u8; 32];
        let salt = [0x02u8; 16];
        let secret = [0x03u8; 8];
        let ad = [0x04u8; 12];
        const EXPECTED: &str = "0d640df58d78766c08c037a34a8b53c9d01ef0452d75b65eb52520e96b01e659";

        let ad_field = argon2::AssociatedData::try_from(&ad[..]).expect("valid associated data");
        let params = ParamsBuilder::new()
            .m_cost(32)
            .t_cost(3)
            .p_cost(4)
            .data(ad_field)
            .output_len(32)
            .build()
            .expect("valid argon2 params");
        let argon = Argon2::new_with_secret(&secret, Algorithm::Argon2id, Version::V0x13, params)
            .expect("valid secret");
        let mut out = [0u8; 32];
        argon.hash_password_into(&password, &salt, &mut out).expect("hash");
        assert_eq!(hex::encode(out), EXPECTED);
    }

    #[test]
    fn derive_is_deterministic() {
        // Cheap params so the test stays fast.
        let p = KdfParams { alg: "argon2id".into(), m: 8 * 1024, t: 1, p: 1 };
        let a = derive_master_key("correct horse", b"0123456789abcdef", &p).unwrap();
        let b = derive_master_key("correct horse", b"0123456789abcdef", &p).unwrap();
        assert_eq!(a, b);
        let c = derive_master_key("wrong horse", b"0123456789abcdef", &p).unwrap();
        assert_ne!(a, c);
    }

    #[test]
    fn empty_password_rejected() {
        assert!(matches!(derive_master_key("", b"0123456789abcdef", &KdfParams::default()), Err(CoreError::Invalid(_))));
    }
}
