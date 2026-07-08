//! Password-health primitives used by the desktop smart-views and breach check.
//! These are pure helpers; the vault iteration (decrypt each item, group, etc.)
//! happens in the shell, which has the cache. Full password hashes computed here
//! stay in the shell's Rust process — only 5-char k-anonymity prefixes ever go to
//! the network.

use sha1::{Digest, Sha1};

/// zxcvbn strength score, 0 (weakest) … 4 (strongest). Empty password → 0.
#[uniffi::export]
pub fn password_score(password: String) -> u8 {
    if password.is_empty() {
        return 0;
    }
    u8::from(zxcvbn::zxcvbn(&password, &[]).score())
}

/// Uppercase full SHA-1 hex of a password (40 chars). Used to group reused
/// passwords and to build HIBP range queries. Never sent to JS/network whole.
#[uniffi::export]
pub fn sha1_hex(password: String) -> String {
    let mut hasher = Sha1::new();
    hasher.update(password.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(40);
    for b in digest {
        out.push_str(&format!("{b:02X}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weak_scores_low_strong_scores_high() {
        assert!(password_score("password".into()) < 3);
        assert!(password_score("123456".into()) < 2);
        assert!(password_score("Tr0ub4dour&3xpl-Zebra!Quokka".into()) >= 3);
    }

    #[test]
    fn sha1_of_password_matches_known_hibp_hash() {
        // "password" → 5BAA61E4C9B93F3F0682250B6CF8331B7EE68FD8 (well-known HIBP value).
        assert_eq!(sha1_hex("password".into()), "5BAA61E4C9B93F3F0682250B6CF8331B7EE68FD8");
    }

    #[test]
    fn empty_password_scores_zero() {
        assert_eq!(password_score("".into()), 0);
    }
}
