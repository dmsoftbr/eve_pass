//! Recovery Code — the only way back into the vault if the master password is
//! forgotten (there is no Secret Key). 128 bits of entropy, shown once at
//! onboarding as the "emergency kit". Encoded in Crockford base32 (no I/L/O/U
//! to avoid ambiguity), grouped in 5-char blocks for legibility.

use crate::error::{CoreError, Result};
use zeroize::Zeroizing;

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const ENTROPY_BYTES: usize = 16; // 128 bits

/// Generate fresh recovery entropy and its human-facing code. Returns
/// `(entropy, code_string)`; the entropy derives the recovery wrapping key,
/// the code string is displayed to the user.
pub fn generate() -> Result<(Zeroizing<[u8; ENTROPY_BYTES]>, String)> {
    let mut entropy = Zeroizing::new([0u8; ENTROPY_BYTES]);
    getrandom::getrandom(entropy.as_mut()).map_err(|e| CoreError::Random(e.to_string()))?;
    let code = encode(entropy.as_slice());
    Ok((entropy, code))
}

/// Parse a user-entered recovery code back into its 16-byte entropy. Tolerant
/// of dashes, spaces, lowercase, and Crockford's O→0 / I,L→1 confusions.
pub fn parse(code: &str) -> Result<Zeroizing<[u8; ENTROPY_BYTES]>> {
    let mut bits: u32 = 0;
    let mut nbits: u32 = 0;
    let mut out = Vec::with_capacity(ENTROPY_BYTES);
    for ch in code.chars() {
        if ch == '-' || ch.is_whitespace() {
            continue;
        }
        let v = decode_char(ch).ok_or_else(|| CoreError::Invalid(format!("bad recovery char: {ch}")))?;
        bits = (bits << 5) | v as u32;
        nbits += 5;
        if nbits >= 8 {
            nbits -= 8;
            out.push((bits >> nbits) as u8);
        }
    }
    if out.len() < ENTROPY_BYTES {
        return Err(CoreError::Invalid("recovery code too short".into()));
    }
    let mut entropy = Zeroizing::new([0u8; ENTROPY_BYTES]);
    entropy.copy_from_slice(&out[..ENTROPY_BYTES]);
    Ok(entropy)
}

fn encode(bytes: &[u8]) -> String {
    let mut symbols = String::new();
    let mut bits: u32 = 0;
    let mut nbits: u32 = 0;
    for &b in bytes {
        bits = (bits << 8) | b as u32;
        nbits += 8;
        while nbits >= 5 {
            nbits -= 5;
            let idx = ((bits >> nbits) & 0x1f) as usize;
            symbols.push(ALPHABET[idx] as char);
        }
    }
    if nbits > 0 {
        let idx = ((bits << (5 - nbits)) & 0x1f) as usize;
        symbols.push(ALPHABET[idx] as char);
    }
    // Group into 5-char blocks separated by dashes.
    symbols
        .as_bytes()
        .chunks(5)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect::<Vec<_>>()
        .join("-")
}

fn decode_char(ch: char) -> Option<u8> {
    let c = ch.to_ascii_uppercase();
    let c = match c {
        'O' => '0',
        'I' | 'L' => '1',
        'U' => 'V', // Crockford treats U as V-ish; keep decodable
        other => other,
    };
    ALPHABET.iter().position(|&a| a as char == c).map(|p| p as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let (entropy, code) = generate().unwrap();
        let parsed = parse(&code).unwrap();
        assert_eq!(*entropy, *parsed);
    }

    #[test]
    fn tolerant_of_formatting() {
        let (entropy, code) = generate().unwrap();
        let messy = code.to_lowercase().replace('-', " ");
        assert_eq!(*parse(&messy).unwrap(), *entropy);
    }

    #[test]
    fn rejects_short_code() {
        assert!(parse("ABC").is_err());
    }
}
