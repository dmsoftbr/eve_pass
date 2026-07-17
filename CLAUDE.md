# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Current state

**Fases 0, 1, 2, 4 done; Fase 5 wired into the core + desktop commands (runtime + shell/SQL/UX ends pending); Fase 3 partial. All code compiles clean; 61 core tests pass.** See `docs/STATUS.md` — the canonical progress doc; update it (and the PRD banner + plan roadmap) as work lands. `PENDENCIAS.md` is the acionable checklist.
- **Fase 0** — `core/` + `cli/` + `infra/`, crypto core, 57 core tests pass (RFC vectors + eTLD+1 + biometric key round-trip + HPKE sharing + post-quantum hybrid + Secret Key 2SKD + passkey ES256). UniFFI bindings generate.
- **Fase 1** — desktop app `apps/desktop/` (Tauri v2 + React/Vite/Tailwind). All PRD §6 commands; offline cache + Realtime sync + conflict copy + `copy_field`-in-Rust.
- **Fase 2** — command palette (2nd window + global hotkey), tray with lock state + hide-on-close + autostart, smart views (`vault_health`; breach via HIBP k-anonymity, hashes stay in Rust), live TOTP, auto-lock + clipboard auto-clear, settings, import.
- **Fase 3 (partial)** — mobile core done + **verified**: `match_credentials` (eTLD+1), `extract_credential`, `session_from_vault_key`, `Session.export_vault_key`. Core **builds for iOS** (`EvepassCore.xcframework`) and **Android** (`.so`) via `scripts/build-{ios,android}.sh`, bindings generated. `apps/mobile/` (RN app + native Swift/Kotlin autofill+biometric modules) is a **scaffold** — needs a bare RN project + device.
- **Fase 4** — team sharing E2E via **HPKE (RFC 9180)** + Ed25519 sharer signature (`core/src/collections.rs`): create/wrap/load/rotate collection keys, fingerprint, `reset_password` (recovery). SQL `0002` (public_keys + `is_member`/`can_write`/`is_admin` + RLS). Desktop: private keys loaded into Session, cache `collection_id`, ShareModal (fingerprint verify), collections sidebar, recovery flow. Session now holds a keypair + `collection_id→key` map (behind `Mutex`).

- **Fase 5** — all four optional modules **wired into the core + desktop commands + tested** (61 core tests): **5A** browser extension (`apps/browser-extension/`, MV3) now has a native-messaging **host** — thin bridge `native-host/` (`evepass-native-host`, Chrome stdio ↔ Unix socket `~/.evepass/host.sock`) + server `apps/desktop/src-tauri/src/host.rs` against the live Session + **pairing** UI (`HostPairModal`). **5B** post-quantum hybrid X25519+ML-KEM-768 (`core/src/pq.rs`) is wired into collection sharing: accounts carry an ML-KEM keypair, `Session::wrap_collection_key_for_pq` produces a signed hybrid (v2) wrap, and `load_collection_keys` dispatches on the version byte (`pq::is_hybrid`) so HPKE v1 and hybrid v2 coexist. **5C** Secret Key 2SKD is opt-in: `Session::enable_secret_key` + `unlock_with_secret`/`begin_login_with_secret`; the desktop stores a local `secret.key` and `begin_login` uses it transparently. **5D** passkeys add `passkey_assert` (sign + counter bump) + desktop `create/list/sign` commands (passkey = encrypted item). **Remaining = runtime validation + shell/SQL/UX ends:** 5A needs Chrome load + host registration; 5B needs a `mlkem_public_key` column + shell fetch; 5C needs the enable/import UX + server authKey update; 5D needs the browser WebAuthn ceremony / mobile providers. See `docs/STATUS.md` and `PENDENCIAS.md`.

What remains is **live runtime validation** (Supabase project + display for the GUI, 2 accounts for sharing, a device/simulator + RN project for mobile, Chrome + host for the extension) and wiring the Fase 5 primitives into the live flow. See `apps/desktop/README.md`, `apps/mobile/README.md`, `apps/browser-extension/README.md`, `infra/supabase/README.md`.

Documents (read these before touching related code):
- `docs/EVEPass-Plano.md` — master plan: principles, locked decisions, crypto model, data model, roadmap.
- `docs/PRD-EVEPass-Fase0.md` — crypto core (Rust) + CLI + Supabase schema. **Start here.**
- `docs/PRD-EVEPass-Fase1.md` — desktop MVP (Tauri v2 + React).
- `docs/PRD-EVEPass-Fase2.md` — command palette, tray, smart views, TOTP, import.
- `docs/PRD-EVEPass-Fase3.md` — mobile (React Native) + native autofill.
- `docs/PRD-EVEPass-Fase4.md` — team collections + HPKE sharing.
- `docs/PRD-EVEPass-Fase5.md` — optional: browser ext, post-quantum, Secret Key, passkeys.

The PRDs are written in Portuguese; match that language in docs and user-facing strings.

## What EVEPass is

A zero-knowledge, cross-platform password manager for personal/team use (not a SaaS). All cryptography happens on the client; the server (Supabase) only ever stores ciphertext.

## Planned architecture (the big picture)

A single **pure Rust core** (`evepass-core`) holds all cryptography, vault logic, and the local encrypted cache. **The core never touches the network.** Each platform shell does the Supabase I/O (auth, REST, Realtime) in JS/TS and passes only ciphertext to/from the core.

- **Core ↔ shells binding differs by platform, intentionally:**
  - **Desktop (Tauri v2):** the backend is already Rust, so it depends on `evepass-core` **directly** and exposes `#[tauri::command]`. UniFFI is *not* used here.
  - **Mobile (React Native, Fase 3):** consumes the core via **UniFFI** (Swift/Kotlin → JS). The core must compile to iOS xcframework and Android .so/AAR.
  - **Native autofill extensions:** separate OS processes that link the core and read the local cache offline (no network).

## Non-negotiable invariants (these define the product — do not violate)

- **Keys never leave Rust.** `vaultKey` and any key material live only inside a `Session` in the Rust core, held in memory and zeroized on drop. Plaintext of an item may cross the Rust↔JS boundary only for display/edit; **keys never cross it.**
- **`copy_field` decrypts and writes to the clipboard inside Rust** — the secret value does not pass through JS.
- **Only ciphertext is persisted or transmitted.** Every stored blob is a self-describing **envelope**: `version(1) || alg_id(1) || nonce(24) || ciphertext+tag`. There is no separate `nonce` column — the envelope carries it. Decrypt dispatches on `version`/`alg_id` (crypto-agility layer; keep the v2 dispatch stub working).
- **Organization data lives inside the encrypted blob.** Folder membership and tags are fields of the item plaintext (`folders[]`, `tags[]`); the server sees only opaque rows. The client decrypts everything and rebuilds tree/tags/smart-views in memory. Folders/collections also exist as encrypted rows.
- **Crypto baseline (versioned as alg v1):** Argon2id (default `m=256MiB, t=3, p=4`, calibratable) → HKDF-SHA-256 domain separation (`eve/enc`, `eve/auth`) → XChaCha20-Poly1305 (24-byte nonce). X25519 + Ed25519 keypair generated at signup, private keys wrapped with `vaultKey`. Sharing uses HPKE (Fase 4).
- **Key hierarchy:** `masterKey = Argon2id(password, salt)` → `encKey` (stays on device) + `authKey` (sent to Supabase as the "password", base64). A random `vaultKey` encrypts all items; `wrapped_vault_key = AEAD(encKey, vaultKey)`. Changing the password only re-wraps `vaultKey` and updates `authKey` — **items are never re-encrypted.**
- **No Secret Key exists** (Fase 0–4). Recovery is via a one-time 128-bit **Recovery Code** shown once at onboarding; `wrapped_vault_key_recovery = AEAD(recoveryKey, vaultKey)`. Forgetting the master password without the Recovery Code = total data loss.
- **Never log** password, `masterKey`, `encKey`, `vaultKey`, or private keys.
- In the core lib, **no `unwrap`/`expect` on error paths**; errors are a `thiserror` `CoreError` enum; all key material implements `Zeroize`.

## Prelogin gotcha (chicken-and-egg)

To derive `authKey` you need the KDF salt, but the salt lives in an RLS-protected `profiles` row you can't read until authenticated. Resolve this with a public, unauthenticated-readable `login_params(email, kdf_salt, kdf_params)` table (populated at signup). Fase 1 splits login into `begin_login` + `complete_login` so Argon2id runs **once**, not twice. Accepted trade-off: allows email enumeration (same as Bitwarden's prelogin), irrelevant at this scale.

## Sync & conflicts (Fase 1)

Local SQLite cache stores envelopes. Reconciliation is per-row, in Rust: last-write-wins by `revision` (tie-break `updated_at`); a real divergence (local `dirty` + remote `revision` advanced past base) produces a **conflict copy** `"<title> (conflito)"` rather than silent loss. Realtime pushes changes (`postgres_changes` filtered by `user_id`); no polling. Every local write increments `revision`. Soft-delete via `deleted_at` for sync.

## Intended repo layout (created in Fase 0)

```
evepass/
├── Cargo.toml          # Rust workspace
├── core/               # evepass-core (lib): kdf, keys, aead, envelope, account, item, generator, totp + UniFFI
├── cli/                # evepass-cli (bin): exercises the core against Supabase over REST
└── infra/supabase/migrations/   # 0001_init.sql (schema + RLS)
```

Desktop (`apps/desktop`) and mobile (`apps/mobile`) are added in later phases.

## Commands

Rust is installed via **brew's rustup**, whose shims are not on the default PATH. Prefix cargo commands with:
`export PATH="$(brew --prefix rustup)/libexec/bin:$PATH"` (or `~/.rustup/toolchains/stable-*/bin`).

- `cargo build` / `cargo test` — whole workspace.
- `cargo test -p evepass-core` — core tests, including the known-answer vector tests (Argon2id RFC 9106, HKDF RFC 5869, XChaCha20-Poly1305 CFRG draft, X25519 RFC 7748, Ed25519 RFC 8032).
- `cargo test <name>` — a single test by substring.
- `cargo run -q -p evepass-cli -- <cmd>` — the test CLI (binary `evepass`): `signup`, `login`, `logout`, `add`, `list`, `get`, `edit`, `rm`, `passwd`, `recover`, `gen`.
- Generate UniFFI bindings: `cargo run --bin uniffi-bindgen -- generate --library target/debug/libevepass_core.dylib --language swift --out-dir <dir>` (also `kotlin`).

**Desktop app** (`apps/desktop/`): `npm install`, then `npm run tauri dev` (Vite + native window) or `npm run build` (frontend only, tsc+vite) / `npm run tauri build`. Needs `apps/desktop/.env` with `VITE_SUPABASE_URL` + `VITE_SUPABASE_ANON_KEY`. `src-tauri/` is a **detached** Rust workspace (its own `[workspace]`), so build it from inside `apps/desktop/src-tauri/` (or via the tauri CLI), not from the repo-root `cargo`. Icons under `src-tauri/icons/` are generated via `npx tauri icon <src.png>`.

The CLI reads `SUPABASE_URL` and `SUPABASE_ANON_KEY` from the environment, and optionally `EVEPASS_PASSWORD` (skips the interactive master-password prompt — for scripted acceptance runs). It caches a local session in `~/.evepass/session.json` (JWT + *wrapped* keys only; never the vault key).

Argon2id at default params (256 MiB) is deliberately slow; `[profile.dev.package.argon2] opt-level = 3` in the root `Cargo.toml` keeps dev-build tests fast. Don't lower it.

## Where the crypto lives (core module map)

`core/src/`: `envelope.rs` (versioned framing + v2 dispatch stub) · `aead.rs` (XChaCha20-Poly1305) · `kdf.rs` (Argon2id + calibrate) · `keys.rs` (HKDF hierarchy + wrap/unwrap) · `keypair.rs` (X25519/Ed25519) · `recovery.rs` (Crockford base32 recovery code) · `item.rs` / `folder.rs` (models + crypt, distinct AAD) · `generator.rs` · `totp.rs` · `account.rs` (`create_account`/`unlock`/`unlock_with_recovery`/`Session`/`change_password`) · `login.rs` (`begin_login`/`LoginContext.complete` — Argon2-once desktop login, **not** UniFFI-exported) · `lib.rs` (public API + `uniffi::setup_scaffolding!`). Modules are private; the UniFFI/public surface is the re-exports in `lib.rs`.

**Desktop backend** (`apps/desktop/src-tauri/src/`): `state.rs` (the `Session` + `LoginContext` + per-user cache + settings + breach index live here, in Rust only) · `cache.rs` (SQLite: `cache_items`/`cache_folders`/`meta` + dirty upload queue) · `settings.rs` (JSON-persisted, non-sensitive) · `commands.rs` (the `#[tauri::command]` boundary — DTOs cross as base64 envelopes; reconciliation LWW + conflict-copy, `vault_health`, breach k-anonymity, `palette_search`, import batches) · `lib.rs` (tray, hide-on-close, global-shortcut→palette, autostart, inactivity auto-lock thread emitting `vault-locked`, `apply_settings`). **Frontend** (`apps/desktop/src/`): `lib/ipc.ts` (typed command wrappers) · `lib/supabase.ts` (only network module; bytea↔base64) · `lib/auth.ts` · `lib/sync.ts` · `lib/health.ts` (HIBP fetch) · `lib/import.ts` (Bitwarden/CSV parsers) · `state/vault.tsx` · `components/*` (incl. `Palette.tsx`, which runs in the 2nd window and shares Rust state, not the React context). Tauri v2 auto-maps camelCase JS args → snake_case Rust params, so ipc.ts uses camelCase keys. Keys/hashes never cross to JS: breach sends only 5-hex prefixes; `copy_field` writes the clipboard in Rust.

## Verifying zero-knowledge (manual acceptance gate)

Every phase's acceptance includes: inspect the Postgres rows directly and confirm **no plaintext is readable** anywhere — not a title, username, folder name, or tag. Treat any readable field as a bug that blocks the phase.
