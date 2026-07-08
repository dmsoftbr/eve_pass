//! TOTP generation from an `otpauth://` URI. In Fase 0 this is just the core
//! primitive; live polling in the UI comes in Fase 2.

use crate::error::{CoreError, Result};
use totp_rs::TOTP;

#[derive(Debug, Clone, uniffi::Record)]
pub struct TotpCode {
    pub code: String,
    pub seconds_remaining: u32,
}

/// Current TOTP code + seconds until it rolls over, from an otpauth URI.
pub fn totp_now(otpauth_uri: &str) -> Result<TotpCode> {
    let totp = TOTP::from_url(otpauth_uri).map_err(|e| CoreError::Invalid(format!("bad otpauth uri: {e}")))?;
    let seconds = system_unix_time()?;
    let code = totp
        .generate(seconds)
        .into();
    let step = totp.step.max(1);
    let seconds_remaining = (step - (seconds % step)) as u32;
    Ok(TotpCode { code, seconds_remaining })
}

fn system_unix_time() -> Result<u64> {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| CoreError::Invalid(format!("system clock before epoch: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    // otpauth URI with a 160-bit base32 secret (totp-rs requires >= 128 bits).
    const URI: &str =
        "otpauth://totp/EVEPass:diogo?secret=JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP&issuer=EVEPass";

    #[test]
    fn generates_six_digits() {
        let c = totp_now(URI).unwrap();
        assert_eq!(c.code.len(), 6);
        assert!(c.code.chars().all(|ch| ch.is_ascii_digit()));
        assert!(c.seconds_remaining <= 30 && c.seconds_remaining >= 1);
    }

    #[test]
    fn rejects_bad_uri() {
        assert!(totp_now("not-a-uri").is_err());
    }
}
