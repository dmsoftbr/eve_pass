//! App state. The security invariant of Fase 1 lives here: the `vaultKey` (inside
//! `Session`) and the transient `encKey` (inside `LoginContext`) live **only** in
//! this Rust process. The React layer never receives key material — only
//! ciphertext envelopes and, for display/edit, item plaintext.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use evepass_core::{LoginContext, Session};

use crate::cache::Cache;
use crate::settings::Settings;

pub struct AppState {
    pub inner: Mutex<Inner>,
    /// Base directory for per-user cache files + settings.
    pub app_dir: PathBuf,
}

#[derive(Default)]
pub struct Inner {
    /// Between `begin_login` and `complete_login`: (opaque token, held encKey).
    pub pending: Option<(String, LoginContext)>,
    /// The unlocked vault, if any.
    pub session: Option<Arc<Session>>,
    /// Local encrypted cache, opened per user on login.
    pub cache: Option<Cache>,
    /// Supabase user id of the logged-in account.
    pub user_id: Option<String>,
    /// App settings (auto-lock, clipboard clear, hotkey, theme…).
    pub settings: Settings,
    /// Last user activity — drives the inactivity auto-lock.
    pub last_activity: Option<Instant>,
    /// HIBP breach index built by `breach_prefixes`: prefix5 → [(suffix35, item_id)].
    /// Full password hashes stay here in Rust; only the 5-char prefixes reach JS.
    pub breach_index: Option<HashMap<String, Vec<(String, String)>>>,
}

impl AppState {
    pub fn new(app_dir: PathBuf, settings: Settings) -> AppState {
        AppState {
            inner: Mutex::new(Inner { settings, ..Inner::default() }),
            app_dir,
        }
    }
}
