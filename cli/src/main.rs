//! evepass-cli — a test harness that exercises `evepass-core` against Supabase,
//! proving the zero-knowledge flow end to end. All networking lives in
//! `supabase.rs`; all cryptography lives in the core. Only ciphertext (bytea
//! envelopes) is ever sent to or read from the server.

mod session;
mod supabase;

use std::sync::Arc;

use anyhow::{Context, Result};
use base64::Engine;
use clap::{Parser, Subcommand};
use evepass_core::{
    auth_key_for_login, create_account, generate_password, unlock, unlock_with_recovery, GenOptions,
    KdfParams, Session,
};
use serde_json::{json, Value};

use session::SessionFile;
use supabase::Supabase;

#[derive(Parser)]
#[command(name = "evepass", about = "EVEPass zero-knowledge test CLI (Fase 0)")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create an account and store its profile on Supabase.
    Signup { email: String },
    /// Log in (prelogin dance) and cache the session locally.
    Login { email: String },
    /// Log out (clear the local session).
    Logout,
    /// Add an item.
    Add {
        #[arg(long)]
        title: String,
        #[arg(long, default_value = "login")]
        r#type: String,
        #[arg(long, default_value = "")]
        username: String,
        #[arg(long, default_value = "")]
        password: String,
        #[arg(long, default_value = "")]
        url: String,
        #[arg(long, default_value = "")]
        notes: String,
        /// Raw item JSON; overrides the field flags when given.
        #[arg(long)]
        json: Option<String>,
    },
    /// List items (decrypted titles/usernames, shown locally only).
    List,
    /// Print one item's full decrypted JSON.
    Get { id: String },
    /// Edit an item: override individual fields on the existing item.
    Edit {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        username: Option<String>,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        notes: Option<String>,
    },
    /// Soft-delete an item.
    Rm { id: String },
    /// Change the master password.
    Passwd,
    /// Recover with the recovery code and set a new password.
    Recover { email: String },
    /// Generate a password (no account needed).
    Gen {
        #[arg(long, default_value_t = 20)]
        length: u32,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Command::Signup { email } => signup(&email),
        Command::Login { email } => login(&email),
        Command::Logout => {
            SessionFile::clear()?;
            println!("logged out.");
            Ok(())
        }
        Command::Add { title, r#type, username, password, url, notes, json } => {
            add(title, r#type, username, password, url, notes, json)
        }
        Command::List => list(),
        Command::Get { id } => get(&id),
        Command::Edit { id, title, username, password, url, notes } => {
            edit(&id, title, username, password, url, notes)
        }
        Command::Rm { id } => rm(&id),
        Command::Passwd => passwd(),
        Command::Recover { email } => recover(&email),
        Command::Gen { length } => {
            let opts = GenOptions { length, ..GenOptions::default() };
            println!("{}", generate_password(opts)?);
            Ok(())
        }
    }
}

// ── password input ─────────────────────────────────────────────────────────

/// Master password from `EVEPASS_PASSWORD` (for scripted acceptance runs) or an
/// interactive prompt.
fn prompt_password(label: &str) -> Result<String> {
    if let Ok(p) = std::env::var("EVEPASS_PASSWORD") {
        return Ok(p);
    }
    rpassword::prompt_password(format!("{label}: ")).context("reading password")
}

fn b64_decode(s: &str) -> Result<Vec<u8>> {
    base64::engine::general_purpose::STANDARD.decode(s).context("base64 decode")
}

fn b64_encode(b: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(b)
}

// ── unlock helper: re-derive the Session for vault-touching commands ────────

struct Unlocked {
    session: Arc<Session>,
    sf: SessionFile,
    sb: Supabase,
}

fn open_vault() -> Result<Unlocked> {
    let sf = SessionFile::load()?;
    let password = prompt_password("Master password")?;
    let salt = b64_decode(&sf.kdf_salt_b64)?;
    let wrapped = b64_decode(&sf.wrapped_vault_key_b64)?;
    let session = unlock(password, salt, sf.kdf_params.clone(), wrapped)
        .context("unlock failed (wrong password?)")?;
    let mut sb = Supabase::from_env()?;
    sb.set_token(&sf.access_token);
    Ok(Unlocked { session, sf, sb })
}

// ── commands ────────────────────────────────────────────────────────────────

fn signup(email: &str) -> Result<()> {
    let password = prompt_password("Choose a master password")?;
    let confirm = prompt_password("Confirm master password")?;
    if password != confirm {
        anyhow::bail!("passwords do not match");
    }

    // 1. Core: derive keys, generate vault key/keypair/recovery code.
    let acct = create_account(password)?;

    // 2. Supabase Auth: register with email + authKey as the "password".
    let sb = Supabase::from_env()?;
    let auth = sb.signup(email, &acct.auth_key_b64)?;

    // 3. Authenticated writes: login_params (prelogin) + profile (wrapped keys).
    let mut sb = sb;
    sb.set_token(&auth.access_token);
    sb.insert_login_params(email, &acct.kdf_salt, &acct.kdf_params)?;
    sb.insert_profile(
        &auth.user_id,
        &acct.kdf_salt,
        &acct.kdf_params,
        &acct.wrapped_vault_key,
        &acct.wrapped_vault_key_recovery,
        &acct.public_key,
        &acct.signing_public_key,
        &acct.wrapped_private_keys,
    )?;

    // 4. Cache the session (wrapped keys only — never the vault key).
    SessionFile {
        email: email.to_string(),
        user_id: auth.user_id,
        access_token: auth.access_token,
        refresh_token: auth.refresh_token,
        kdf_salt_b64: b64_encode(&acct.kdf_salt),
        kdf_params: acct.kdf_params.clone(),
        wrapped_vault_key_b64: b64_encode(&acct.wrapped_vault_key),
        wrapped_vault_key_recovery_b64: b64_encode(&acct.wrapped_vault_key_recovery),
    }
    .save()?;

    println!("account created for {email}.\n");
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  RECOVERY CODE — write this down and store it offline.     ║");
    println!("║  It is shown ONCE. Without it, a forgotten password means  ║");
    println!("║  permanent data loss.                                      ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!("\n    {}\n", acct.recovery_code);
    Ok(())
}

fn login(email: &str) -> Result<()> {
    let mut sb = Supabase::from_env()?;

    // Prelogin: fetch salt/params without auth, derive authKey.
    let (salt, params) = sb.get_login_params(email)?;
    let password = prompt_password("Master password")?;
    let auth_key = auth_key_for_login(password, salt.clone(), params.clone())?;

    // Sign in → JWT.
    let auth = sb.signin(email, &auth_key)?;
    sb.set_token(&auth.access_token);

    // Download the profile (wrapped keys).
    let profile = sb.get_profile()?;

    SessionFile {
        email: email.to_string(),
        user_id: auth.user_id,
        access_token: auth.access_token,
        refresh_token: auth.refresh_token,
        kdf_salt_b64: b64_encode(&salt),
        kdf_params: params,
        wrapped_vault_key_b64: b64_encode(&profile.wrapped_vault_key),
        wrapped_vault_key_recovery_b64: b64_encode(&profile.wrapped_vault_key_recovery),
    }
    .save()?;

    println!("logged in as {email}. Session cached.");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn add(
    title: String,
    type_: String,
    username: String,
    password: String,
    url: String,
    notes: String,
    json: Option<String>,
) -> Result<()> {
    let u = open_vault()?;
    let item_json = match json {
        Some(j) => j,
        None => json!({
            "type": type_, "title": title, "username": username,
            "password": password, "url": url, "notes": notes,
            "folders": [], "tags": []
        })
        .to_string(),
    };
    let blob = u.session.encrypt_item(item_json)?;
    let id = u.sb.insert_item(&u.sf.user_id, &blob.envelope)?;
    println!("added item {id}");
    Ok(())
}

fn list() -> Result<()> {
    let u = open_vault()?;
    let rows = u.sb.list_items(false)?;
    if rows.is_empty() {
        println!("(vault empty)");
        return Ok(());
    }
    println!("{:<38}  {:<24}  {}", "ID", "TITLE", "USERNAME");
    for row in rows {
        let json = u.session.decrypt_item(evepass_core::Blob { envelope: row.envelope })?;
        let v: Value = serde_json::from_str(&json)?;
        println!(
            "{:<38}  {:<24}  {}",
            row.id,
            v["title"].as_str().unwrap_or(""),
            v["username"].as_str().unwrap_or("")
        );
    }
    Ok(())
}

fn get(id: &str) -> Result<()> {
    let u = open_vault()?;
    let row = u.sb.get_item(id)?;
    let json = u.session.decrypt_item(evepass_core::Blob { envelope: row.envelope })?;
    let pretty: Value = serde_json::from_str(&json)?;
    println!("{}", serde_json::to_string_pretty(&pretty)?);
    Ok(())
}

fn edit(
    id: &str,
    title: Option<String>,
    username: Option<String>,
    password: Option<String>,
    url: Option<String>,
    notes: Option<String>,
) -> Result<()> {
    let u = open_vault()?;
    let row = u.sb.get_item(id)?;
    let json = u.session.decrypt_item(evepass_core::Blob { envelope: row.envelope.clone() })?;
    let mut v: Value = serde_json::from_str(&json)?;

    for (field, val) in [
        ("title", title),
        ("username", username),
        ("password", password),
        ("url", url),
        ("notes", notes),
    ] {
        if let Some(val) = val {
            v[field] = Value::String(val);
        }
    }

    let blob = u.session.encrypt_item(v.to_string())?;
    u.sb.update_item(id, &blob.envelope, row.revision + 1)?;
    println!("updated item {id}");
    Ok(())
}

fn rm(id: &str) -> Result<()> {
    let u = open_vault()?;
    u.sb.soft_delete_item(id)?;
    println!("deleted item {id}");
    Ok(())
}

fn passwd() -> Result<()> {
    let u = open_vault()?;
    let new_password = prompt_password("New master password")?;
    let confirm = prompt_password("Confirm new master password")?;
    if new_password != confirm {
        anyhow::bail!("passwords do not match");
    }

    let salt = b64_decode(&u.sf.kdf_salt_b64)?;
    let params: KdfParams = u.sf.kdf_params.clone();
    let change = u.session.change_password(new_password, salt, params)?;

    // Update GoTrue password (new authKey) and the profile's wrapped_vault_key.
    u.sb.update_auth_password(&change.auth_key_b64)?;
    u.sb.update_wrapped_vault_key(&u.sf.user_id, &change.wrapped_vault_key)?;

    // Refresh the local cache with the new wrapped vault key.
    let mut sf = u.sf;
    sf.wrapped_vault_key_b64 = b64_encode(&change.wrapped_vault_key);
    sf.save()?;

    println!("master password changed. Existing items are untouched (not re-encrypted).");
    Ok(())
}

fn recover(email: &str) -> Result<()> {
    let mut sb = Supabase::from_env()?;
    let (salt, params) = sb.get_login_params(email)?;

    // Fase 0 scope: recovery reads the recovery-wrapped vault key from the local
    // session cache (written at signup/login on this device). A full
    // cross-device reset — reading wrapped_vault_key_recovery without any prior
    // session, or resetting the server password via an email link — is deferred
    // to Fase 4's polished recovery. The core capability validated here is that
    // the recovery code alone can unlock the vault key.
    let sf = SessionFile::load()
        .context("recovery in Fase 0 needs a prior session on this device; run `login` once first")?;
    let wrapped_recovery = b64_decode(&sf.wrapped_vault_key_recovery_b64)?;

    let recovery_code = prompt_password("Recovery code")?;
    let session = unlock_with_recovery(recovery_code, wrapped_recovery)
        .context("recovery failed (wrong code?)")?;
    println!("recovery code accepted — vault key recovered.");

    // Set a new master password: re-wrap the vault key + rotate the authKey.
    let new_password = prompt_password("New master password")?;
    let change = session.change_password(new_password, salt.clone(), params.clone())?;

    sb.set_token(&sf.access_token);
    sb.update_auth_password(&change.auth_key_b64)?;
    sb.update_wrapped_vault_key(&sf.user_id, &change.wrapped_vault_key)?;

    let mut sf = sf;
    sf.wrapped_vault_key_b64 = b64_encode(&change.wrapped_vault_key);
    sf.kdf_salt_b64 = b64_encode(&salt);
    sf.kdf_params = params;
    sf.save()?;

    println!("new master password set.");
    Ok(())
}
