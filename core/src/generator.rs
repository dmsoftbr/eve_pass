//! Password generator. Uses the OS CSPRNG and rejection sampling for an
//! unbiased pick over the selected character classes.

use crate::error::{CoreError, Result};

#[derive(Debug, Clone, uniffi::Record)]
pub struct GenOptions {
    pub length: u32,
    pub upper: bool,
    pub lower: bool,
    pub digits: bool,
    pub symbols: bool,
}

impl Default for GenOptions {
    fn default() -> Self {
        GenOptions { length: 20, upper: true, lower: true, digits: true, symbols: true }
    }
}

const UPPER: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ"; // no I/O
const LOWER: &[u8] = b"abcdefghijkmnpqrstuvwxyz"; // no l/o
const DIGITS: &[u8] = b"23456789"; // no 0/1
const SYMBOLS: &[u8] = b"!@#$%^&*()-_=+[]{};:,.?";

/// Uniform byte in `0..bound` via rejection sampling (no modulo bias).
fn uniform_below(bound: usize) -> Result<usize> {
    debug_assert!(bound > 0 && bound <= 256);
    let limit = 256 - (256 % bound);
    let mut b = [0u8; 1];
    loop {
        getrandom::getrandom(&mut b).map_err(|e| CoreError::Random(e.to_string()))?;
        let v = b[0] as usize;
        if v < limit {
            return Ok(v % bound);
        }
    }
}

pub fn generate(opts: &GenOptions) -> Result<String> {
    let mut pool: Vec<u8> = Vec::new();
    if opts.upper {
        pool.extend_from_slice(UPPER);
    }
    if opts.lower {
        pool.extend_from_slice(LOWER);
    }
    if opts.digits {
        pool.extend_from_slice(DIGITS);
    }
    if opts.symbols {
        pool.extend_from_slice(SYMBOLS);
    }
    if pool.is_empty() {
        return Err(CoreError::Invalid("no character classes selected".into()));
    }
    let len = opts.length.clamp(1, 256) as usize;
    let mut out = String::with_capacity(len);
    for _ in 0..len {
        let idx = uniform_below(pool.len())?;
        out.push(pool[idx] as char);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn respects_length_and_classes() {
        let opts = GenOptions { length: 40, upper: false, lower: false, digits: true, symbols: false };
        let pw = generate(&opts).unwrap();
        assert_eq!(pw.chars().count(), 40);
        assert!(pw.bytes().all(|b| DIGITS.contains(&b)));
    }

    #[test]
    fn empty_pool_errors() {
        let opts = GenOptions { length: 10, upper: false, lower: false, digits: false, symbols: false };
        assert!(matches!(generate(&opts), Err(CoreError::Invalid(_))));
    }

    #[test]
    fn is_random() {
        let opts = GenOptions::default();
        assert_ne!(generate(&opts).unwrap(), generate(&opts).unwrap());
    }
}
