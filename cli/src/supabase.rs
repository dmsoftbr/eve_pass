//! Thin Supabase REST/GoTrue client. This is the only place that talks to the
//! network — it moves **ciphertext only** (bytea envelopes) to/from Postgres and
//! handles auth. The core never sees any of this.

use anyhow::{anyhow, bail, Context, Result};
use evepass_core::KdfParams;
use reqwest::blocking::Client;
use serde_json::{json, Value};

pub struct Supabase {
    http: Client,
    url: String,
    anon_key: String,
    token: Option<String>,
}

pub struct Auth {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: String,
}

/// A decrypted-nothing row: the ciphertext envelope plus sync metadata.
pub struct Row {
    pub id: String,
    pub envelope: Vec<u8>,
    pub revision: i64,
    #[allow(dead_code)] // consumed by sync/reconciliation in Fase 1
    pub deleted: bool,
}

/// Encode bytes as a PostgREST `bytea` literal (`\x`-prefixed hex).
fn bytea(bytes: &[u8]) -> String {
    format!("\\x{}", hex::encode(bytes))
}

/// Decode a PostgREST `bytea` value (`\x<hex>`) back to bytes.
fn parse_bytea(s: &str) -> Result<Vec<u8>> {
    let hexpart = s.strip_prefix("\\x").unwrap_or(s);
    hex::decode(hexpart).context("decoding bytea hex")
}

impl Supabase {
    pub fn from_env() -> Result<Supabase> {
        let url = std::env::var("SUPABASE_URL")
            .context("SUPABASE_URL not set")?
            .trim_end_matches('/')
            .to_string();
        let anon_key = std::env::var("SUPABASE_ANON_KEY").context("SUPABASE_ANON_KEY not set")?;
        Ok(Supabase { http: Client::new(), url, anon_key, token: None })
    }

    pub fn set_token(&mut self, token: impl Into<String>) {
        self.token = Some(token.into());
    }

    fn bearer(&self) -> &str {
        self.token.as_deref().unwrap_or(&self.anon_key)
    }

    // ── Auth (GoTrue) ──────────────────────────────────────────────────────

    pub fn signup(&self, email: &str, auth_key_b64: &str) -> Result<Auth> {
        let resp = self
            .http
            .post(format!("{}/auth/v1/signup", self.url))
            .header("apikey", &self.anon_key)
            .json(&json!({ "email": email, "password": auth_key_b64 }))
            .send()?;
        let status = resp.status();
        let body: Value = resp.json().context("signup response not JSON")?;
        if !status.is_success() {
            bail!("signup failed ({status}): {body}");
        }
        parse_auth(&body).ok_or_else(|| {
            anyhow!(
                "signup succeeded but returned no session — disable email confirmation in \
                 Supabase (Auth → Providers → Email → confirm email OFF). Body: {body}"
            )
        })
    }

    pub fn signin(&self, email: &str, auth_key_b64: &str) -> Result<Auth> {
        let resp = self
            .http
            .post(format!("{}/auth/v1/token?grant_type=password", self.url))
            .header("apikey", &self.anon_key)
            .json(&json!({ "email": email, "password": auth_key_b64 }))
            .send()?;
        let status = resp.status();
        let body: Value = resp.json().context("signin response not JSON")?;
        if !status.is_success() {
            bail!("login failed ({status}): {body}");
        }
        parse_auth(&body).ok_or_else(|| anyhow!("login returned no session: {body}"))
    }

    /// Update the GoTrue account password (the base64 authKey) after a master
    /// password change. Requires the bearer token to be set.
    pub fn update_auth_password(&self, new_auth_key_b64: &str) -> Result<()> {
        let resp = self
            .http
            .put(format!("{}/auth/v1/user", self.url))
            .header("apikey", &self.anon_key)
            .bearer_auth(self.bearer())
            .json(&json!({ "password": new_auth_key_b64 }))
            .send()?;
        let status = resp.status();
        if !status.is_success() {
            let body: Value = resp.json().unwrap_or(Value::Null);
            bail!("password update failed ({status}): {body}");
        }
        Ok(())
    }

    // ── login_params (prelogin; salt readable without auth) ────────────────

    pub fn insert_login_params(&self, email: &str, salt: &[u8], params: &KdfParams) -> Result<()> {
        self.rest_insert(
            "login_params",
            json!({ "email": email, "kdf_salt": bytea(salt), "kdf_params": params }),
        )
        .map(|_| ())
    }

    pub fn get_login_params(&self, email: &str) -> Result<(Vec<u8>, KdfParams)> {
        let url = format!(
            "{}/rest/v1/login_params?email=eq.{}&select=kdf_salt,kdf_params",
            self.url, email
        );
        let resp = self.http.get(url).header("apikey", &self.anon_key).send()?;
        let rows: Value = resp.json()?;
        let row = rows
            .as_array()
            .and_then(|a| a.first())
            .ok_or_else(|| anyhow!("no login_params for {email} (unknown account?)"))?;
        let salt = parse_bytea(row["kdf_salt"].as_str().context("kdf_salt missing")?)?;
        let params: KdfParams = serde_json::from_value(row["kdf_params"].clone())?;
        Ok((salt, params))
    }

    // ── profiles ───────────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub fn insert_profile(
        &self,
        user_id: &str,
        salt: &[u8],
        params: &KdfParams,
        wrapped_vault_key: &[u8],
        wrapped_vault_key_recovery: &[u8],
        public_key: &[u8],
        signing_public_key: &[u8],
        wrapped_private_keys: &[u8],
    ) -> Result<()> {
        self.rest_insert(
            "profiles",
            json!({
                "user_id": user_id,
                "kdf_salt": bytea(salt),
                "kdf_params": params,
                "wrapped_vault_key": bytea(wrapped_vault_key),
                "wrapped_vault_key_recovery": bytea(wrapped_vault_key_recovery),
                "public_key": bytea(public_key),
                "signing_public_key": bytea(signing_public_key),
                "wrapped_private_keys": bytea(wrapped_private_keys),
            }),
        )
        .map(|_| ())
    }

    /// Patch the caller's `profiles.wrapped_vault_key` after a password change.
    pub fn update_wrapped_vault_key(&self, user_id: &str, wrapped: &[u8]) -> Result<()> {
        let url = format!("{}/rest/v1/profiles?user_id=eq.{}", self.url, user_id);
        let resp = self
            .http
            .patch(url)
            .header("apikey", &self.anon_key)
            .bearer_auth(self.bearer())
            .header("Content-Type", "application/json")
            .json(&json!({ "wrapped_vault_key": bytea(wrapped) }))
            .send()?;
        ensure_ok(resp, "update wrapped_vault_key")
    }

    pub fn get_profile(&self) -> Result<Profile> {
        let url = format!(
            "{}/rest/v1/profiles?select=wrapped_vault_key,wrapped_vault_key_recovery,wrapped_private_keys",
            self.url
        );
        let resp = self.http.get(url).header("apikey", &self.anon_key).bearer_auth(self.bearer()).send()?;
        let rows: Value = resp.json()?;
        let row = rows
            .as_array()
            .and_then(|a| a.first())
            .ok_or_else(|| anyhow!("profile not found"))?;
        Ok(Profile {
            wrapped_vault_key: parse_bytea(row["wrapped_vault_key"].as_str().context("wvk")?)?,
            wrapped_vault_key_recovery: parse_bytea(
                row["wrapped_vault_key_recovery"].as_str().context("wvkr")?,
            )?,
        })
    }

    // ── items ────────────────────────────────────────────────────────────

    pub fn insert_item(&self, user_id: &str, envelope: &[u8]) -> Result<String> {
        let out = self.rest_insert(
            "items",
            json!({ "user_id": user_id, "ciphertext": bytea(envelope) }),
        )?;
        out.as_array()
            .and_then(|a| a.first())
            .and_then(|r| r["id"].as_str())
            .map(String::from)
            .ok_or_else(|| anyhow!("insert item returned no id: {out}"))
    }

    pub fn list_items(&self, include_deleted: bool) -> Result<Vec<Row>> {
        let mut url = format!(
            "{}/rest/v1/items?select=id,ciphertext,revision,deleted_at&order=updated_at.desc",
            self.url
        );
        if !include_deleted {
            url.push_str("&deleted_at=is.null");
        }
        let resp = self.http.get(url).header("apikey", &self.anon_key).bearer_auth(self.bearer()).send()?;
        let rows: Value = resp.json()?;
        parse_rows(&rows)
    }

    pub fn get_item(&self, id: &str) -> Result<Row> {
        let url = format!(
            "{}/rest/v1/items?id=eq.{}&select=id,ciphertext,revision,deleted_at",
            self.url, id
        );
        let resp = self.http.get(url).header("apikey", &self.anon_key).bearer_auth(self.bearer()).send()?;
        let rows: Value = resp.json()?;
        parse_rows(&rows)?.into_iter().next().ok_or_else(|| anyhow!("item {id} not found"))
    }

    pub fn update_item(&self, id: &str, envelope: &[u8], revision: i64) -> Result<()> {
        let url = format!("{}/rest/v1/items?id=eq.{}", self.url, id);
        let resp = self
            .http
            .patch(url)
            .header("apikey", &self.anon_key)
            .bearer_auth(self.bearer())
            .header("Content-Type", "application/json")
            .json(&json!({
                "ciphertext": bytea(envelope),
                "revision": revision,
                "updated_at": "now()",
            }))
            .send()?;
        ensure_ok(resp, "update item")
    }

    pub fn soft_delete_item(&self, id: &str) -> Result<()> {
        let url = format!("{}/rest/v1/items?id=eq.{}", self.url, id);
        let resp = self
            .http
            .patch(url)
            .header("apikey", &self.anon_key)
            .bearer_auth(self.bearer())
            .header("Content-Type", "application/json")
            .json(&json!({ "deleted_at": "now()" }))
            .send()?;
        ensure_ok(resp, "delete item")
    }

    // ── low-level insert helper (returns representation) ───────────────────

    fn rest_insert(&self, table: &str, body: Value) -> Result<Value> {
        let resp = self
            .http
            .post(format!("{}/rest/v1/{}", self.url, table))
            .header("apikey", &self.anon_key)
            .bearer_auth(self.bearer())
            .header("Content-Type", "application/json")
            .header("Prefer", "return=representation")
            .json(&body)
            .send()?;
        let status = resp.status();
        let out: Value = resp.json().unwrap_or(Value::Null);
        if !status.is_success() {
            bail!("insert into {table} failed ({status}): {out}");
        }
        Ok(out)
    }
}

pub struct Profile {
    pub wrapped_vault_key: Vec<u8>,
    pub wrapped_vault_key_recovery: Vec<u8>,
}

fn parse_auth(body: &Value) -> Option<Auth> {
    let access_token = body["access_token"].as_str()?.to_string();
    let refresh_token = body["refresh_token"].as_str().unwrap_or("").to_string();
    let user_id = body["user"]["id"].as_str().or_else(|| body["id"].as_str())?.to_string();
    Some(Auth { access_token, refresh_token, user_id })
}

fn parse_rows(rows: &Value) -> Result<Vec<Row>> {
    let arr = rows.as_array().ok_or_else(|| anyhow!("expected array, got: {rows}"))?;
    let mut out = Vec::with_capacity(arr.len());
    for r in arr {
        out.push(Row {
            id: r["id"].as_str().context("row id missing")?.to_string(),
            envelope: parse_bytea(r["ciphertext"].as_str().context("ciphertext missing")?)?,
            revision: r["revision"].as_i64().unwrap_or(1),
            deleted: !r["deleted_at"].is_null(),
        });
    }
    Ok(out)
}

fn ensure_ok(resp: reqwest::blocking::Response, what: &str) -> Result<()> {
    let status = resp.status();
    if !status.is_success() {
        let body: Value = resp.json().unwrap_or(Value::Null);
        bail!("{what} failed ({status}): {body}");
    }
    Ok(())
}
