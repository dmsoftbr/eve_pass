//! Mobile / autofill additions (Fase 3).
//!
//! - Credential matching by **eTLD+1** (registrable domain) so `accounts.google.com`
//!   matches an item saved as `mail.google.com`, and multiple URLs per item work.
//! - The biometric path: the native module (Swift/Kotlin) exports the `vaultKey`
//!   from a `Session` once, stores it in the secure enclave (Keychain/Keystore)
//!   under biometric control, and later rebuilds a `Session` from it.
//!
//! ⚠️ `Session::export_vault_key` and [`session_from_vault_key`] hand the raw
//! vault key across the FFI boundary. They exist **only** for the native
//! enclave flow and must **never** be called from the React Native JS layer —
//! the key goes device-enclave ⇄ Rust, not through JS. Everything else keeps the
//! key inside `Session`.

use std::sync::Arc;

use zeroize::Zeroizing;

use crate::account::Session;
use crate::error::{CoreError, Result};
use crate::keys::{Key, KEY_LEN};

/// An item as the autofill matcher needs to see it (already decrypted by the
/// shell/extension, which holds the `Session` + cache).
#[derive(Debug, Clone, uniffi::Record)]
pub struct MatchItem {
    pub id: String,
    pub title: String,
    pub username: String,
    pub urls: Vec<String>,
}

/// A matched credential candidate to show in the autofill UI.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ItemMatch {
    pub id: String,
    pub title: String,
    pub username: String,
}

/// The actual secret the extension returns to the OS once the user picks a match.
#[derive(Debug, Clone, uniffi::Record)]
pub struct Credential {
    pub username: String,
    pub password: String,
}

/// Registrable domain (eTLD+1) of a URL/host/`otpauth`-free string, or `None`
/// if it has no public-suffix-registrable domain (e.g. an Android package name).
fn etld1(input: &str) -> Option<String> {
    let host = host_of(input);
    psl::domain_str(&host).map(|d| d.to_ascii_lowercase())
}

/// Extract a bare host from a URL-ish string: strip scheme, path, port, creds.
fn host_of(input: &str) -> String {
    let s = input.trim();
    let s = s.split("://").last().unwrap_or(s); // drop scheme
    let s = s.split('/').next().unwrap_or(s); // drop path
    let s = s.split('@').last().unwrap_or(s); // drop userinfo
    let s = s.split('?').next().unwrap_or(s);
    let s = s.rsplit_once(':').map(|(h, _)| h).unwrap_or(s); // drop :port
    s.trim().to_ascii_lowercase()
}

/// Match `query` (a domain from the OS autofill request) against the items by
/// eTLD+1. Order is preserved (caller may pre-sort by recency).
#[uniffi::export]
pub fn match_credentials(items: Vec<MatchItem>, query: String) -> Vec<ItemMatch> {
    let target = match etld1(&query) {
        Some(d) => d,
        None => return Vec::new(),
    };
    items
        .into_iter()
        .filter(|it| it.urls.iter().filter_map(|u| etld1(u)).any(|d| d == target))
        .map(|it| ItemMatch { id: it.id, title: it.title, username: it.username })
        .collect()
}

/// Pull the username/password out of a decrypted item JSON (the extension
/// decrypts the chosen item via the `Session`, then calls this).
#[uniffi::export]
pub fn extract_credential(item_json: String) -> Result<Credential> {
    let v: serde_json::Value =
        serde_json::from_str(&item_json).map_err(|e| CoreError::Serde(e.to_string()))?;
    Ok(Credential {
        username: v["username"].as_str().unwrap_or("").to_string(),
        password: v["password"].as_str().unwrap_or("").to_string(),
    })
}

/// Rebuild a `Session` from a raw vault key recovered from the enclave.
/// ⚠️ Native-only (see module docs).
#[uniffi::export]
pub fn session_from_vault_key(vault_key: Vec<u8>) -> Result<Arc<Session>> {
    if vault_key.len() != KEY_LEN {
        return Err(CoreError::Invalid("vault key must be 32 bytes".into()));
    }
    let mut key: Key = Zeroizing::new([0u8; KEY_LEN]);
    key.copy_from_slice(&vault_key);
    Ok(Session::from_vault_key(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, urls: &[&str]) -> MatchItem {
        MatchItem {
            id: id.into(),
            title: id.into(),
            username: "u".into(),
            urls: urls.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn matches_by_etld1_across_subdomains() {
        let items = vec![
            item("g", &["https://mail.google.com/x"]),
            item("gh", &["https://github.com"]),
        ];
        let hits = match_credentials(items, "accounts.google.com".into());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "g");
    }

    #[test]
    fn supports_multiple_urls_per_item() {
        let items = vec![item("multi", &["https://foo.example", "https://app.acme.co.uk/login"])];
        assert_eq!(match_credentials(items.clone(), "acme.co.uk".into()).len(), 1);
        assert_eq!(match_credentials(items, "other.com".into()).len(), 0);
    }

    #[test]
    fn no_match_for_different_domain_or_package() {
        let items = vec![item("g", &["https://google.com"])];
        assert!(match_credentials(items.clone(), "bing.com".into()).is_empty());
        // Android package name has no eTLD+1 → no URL match (Digital Asset Links
        // association is a later refinement).
        assert!(match_credentials(items, "com.google.android.gm".into()).is_empty());
    }

    #[test]
    fn host_parsing_strips_scheme_port_path() {
        assert_eq!(host_of("https://user:pass@Sub.Example.com:8443/path?q=1"), "sub.example.com");
    }

    #[test]
    fn extract_credential_reads_fields() {
        let c = extract_credential(r#"{"type":"login","title":"T","username":"me","password":"p"}"#.into())
            .unwrap();
        assert_eq!(c.username, "me");
        assert_eq!(c.password, "p");
    }

    #[test]
    fn session_from_vault_key_round_trips() {
        let acct = crate::account::create_account("pw".into()).unwrap();
        let session = crate::account::unlock(
            "pw".into(),
            acct.kdf_salt.clone(),
            acct.kdf_params.clone(),
            acct.wrapped_vault_key.clone(),
        )
        .unwrap();
        let raw = session.export_vault_key();
        assert_eq!(raw.len(), 32);
        // A fresh Session from the exported key decrypts what the first one wrote.
        let blob = session.encrypt_item(r#"{"type":"login","title":"X"}"#.into()).unwrap();
        let session2 = session_from_vault_key(raw).unwrap();
        assert!(session2.decrypt_item(blob).is_ok());
    }

    #[test]
    fn session_from_bad_length_errors() {
        assert!(session_from_vault_key(vec![0u8; 16]).is_err());
    }
}
