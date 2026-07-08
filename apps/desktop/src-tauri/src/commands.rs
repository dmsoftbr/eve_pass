//! Tauri command surface — the Rust↔React boundary (PRD Fase 1 §6).
//!
//! Rules enforced here:
//! - Keys never cross to JS. `create_account` returns *wrapped* keys (base64) for
//!   the JS side to store on Supabase; it never returns the vault key.
//! - Item plaintext crosses only for display/edit (`get_item`, `list_items`).
//! - `copy_field` decrypts and writes the clipboard **inside Rust** — the secret
//!   value never reaches JS.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use base64::Engine;
use evepass_core::{
    begin_login as core_begin_login, create_account as core_create_account, generate_password,
    password_score, sha1_hex, totp_now, GenOptions, KdfParams, Session,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::cache::{Cache, CacheRow, Kind};
use crate::settings::Settings;
use crate::state::{AppState, Inner};

type CmdResult<T> = Result<T, String>;

fn b64e(b: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(b)
}
fn b64d(s: &str) -> CmdResult<Vec<u8>> {
    base64::engine::general_purpose::STANDARD.decode(s).map_err(|e| format!("base64: {e}"))
}

/// Random UUID v4 string, for new item/folder ids created offline.
fn new_uuid() -> String {
    let mut b = [0u8; 16];
    getrandom::getrandom(&mut b).expect("os rng");
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // variant
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

/// Monotonic, lexically-sortable local timestamp (epoch millis, zero-padded).
/// Server rows keep their own ISO timestamp; used only as an LWW tie-break.
fn now_stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    format!("{ms:015}")
}

// ── DTOs crossing to JS ─────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct NewAccountJs {
    pub kdf_salt_b64: String,
    pub kdf_params: KdfParams,
    pub auth_key_b64: String,
    pub wrapped_vault_key_b64: String,
    pub wrapped_vault_key_recovery_b64: String,
    pub recovery_code: String,
    pub public_key_b64: String,
    pub signing_public_key_b64: String,
    pub wrapped_private_keys_b64: String,
}

#[derive(Serialize)]
pub struct BeginLoginJs {
    pub auth_key_b64: String,
    pub login_token: String,
}

#[derive(Serialize)]
pub struct ItemView {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub title: String,
    pub username: String,
    pub url: String,
    pub has_totp: bool,
    pub folders: Vec<String>,
    pub tags: Vec<String>,
    pub revision: i64,
    pub updated_at: String,
    pub collection_id: Option<String>,
}

#[derive(Serialize)]
pub struct FolderView {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub revision: i64,
}

#[derive(Serialize)]
pub struct Saved {
    pub id: String,
    pub envelope_b64: String,
    pub revision: i64,
    pub deleted: bool,
    #[serde(default)]
    pub collection_id: Option<String>,
}

#[derive(Deserialize)]
pub struct RemoteRow {
    pub kind: String,
    pub id: String,
    pub envelope_b64: String,
    pub revision: i64,
    #[serde(default)]
    pub updated_at: String,
    pub deleted: bool,
    #[serde(default)]
    pub collection_id: Option<String>,
}

#[derive(Serialize, Default)]
pub struct SyncResult {
    pub updated: Vec<String>,
    pub conflicts: Vec<String>,
}

#[derive(Serialize)]
pub struct PendingRow {
    pub kind: String,
    pub id: String,
    pub envelope_b64: String,
    pub revision: i64,
    pub deleted: bool,
    pub collection_id: Option<String>,
}

// ── State / auth commands ───────────────────────────────────────────────────

#[tauri::command]
pub fn vault_status(state: State<AppState>) -> String {
    let inner = state.inner.lock().unwrap();
    if inner.session.is_some() { "unlocked".into() } else { "locked".into() }
}

#[tauri::command]
pub fn create_account(password: String) -> CmdResult<NewAccountJs> {
    let a = core_create_account(password).map_err(|e| e.to_string())?;
    Ok(NewAccountJs {
        kdf_salt_b64: b64e(&a.kdf_salt),
        kdf_params: a.kdf_params,
        auth_key_b64: a.auth_key_b64,
        wrapped_vault_key_b64: b64e(&a.wrapped_vault_key),
        wrapped_vault_key_recovery_b64: b64e(&a.wrapped_vault_key_recovery),
        recovery_code: a.recovery_code,
        public_key_b64: b64e(&a.public_key),
        signing_public_key_b64: b64e(&a.signing_public_key),
        wrapped_private_keys_b64: b64e(&a.wrapped_private_keys),
    })
}

#[tauri::command]
pub fn begin_login(
    state: State<AppState>,
    password: String,
    salt_b64: String,
    params: KdfParams,
) -> CmdResult<BeginLoginJs> {
    let salt = b64d(&salt_b64)?;
    let ctx = core_begin_login(&password, &salt, &params).map_err(|e| e.to_string())?;
    let auth_key_b64 = ctx.auth_key_b64().to_string();
    let token = b64e(&{
        let mut t = [0u8; 16];
        getrandom::getrandom(&mut t).map_err(|e| e.to_string())?;
        t
    });
    let mut inner = state.inner.lock().unwrap();
    inner.pending = Some((token.clone(), ctx));
    Ok(BeginLoginJs { auth_key_b64, login_token: token })
}

#[tauri::command]
pub fn complete_login(
    app: AppHandle,
    state: State<AppState>,
    login_token: String,
    wrapped_vault_key_b64: String,
    wrapped_private_keys_b64: String,
    user_id: String,
) -> CmdResult<()> {
    let wrapped = b64d(&wrapped_vault_key_b64)?;
    let wrapped_priv = b64d(&wrapped_private_keys_b64)?;
    {
        let mut inner = state.inner.lock().unwrap();
        let (token, ctx) = inner.pending.take().ok_or("no login in progress")?;
        if token != login_token {
            return Err("stale login token".into());
        }
        let session = ctx.complete(&wrapped).map_err(|e| e.to_string())?;
        // Unwrap the X25519/Ed25519 keys into the session for collection sharing.
        session.load_private_keys(wrapped_priv).map_err(|e| e.to_string())?;

        // Open (or create) this user's local cache.
        std::fs::create_dir_all(&state.app_dir).map_err(|e| e.to_string())?;
        let cache_path = state.app_dir.join(format!("cache-{user_id}.sqlite"));
        let cache = Cache::open(&cache_path).map_err(|e| e.to_string())?;

        inner.session = Some(session);
        inner.cache = Some(cache);
        inner.user_id = Some(user_id);
        inner.last_activity = Some(Instant::now());
    }
    crate::update_tray(&app, true);
    Ok(())
}

/// Lock the vault: drop the Session (zeroizes the vault key), clear caches, and
/// reflect the state in the tray. Shared by the `lock` command, the tray menu,
/// and the inactivity auto-lock.
pub fn perform_lock(app: &AppHandle, state: &AppState) {
    {
        let mut inner = state.inner.lock().unwrap();
        inner.session = None; // Session drops → vaultKey zeroized
        inner.cache = None;
        inner.user_id = None;
        inner.pending = None;
        inner.breach_index = None;
        inner.last_activity = None;
    }
    crate::update_tray(app, false);
}

#[tauri::command]
pub fn lock(app: AppHandle, state: State<AppState>) {
    perform_lock(&app, &state);
}

// ── helpers to borrow session + cache under one lock ────────────────────────

/// Run `f` with the unlocked session and cache, or error if locked.
fn with_vault<T>(
    state: &State<AppState>,
    f: impl FnOnce(&Arc<Session>, &Cache) -> CmdResult<T>,
) -> CmdResult<T> {
    let inner = state.inner.lock().unwrap();
    let session = inner.session.as_ref().ok_or("vault is locked")?;
    let cache = inner.cache.as_ref().ok_or("vault is locked")?;
    f(session, cache)
}

/// Decrypt a cached item row to JSON, using the collection key when it is shared.
fn decrypt_row(session: &Arc<Session>, row: &CacheRow) -> CmdResult<String> {
    let blob = evepass_core::Blob { envelope: row.envelope.clone() };
    match &row.collection_id {
        Some(cid) => session.decrypt_collection_item(cid.clone(), blob).map_err(|e| e.to_string()),
        None => session.decrypt_item(blob).map_err(|e| e.to_string()),
    }
}

// ── item commands ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_items(state: State<AppState>) -> CmdResult<Vec<ItemView>> {
    with_vault(&state, |session, cache| {
        let rows = cache.list(Kind::Item).map_err(|e| e.to_string())?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let json = decrypt_row(session, &row)?;
            let v: Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            out.push(ItemView {
                id: row.id,
                type_: v["type"].as_str().unwrap_or("login").to_string(),
                title: v["title"].as_str().unwrap_or("").to_string(),
                username: v["username"].as_str().unwrap_or("").to_string(),
                url: v["url"].as_str().unwrap_or("").to_string(),
                has_totp: v["totp"].as_str().map(|s| !s.is_empty()).unwrap_or(false),
                folders: str_vec(&v["folders"]),
                tags: str_vec(&v["tags"]),
                revision: row.revision,
                updated_at: row.updated_at,
                collection_id: row.collection_id,
            });
        }
        Ok(out)
    })
}

fn str_vec(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

#[tauri::command]
pub fn get_item(state: State<AppState>, id: String) -> CmdResult<String> {
    with_vault(&state, |session, cache| {
        let row = cache.get(Kind::Item, &id).map_err(|e| e.to_string())?.ok_or("item not found")?;
        decrypt_row(session, &row)
    })
}

/// Create/update an item. Pass `collection_id` to store it in a shared
/// collection (encrypted with that collection's key instead of the vault key).
#[tauri::command]
pub fn save_item(
    state: State<AppState>,
    id: Option<String>,
    item_json: String,
    collection_id: Option<String>,
) -> CmdResult<Saved> {
    with_vault(&state, |session, cache| {
        let blob = match &collection_id {
            Some(cid) => session.encrypt_collection_item(cid.clone(), item_json).map_err(|e| e.to_string())?,
            None => session.encrypt_item(item_json).map_err(|e| e.to_string())?,
        };
        let id = id.unwrap_or_else(new_uuid);
        let prev = cache.get(Kind::Item, &id).map_err(|e| e.to_string())?;
        let revision = prev.map(|r| r.revision + 1).unwrap_or(1);
        let row = CacheRow {
            id: id.clone(),
            envelope: blob.envelope.clone(),
            revision,
            updated_at: now_stamp(),
            deleted: false,
            dirty: true,
            collection_id: collection_id.clone(),
        };
        cache.upsert(Kind::Item, &row).map_err(|e| e.to_string())?;
        Ok(Saved { id, envelope_b64: b64e(&blob.envelope), revision, deleted: false, collection_id })
    })
}

#[tauri::command]
pub fn delete_item(state: State<AppState>, id: String) -> CmdResult<Saved> {
    with_vault(&state, |_session, cache| {
        let prev = cache.get(Kind::Item, &id).map_err(|e| e.to_string())?.ok_or("item not found")?;
        let revision = prev.revision + 1;
        let row = CacheRow {
            id: id.clone(),
            envelope: prev.envelope.clone(),
            revision,
            updated_at: now_stamp(),
            deleted: true,
            dirty: true,
            collection_id: prev.collection_id.clone(),
        };
        cache.upsert(Kind::Item, &row).map_err(|e| e.to_string())?;
        Ok(Saved { id, envelope_b64: b64e(&prev.envelope), revision, deleted: true, collection_id: prev.collection_id })
    })
}

#[tauri::command]
pub fn mark_synced(state: State<AppState>, kind: String, id: String, revision: i64) -> CmdResult<()> {
    let kind = Kind::parse(&kind).ok_or("bad kind")?;
    with_vault(&state, |_s, cache| cache.mark_synced(kind, &id, revision).map_err(|e| e.to_string()))
}

/// Decrypt an item and copy one field to the OS clipboard — inside Rust, so the
/// secret value never reaches the JS layer. Schedules a clipboard clear after
/// `clipboard_clear_seconds`, but only clears if the value is still there (never
/// clobbers something the user copied afterwards).
#[tauri::command]
pub fn copy_field(app: AppHandle, state: State<AppState>, id: String, field: String) -> CmdResult<()> {
    let value = with_vault(&state, |session, cache| {
        let row = cache.get(Kind::Item, &id).map_err(|e| e.to_string())?.ok_or("item not found")?;
        let json = decrypt_row(session, &row)?;
        let v: Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        Ok(v.get(&field).and_then(|x| x.as_str()).unwrap_or("").to_string())
    })?;
    let clear_secs = state.inner.lock().unwrap().settings.clipboard_clear_seconds;

    app.clipboard().write_text(value.clone()).map_err(|e| e.to_string())?;

    if clear_secs > 0 && !value.is_empty() {
        let app2 = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(clear_secs as u64));
            if let Ok(current) = app2.clipboard().read_text() {
                if current == value {
                    let _ = app2.clipboard().write_text(String::new());
                }
            }
        });
    }
    Ok(())
}

// ── folder commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_folders(state: State<AppState>) -> CmdResult<Vec<FolderView>> {
    with_vault(&state, |session, cache| {
        let rows = cache.list(Kind::Folder).map_err(|e| e.to_string())?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let json = session
                .decrypt_folder(evepass_core::Blob { envelope: row.envelope })
                .map_err(|e| e.to_string())?;
            let v: Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            out.push(FolderView {
                id: row.id,
                name: v["name"].as_str().unwrap_or("").to_string(),
                parent_id: v["parent_id"].as_str().map(String::from),
                revision: row.revision,
            });
        }
        Ok(out)
    })
}

#[tauri::command]
pub fn save_folder(
    state: State<AppState>,
    id: Option<String>,
    name: String,
    parent_id: Option<String>,
) -> CmdResult<Saved> {
    with_vault(&state, |session, cache| {
        let folder_json = serde_json::json!({ "name": name, "parent_id": parent_id }).to_string();
        let blob = session.encrypt_folder(folder_json).map_err(|e| e.to_string())?;
        let id = id.unwrap_or_else(new_uuid);
        let prev = cache.get(Kind::Folder, &id).map_err(|e| e.to_string())?;
        let revision = prev.map(|r| r.revision + 1).unwrap_or(1);
        let row = CacheRow {
            id: id.clone(),
            envelope: blob.envelope.clone(),
            revision,
            updated_at: now_stamp(),
            deleted: false,
            dirty: true,
            collection_id: None,
        };
        cache.upsert(Kind::Folder, &row).map_err(|e| e.to_string())?;
        Ok(Saved { id, envelope_b64: b64e(&blob.envelope), revision, deleted: false, collection_id: None })
    })
}

#[tauri::command]
pub fn delete_folder(state: State<AppState>, id: String) -> CmdResult<Saved> {
    with_vault(&state, |_session, cache| {
        let prev = cache.get(Kind::Folder, &id).map_err(|e| e.to_string())?.ok_or("folder not found")?;
        let revision = prev.revision + 1;
        let row = CacheRow {
            id: id.clone(),
            envelope: prev.envelope.clone(),
            revision,
            updated_at: now_stamp(),
            deleted: true,
            dirty: true,
            collection_id: None,
        };
        cache.upsert(Kind::Folder, &row).map_err(|e| e.to_string())?;
        Ok(Saved { id, envelope_b64: b64e(&prev.envelope), revision, deleted: true, collection_id: None })
    })
}

// ── sync ────────────────────────────────────────────────────────────────────

/// Reconcile remote rows into the local cache (PRD §8). Runs entirely in Rust so
/// the conflict-copy title is re-encrypted with the vault key here.
#[tauri::command]
pub fn apply_remote_changes(state: State<AppState>, rows: Vec<RemoteRow>) -> CmdResult<SyncResult> {
    with_vault(&state, |session, cache| {
        let mut result = SyncResult::default();
        for r in rows {
            let kind = Kind::parse(&r.kind).ok_or("bad kind")?;
            let envelope = b64d(&r.envelope_b64)?;
            let local = cache.get(kind, &r.id).map_err(|e| e.to_string())?;

            match local {
                // New locally → insert as canonical.
                None => {
                    cache
                        .upsert(kind, &canonical(&r.id, &envelope, r.revision, &r.updated_at, r.deleted, r.collection_id.clone()))
                        .map_err(|e| e.to_string())?;
                    result.updated.push(r.id);
                }
                // Clean local → last-write-wins.
                Some(local) if !local.dirty => {
                    let remote_wins = r.revision > local.revision
                        || (r.revision == local.revision && r.updated_at > local.updated_at);
                    if remote_wins {
                        cache
                            .upsert(kind, &canonical(&r.id, &envelope, r.revision, &r.updated_at, r.deleted, r.collection_id.clone()))
                            .map_err(|e| e.to_string())?;
                        result.updated.push(r.id);
                    }
                }
                // Dirty local + remote advanced → conflict copy, remote canonical.
                Some(local) if r.revision > local.revision => {
                    make_conflict_copy(session, cache, kind, &local)?;
                    cache
                        .upsert(kind, &canonical(&r.id, &envelope, r.revision, &r.updated_at, r.deleted, r.collection_id.clone()))
                        .map_err(|e| e.to_string())?;
                    result.conflicts.push(r.id);
                }
                // Dirty local is ahead → keep local, ignore remote.
                Some(_) => {}
            }
        }
        Ok(result)
    })
}

fn canonical(
    id: &str,
    envelope: &[u8],
    revision: i64,
    updated_at: &str,
    deleted: bool,
    collection_id: Option<String>,
) -> CacheRow {
    CacheRow {
        id: id.to_string(),
        envelope: envelope.to_vec(),
        revision,
        updated_at: updated_at.to_string(),
        deleted,
        dirty: false,
        collection_id,
    }
}

/// Re-encrypt the dirty local content under a new id with a "(conflito)" title,
/// so the offline edit is preserved instead of silently lost.
fn make_conflict_copy(session: &Arc<Session>, cache: &Cache, kind: Kind, local: &CacheRow) -> CmdResult<()> {
    let new_id = new_uuid();
    let new_envelope = match kind {
        Kind::Item => {
            let json = decrypt_row(session, local)?;
            let mut v: Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            let title = v["title"].as_str().unwrap_or("").to_string();
            v["title"] = Value::String(format!("{title} (conflito)"));
            match &local.collection_id {
                Some(cid) => session
                    .encrypt_collection_item(cid.clone(), v.to_string())
                    .map_err(|e| e.to_string())?
                    .envelope,
                None => session.encrypt_item(v.to_string()).map_err(|e| e.to_string())?.envelope,
            }
        }
        Kind::Folder => {
            let json = session
                .decrypt_folder(evepass_core::Blob { envelope: local.envelope.clone() })
                .map_err(|e| e.to_string())?;
            let mut v: Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            let name = v["name"].as_str().unwrap_or("").to_string();
            v["name"] = Value::String(format!("{name} (conflito)"));
            session.encrypt_folder(v.to_string()).map_err(|e| e.to_string())?.envelope
        }
    };
    let row = CacheRow {
        id: new_id,
        envelope: new_envelope,
        revision: 1,
        updated_at: now_stamp(),
        deleted: false,
        dirty: true,
        collection_id: local.collection_id.clone(),
    };
    cache.upsert(kind, &row).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pending_uploads(state: State<AppState>) -> CmdResult<Vec<PendingRow>> {
    with_vault(&state, |_session, cache| {
        let mut out = Vec::new();
        for kind in [Kind::Item, Kind::Folder] {
            for row in cache.pending(kind).map_err(|e| e.to_string())? {
                out.push(PendingRow {
                    kind: kind.as_str().to_string(),
                    id: row.id,
                    envelope_b64: b64e(&row.envelope),
                    revision: row.revision,
                    deleted: row.deleted,
                    collection_id: row.collection_id,
                });
            }
        }
        Ok(out)
    })
}

// ── utilities ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn gen_password(length: u32, upper: bool, lower: bool, digits: bool, symbols: bool) -> CmdResult<String> {
    generate_password(GenOptions { length, upper, lower, digits, symbols }).map_err(|e| e.to_string())
}

// ═══════════════════════════════ Fase 2 ═══════════════════════════════════════

/// Iterate the cached items (personal + shared), decrypting each. Returns
/// (id, parsed JSON). Used by health/breach/palette, so collection items are
/// covered too (PRD Fase 4 §10).
fn decrypt_all_items(session: &Arc<Session>, cache: &Cache) -> CmdResult<Vec<(String, Value)>> {
    let rows = cache.list(Kind::Item).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let json = decrypt_row(session, &row)?;
        let v: Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        out.push((row.id, v));
    }
    Ok(out)
}

// ── smart views (health) ─────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct HealthReport {
    pub weak: Vec<String>,
    /// Groups of item ids that share the same password.
    pub reused: Vec<Vec<String>>,
    pub no_totp: Vec<String>,
}

/// Weak / reused / no-2FA analysis over the decrypted vault. Passwords never
/// leave Rust — only item ids come back.
#[tauri::command]
pub fn vault_health(state: State<AppState>) -> CmdResult<HealthReport> {
    with_vault(&state, |session, cache| {
        let items = decrypt_all_items(session, cache)?;
        let mut weak = Vec::new();
        let mut no_totp = Vec::new();
        let mut by_pw: HashMap<String, Vec<String>> = HashMap::new();
        for (id, v) in items {
            if v["type"].as_str().unwrap_or("login") != "login" {
                continue;
            }
            let pw = v["password"].as_str().unwrap_or("");
            let has_totp = v["totp"].as_str().map(|s| !s.is_empty()).unwrap_or(false);
            if !has_totp {
                no_totp.push(id.clone());
            }
            if !pw.is_empty() {
                if password_score(pw.to_string()) < 3 || pw.chars().count() < 12 {
                    weak.push(id.clone());
                }
                by_pw.entry(sha1_hex(pw.to_string())).or_default().push(id.clone());
            }
        }
        let reused = by_pw.into_values().filter(|ids| ids.len() >= 2).collect();
        Ok(HealthReport { weak, reused, no_totp })
    })
}

// ── breach (HIBP k-anonymity) ────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct Range {
    pub prefix: String,
    pub body: String,
}

/// Build the SHA-1 index of login passwords and return only the unique 5-char
/// prefixes for the JS side to query HIBP. Full hashes stay in `breach_index`.
#[tauri::command]
pub fn breach_prefixes(state: State<AppState>) -> CmdResult<Vec<String>> {
    let mut inner = state.inner.lock().unwrap();
    // Disjoint borrows: read session/cache, write breach_index, under one lock.
    let Inner { session, cache, breach_index, .. } = &mut *inner;
    let session = session.as_ref().ok_or("vault is locked")?;
    let cache = cache.as_ref().ok_or("vault is locked")?;

    let items = decrypt_all_items(session, cache)?;
    let mut index: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for (id, v) in items {
        if v["type"].as_str().unwrap_or("login") != "login" {
            continue;
        }
        let pw = v["password"].as_str().unwrap_or("");
        if pw.is_empty() {
            continue;
        }
        let hash = sha1_hex(pw.to_string()); // 40 uppercase hex
        let (prefix, suffix) = hash.split_at(5);
        index.entry(prefix.to_string()).or_default().push((suffix.to_string(), id));
    }
    let prefixes = index.keys().cloned().collect();
    *breach_index = Some(index);
    Ok(prefixes)
}

/// Match HIBP range responses against the stored suffixes → breached item ids.
#[tauri::command]
pub fn resolve_breaches(state: State<AppState>, ranges: Vec<Range>) -> CmdResult<Vec<String>> {
    let inner = state.inner.lock().unwrap();
    let index = inner.breach_index.as_ref().ok_or("call breach_prefixes first")?;
    let mut breached: HashSet<String> = HashSet::new();
    for range in ranges {
        let prefix = range.prefix.to_uppercase();
        let Some(entries) = index.get(&prefix) else { continue };
        // HIBP body lines are "SUFFIX35:count".
        let hits: HashSet<String> = range
            .body
            .lines()
            .filter_map(|line| {
                let mut parts = line.trim().split(':');
                let suffix = parts.next()?.to_uppercase();
                let count: u64 = parts.next()?.trim().parse().ok()?;
                (count > 0).then_some(suffix)
            })
            .collect();
        for (suffix, id) in entries {
            if hits.contains(&suffix.to_uppercase()) {
                breached.insert(id.clone());
            }
        }
    }
    Ok(breached.into_iter().collect())
}

// ── live TOTP ────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct TotpCodeJs {
    pub code: String,
    pub seconds_remaining: u32,
}

#[tauri::command]
pub fn item_totp(state: State<AppState>, id: String) -> CmdResult<TotpCodeJs> {
    let secret = with_vault(&state, |session, cache| {
        let row = cache.get(Kind::Item, &id).map_err(|e| e.to_string())?.ok_or("item not found")?;
        let json = session
            .decrypt_item(evepass_core::Blob { envelope: row.envelope })
            .map_err(|e| e.to_string())?;
        let v: Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        let totp = v["totp"].as_str().unwrap_or("").to_string();
        if totp.is_empty() {
            return Err("item has no TOTP".into());
        }
        Ok(totp)
    })?;
    // Accept a full otpauth URI or a bare base32 secret.
    let uri = if secret.starts_with("otpauth://") {
        secret
    } else {
        format!("otpauth://totp/EVEPass?secret={}", secret.replace(' ', ""))
    };
    let code = totp_now(uri).map_err(|e| e.to_string())?;
    Ok(TotpCodeJs { code: code.code, seconds_remaining: code.seconds_remaining })
}

// ── command palette search ───────────────────────────────────────────────────

#[derive(Serialize)]
pub struct PaletteHit {
    pub id: String,
    pub title: String,
    pub username: String,
    pub has_totp: bool,
}

/// Subsequence fuzzy score: `None` if `needle` isn't an ordered subsequence of
/// `haystack`; higher score for earlier and more contiguous matches.
fn fuzzy_score(needle: &str, haystack: &str) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    let mut score = 0i32;
    let mut last = None::<usize>;
    let hay: Vec<char> = haystack.chars().collect();
    let mut hi = 0usize;
    for nc in needle.chars() {
        let mut found = None;
        while hi < hay.len() {
            if hay[hi] == nc {
                found = Some(hi);
                break;
            }
            hi += 1;
        }
        let pos = found?;
        score += 10;
        if let Some(prev) = last {
            if pos == prev + 1 {
                score += 15; // contiguous bonus
            }
        }
        score -= pos as i32; // earlier is better
        last = Some(pos);
        hi = pos + 1;
    }
    Some(score)
}

#[tauri::command]
pub fn palette_search(state: State<AppState>, query: String) -> CmdResult<Vec<PaletteHit>> {
    with_vault(&state, |session, cache| {
        let items = decrypt_all_items(session, cache)?;
        let q = query.trim().to_lowercase();
        let mut scored: Vec<(i32, PaletteHit)> = Vec::new();
        for (id, v) in items {
            let title = v["title"].as_str().unwrap_or("");
            let username = v["username"].as_str().unwrap_or("");
            let url = v["url"].as_str().unwrap_or("");
            let hay = format!("{title} {username} {url}").to_lowercase();
            let score = fuzzy_score(&q, &hay);
            if let Some(s) = score {
                scored.push((
                    s,
                    PaletteHit {
                        id,
                        title: title.to_string(),
                        username: username.to_string(),
                        has_totp: v["totp"].as_str().map(|t| !t.is_empty()).unwrap_or(false),
                    },
                ));
            }
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        Ok(scored.into_iter().take(20).map(|(_, h)| h).collect())
    })
}

// ── import (JS parses plaintext; Rust encrypts) ──────────────────────────────

#[tauri::command]
pub fn save_items_batch(state: State<AppState>, items_json: Vec<String>) -> CmdResult<Vec<Saved>> {
    with_vault(&state, |session, cache| {
        let mut out = Vec::with_capacity(items_json.len());
        for j in items_json {
            let blob = session.encrypt_item(j).map_err(|e| e.to_string())?;
            let id = new_uuid();
            let row = CacheRow {
                id: id.clone(),
                envelope: blob.envelope.clone(),
                revision: 1,
                updated_at: now_stamp(),
                deleted: false,
                dirty: true,
                collection_id: None,
            };
            cache.upsert(Kind::Item, &row).map_err(|e| e.to_string())?;
            out.push(Saved { id, envelope_b64: b64e(&blob.envelope), revision: 1, deleted: false, collection_id: None });
        }
        Ok(out)
    })
}

#[tauri::command]
pub fn save_folders_batch(
    state: State<AppState>,
    folders: Vec<(String, Option<String>)>,
) -> CmdResult<Vec<Saved>> {
    with_vault(&state, |session, cache| {
        let mut out = Vec::with_capacity(folders.len());
        for (name, parent_id) in folders {
            let folder_json = serde_json::json!({ "name": name, "parent_id": parent_id }).to_string();
            let blob = session.encrypt_folder(folder_json).map_err(|e| e.to_string())?;
            let id = new_uuid();
            let row = CacheRow {
                id: id.clone(),
                envelope: blob.envelope.clone(),
                revision: 1,
                updated_at: now_stamp(),
                deleted: false,
                dirty: true,
                collection_id: None,
            };
            cache.upsert(Kind::Folder, &row).map_err(|e| e.to_string())?;
            out.push(Saved { id, envelope_b64: b64e(&blob.envelope), revision: 1, deleted: false, collection_id: None });
        }
        Ok(out)
    })
}

// ── settings + activity ──────────────────────────────────────────────────────

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Settings {
    state.inner.lock().unwrap().settings.clone()
}

#[tauri::command]
pub fn set_settings(app: AppHandle, state: State<AppState>, settings: Settings) -> CmdResult<()> {
    settings.save(&state.app_dir).map_err(|e| e.to_string())?;
    state.inner.lock().unwrap().settings = settings.clone();
    crate::apply_settings(&app, &settings);
    Ok(())
}

/// Frontend heartbeat that resets the inactivity auto-lock timer.
#[tauri::command]
pub fn ping_activity(state: State<AppState>) {
    let mut inner = state.inner.lock().unwrap();
    if inner.session.is_some() {
        inner.last_activity = Some(Instant::now());
    }
}

// ═══════════════════════════════ Fase 4 (collections) ═════════════════════════

#[derive(Serialize)]
pub struct NewCollectionJs {
    pub collection_id: String,
    pub name_ciphertext_b64: String,
}

#[derive(Deserialize)]
pub struct MemberRowJs {
    pub collection_id: String,
    pub wrapped_collection_key_b64: String,
    pub sender_signing_pub_b64: String,
}

#[derive(Serialize)]
pub struct PasswordResetJs {
    pub auth_key_b64: String,
    pub wrapped_vault_key_b64: String,
    pub wrapped_vault_key_recovery_b64: String,
    pub recovery_code: String,
}

#[tauri::command]
pub fn create_collection(state: State<AppState>, name: String) -> CmdResult<NewCollectionJs> {
    with_vault(&state, |session, _cache| {
        let nc = session.create_collection(name).map_err(|e| e.to_string())?;
        Ok(NewCollectionJs {
            collection_id: nc.collection_id,
            name_ciphertext_b64: b64e(&nc.name_ciphertext),
        })
    })
}

/// Load the HPKE-wrapped collection keys into the session (call after unlock,
/// with the user's `collection_members` rows).
#[tauri::command]
pub fn load_collection_keys(state: State<AppState>, members: Vec<MemberRowJs>) -> CmdResult<()> {
    with_vault(&state, |session, _cache| {
        let mut rows = Vec::with_capacity(members.len());
        for m in members {
            rows.push(evepass_core::MemberRow {
                collection_id: m.collection_id,
                wrapped_collection_key: b64d(&m.wrapped_collection_key_b64)?,
                sender_signing_pub: b64d(&m.sender_signing_pub_b64)?,
            });
        }
        session.load_collection_keys(rows).map_err(|e| e.to_string())
    })
}

/// HPKE-wrap a collection key for a member's X25519 public key (returns b64).
#[tauri::command]
pub fn wrap_collection_key_for(
    state: State<AppState>,
    collection_id: String,
    recipient_pub_b64: String,
) -> CmdResult<String> {
    let recipient_pub = b64d(&recipient_pub_b64)?;
    with_vault(&state, |session, _cache| {
        let wrapped = session
            .wrap_collection_key_for(collection_id, recipient_pub)
            .map_err(|e| e.to_string())?;
        Ok(b64e(&wrapped))
    })
}

#[tauri::command]
pub fn decrypt_collection_name(
    state: State<AppState>,
    collection_id: String,
    name_ct_b64: String,
) -> CmdResult<String> {
    let name_ct = b64d(&name_ct_b64)?;
    with_vault(&state, |session, _cache| {
        session.decrypt_collection_name(collection_id, name_ct).map_err(|e| e.to_string())
    })
}

/// Rotate a collection's key after revoking a member. Returns the new encrypted
/// name; the JS side then re-uploads the collection's items (re-encrypted) and
/// re-wraps the key for the remaining members.
#[tauri::command]
pub fn rotate_collection_key(
    state: State<AppState>,
    collection_id: String,
    name: String,
) -> CmdResult<String> {
    with_vault(&state, |session, _cache| {
        let name_ct = session.rotate_collection_key(collection_id, name).map_err(|e| e.to_string())?;
        Ok(b64e(&name_ct))
    })
}

#[tauri::command]
pub fn public_key_fingerprint(pub_key_b64: String) -> CmdResult<String> {
    Ok(evepass_core::public_key_fingerprint(b64d(&pub_key_b64)?))
}

/// Remove a deleted collection's items from the local cache (the server-side
/// deletes are done in JS).
#[tauri::command]
pub fn delete_collection_cache(state: State<AppState>, collection_id: String) -> CmdResult<()> {
    with_vault(&state, |_session, cache| {
        cache.delete_collection_items(&collection_id).map_err(|e| e.to_string())
    })
}

/// Unlock the vault via the recovery code (pre-auth, no cache). The recovery
/// screen then calls `reset_password` to set a new password + rotate the code.
#[tauri::command]
pub fn unlock_with_recovery(
    app: AppHandle,
    state: State<AppState>,
    recovery_code: String,
    wrapped_vault_key_recovery_b64: String,
) -> CmdResult<()> {
    let wrapped = b64d(&wrapped_vault_key_recovery_b64)?;
    let session = evepass_core::unlock_with_recovery(recovery_code, wrapped).map_err(|e| e.to_string())?;
    state.inner.lock().unwrap().session = Some(session);
    crate::update_tray(&app, true);
    Ok(())
}

/// Recovery flow (§9): set a new password and rotate the recovery code. The
/// session must be unlocked (e.g. via the recovery code). Preserves collection
/// access (asymmetric keys are wrapped with the unchanged vault key).
#[tauri::command]
pub fn reset_password(
    state: State<AppState>,
    new_password: String,
    salt_b64: String,
    params: KdfParams,
) -> CmdResult<PasswordResetJs> {
    let salt = b64d(&salt_b64)?;
    with_vault(&state, |session, _cache| {
        let r = session.reset_password(new_password, salt, params).map_err(|e| e.to_string())?;
        Ok(PasswordResetJs {
            auth_key_b64: r.auth_key_b64,
            wrapped_vault_key_b64: b64e(&r.wrapped_vault_key),
            wrapped_vault_key_recovery_b64: b64e(&r.wrapped_vault_key_recovery),
            recovery_code: r.recovery_code,
        })
    })
}
