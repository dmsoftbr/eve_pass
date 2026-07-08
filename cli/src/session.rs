//! Local session persistence for the test CLI.
//!
//! Between processes we cache the Supabase JWT plus the *wrapped* vault key and
//! KDF params — **never** the vault key itself. Vault-touching commands re-prompt
//! for the master password (or read `EVEPASS_PASSWORD`) and re-unlock via the
//! core, so no plaintext key ever hits disk. This keeps the CLI honest about the
//! zero-knowledge model even though it is only a validation harness.

use std::path::PathBuf;

use anyhow::{Context, Result};
use evepass_core::KdfParams;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SessionFile {
    pub email: String,
    pub user_id: String,
    pub access_token: String,
    pub refresh_token: String,
    pub kdf_salt_b64: String,
    pub kdf_params: KdfParams,
    pub wrapped_vault_key_b64: String,
    pub wrapped_vault_key_recovery_b64: String,
}

fn path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home).join(".evepass").join("session.json"))
}

impl SessionFile {
    pub fn save(&self) -> Result<()> {
        let p = path()?;
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).context("creating ~/.evepass")?;
        }
        let json = serde_json::to_vec_pretty(self)?;
        std::fs::write(&p, json).with_context(|| format!("writing {}", p.display()))?;
        Ok(())
    }

    pub fn load() -> Result<SessionFile> {
        let p = path()?;
        let data = std::fs::read(&p)
            .with_context(|| format!("no active session ({}). Run `evepass login` first.", p.display()))?;
        Ok(serde_json::from_slice(&data)?)
    }

    pub fn clear() -> Result<()> {
        let p = path()?;
        if p.exists() {
            std::fs::remove_file(&p)?;
        }
        Ok(())
    }
}
