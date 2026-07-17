//! Native-messaging bridge server (Fase 5A).
//!
//! The browser extension has no vault of its own. Chrome launches a thin
//! `evepass-native-host` binary (see the `native-host/` crate) that forwards the
//! extension's JSON requests over a local Unix socket to *this* server, running
//! inside the desktop app. The vault `Session` and all key material stay in this
//! process; a plaintext credential only crosses back to the extension at
//! `getCredential` — the moment of fill.
//!
//! Socket protocol (newline-delimited JSON; the bridge injects `_origin`):
//! ```text
//!   {type:"status"}                        -> {locked: bool}
//!   {type:"match", domain}                 -> {candidates:[{id,title,username}]}
//!   {type:"getCredential", id}             -> {username, password}
//!   {type:"saveCredential", domain, username, password} -> {ok: bool}
//! ```
//! Access rules: `status` always answers (it's the probe). Everything else
//! requires the vault **unlocked** AND the extension origin **paired** — a
//! first request from an unknown origin emits `host-pair-request` so the user
//! can approve it in the app; the approval persists in settings.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use evepass_core::{extract_credential, match_credentials, MatchItem, Session};

use crate::cache::Kind;
use crate::commands::{decrypt_row, new_uuid, now_stamp};
use crate::state::AppState;

/// `~/.evepass/host.sock` — a stable path both the app and the bridge derive
/// from `$HOME` (independent of the Tauri bundle id), matching the CLI's
/// `~/.evepass` convention so the standalone host binary can find it.
pub fn socket_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    home.join(".evepass").join("host.sock")
}

/// Start the Unix-socket listener in a background thread. Runs for the whole app
/// lifetime; per-request checks enforce unlocked + paired.
pub fn spawn_server(app: &AppHandle) {
    let handle = app.clone();
    std::thread::spawn(move || {
        let path = socket_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::remove_file(&path); // clear a stale socket from a crash
        let listener = match UnixListener::bind(&path) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("evepass host: could not bind {}: {e}", path.display());
                return;
            }
        };
        // Only the current user may connect to the socket.
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        for stream in listener.incoming().flatten() {
            let h = handle.clone();
            std::thread::spawn(move || handle_conn(&h, stream));
        }
    });
}

fn handle_conn(app: &AppHandle, stream: UnixStream) {
    let reader_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut reader = BufReader::new(reader_stream);
    let mut writer = stream;
    let mut line = String::new();
    // The bridge opens a short-lived connection per request, but loop so a
    // future persistent bridge also works.
    while reader.read_line(&mut line).unwrap_or(0) > 0 {
        let req: Value = serde_json::from_str(line.trim()).unwrap_or(Value::Null);
        let resp = handle_request(app, &req);
        let mut out = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".into());
        out.push('\n');
        if writer.write_all(out.as_bytes()).is_err() || writer.flush().is_err() {
            break;
        }
        line.clear();
    }
}

fn handle_request(app: &AppHandle, req: &Value) -> Value {
    let origin = req["_origin"].as_str().unwrap_or("").to_string();
    let kind = req["type"].as_str().unwrap_or("");
    let state = app.state::<AppState>();

    // `status` is the extension's probe: always answer, and nudge the pairing
    // prompt for an unknown origin so the user can approve it.
    if kind == "status" {
        let locked = state.inner.lock().unwrap().session.is_none();
        if !origin.is_empty() && !is_paired(&state, &origin) {
            let _ = app.emit("host-pair-request", &origin);
        }
        return json!({ "locked": locked });
    }

    if origin.is_empty() || !is_paired(&state, &origin) {
        if !origin.is_empty() {
            let _ = app.emit("host-pair-request", &origin);
        }
        return json!({ "error": "pareamento pendente — aprove no app EVEPass" });
    }

    let result = match kind {
        "match" => host_match(app, req["domain"].as_str().unwrap_or("")),
        "getCredential" => host_get(app, req["id"].as_str().unwrap_or("")),
        "saveCredential" => host_save(app, req),
        _ => Err("unknown message".to_string()),
    };
    result.unwrap_or_else(|e| json!({ "error": e }))
}

fn is_paired(state: &AppState, origin: &str) -> bool {
    state.inner.lock().unwrap().settings.paired_origins.iter().any(|o| o == origin)
}

/// Decrypt every cached login into the matcher's view (id/title/username/urls).
fn decrypted_match_items(app: &AppHandle) -> Result<Vec<MatchItem>, String> {
    let state = app.state::<AppState>();
    let inner = state.inner.lock().unwrap();
    let session = inner.session.as_ref().ok_or("vault is locked")?;
    let cache = inner.cache.as_ref().ok_or("vault is locked")?;
    let rows = cache.list(Kind::Item).map_err(|e| e.to_string())?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let json = decrypt_row(session, &row)?;
        let v: Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        let url = v["url"].as_str().unwrap_or("").to_string();
        items.push(MatchItem {
            id: row.id,
            title: v["title"].as_str().unwrap_or("").to_string(),
            username: v["username"].as_str().unwrap_or("").to_string(),
            urls: if url.is_empty() { vec![] } else { vec![url] },
        });
    }
    Ok(items)
}

fn host_match(app: &AppHandle, domain: &str) -> Result<Value, String> {
    let items = decrypted_match_items(app)?;
    let hits = match_credentials(items, domain.to_string());
    let candidates: Vec<Value> = hits
        .into_iter()
        .map(|h| json!({ "id": h.id, "title": h.title, "username": h.username }))
        .collect();
    Ok(json!({ "candidates": candidates }))
}

fn host_get(app: &AppHandle, id: &str) -> Result<Value, String> {
    let state = app.state::<AppState>();
    let inner = state.inner.lock().unwrap();
    let session = inner.session.as_ref().ok_or("vault is locked")?;
    let cache = inner.cache.as_ref().ok_or("vault is locked")?;
    let row = cache
        .get(Kind::Item, id)
        .map_err(|e| e.to_string())?
        .ok_or("item not found")?;
    let json = decrypt_row(session, &row)?;
    let cred = extract_credential(json).map_err(|e| e.to_string())?;
    Ok(json!({ "username": cred.username, "password": cred.password }))
}

fn host_save(app: &AppHandle, req: &Value) -> Result<Value, String> {
    let domain = req["domain"].as_str().unwrap_or("");
    let username = req["username"].as_str().unwrap_or("");
    let password = req["password"].as_str().unwrap_or("");
    if password.is_empty() {
        return Err("empty credential".into());
    }
    let item = json!({
        "type": "login",
        "title": domain,
        "username": username,
        "password": password,
        "url": domain,
        "folders": [],
        "tags": [],
    })
    .to_string();

    let state = app.state::<AppState>();
    {
        let inner = state.inner.lock().unwrap();
        let session: &Arc<Session> = inner.session.as_ref().ok_or("vault is locked")?;
        let cache = inner.cache.as_ref().ok_or("vault is locked")?;
        let blob = session.encrypt_item(item).map_err(|e| e.to_string())?;
        let row = crate::cache::CacheRow {
            id: new_uuid(),
            envelope: blob.envelope,
            revision: 1,
            updated_at: now_stamp(),
            deleted: false,
            dirty: true, // the JS sync queue uploads it on the next push
            collection_id: None,
        };
        cache.upsert(Kind::Item, &row).map_err(|e| e.to_string())?;
    }
    // Ask the UI to flush the sync queue and refresh its list.
    let _ = app.emit("host-item-saved", ());
    Ok(json!({ "ok": true }))
}
