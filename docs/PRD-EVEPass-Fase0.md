# PRD — EVEPass · Fase 0: Fundação criptográfica zero-knowledge

> **Status (2026-07-06): ✅ implementado e compilando** (core Rust + CLI + esquema Supabase). 39 testes passam, incl. vetores RFC conhecidos; bindings UniFFI gerados. 🟡 Pendente apenas a validação ZK ponta a ponta contra um Supabase real + inspeção de plaintext no Postgres (requer provisionamento — ver `infra/supabase/README.md`). Progresso consolidado em [`STATUS.md`](./STATUS.md).

> Primeiro PRD da série (um por fase). Escopo: provar o modelo zero-knowledge ponta a ponta com criptografia testada, **sem nenhuma UI**. Consumir com Claude Code.

## 1. Objetivo

Entregar uma fundação onde uma CLI consegue **criar conta, destravar, e fazer CRUD de itens cifrados** contra o Supabase, provando que o servidor nunca vê plaintext. Toda a criptografia vive num core Rust reutilizável e coberto por testes de vetor conhecidos. Ao fim da fase, o modelo ZK está validado e a camada de cripto-agilidade está no lugar — antes de qualquer investimento em interface.

## 2. Escopo

**Dentro:**
- Crate `evepass-core` (lib) com toda a criptografia e lógica de cofre.
- Scaffolding de bindings UniFFI no core (mesmo que só a CLI consuma agora).
- Projeto Supabase: esquema + RLS versionados como migrations SQL.
- Crate `evepass-cli` (bin) que exercita o core contra o Supabase.
- Testes: vetores conhecidos por primitive + round-trip + fluxo de integração completo.

**Fora (fases seguintes):**
- Tauri, React, mobile, tray, command palette, autofill — Fases 1–3.
- Realtime/push de sync — Fase 1 (aqui o CRUD é REST simples).
- HPKE para compartilhamento — Fase 4 (aqui só geramos e guardamos o par de chaves).
- Smart views, breach monitoring, gerador na UI — Fases 1–2.

## 3. Estrutura do repositório

```
evepass/
├── Cargo.toml                 # workspace
├── core/                      # crate evepass-core (lib) + UniFFI
│   ├── src/
│   │   ├── lib.rs
│   │   ├── kdf.rs             # Argon2id + calibração
│   │   ├── keys.rs           # HKDF, hierarquia, wrap/unwrap
│   │   ├── aead.rs           # XChaCha20-Poly1305 + envelope
│   │   ├── envelope.rs       # header versionado (agilidade)
│   │   ├── account.rs        # create_account, unlock, change_password, recovery
│   │   ├── item.rs           # encrypt_item / decrypt_item + modelo do item
│   │   ├── generator.rs      # gerador de senhas
│   │   └── totp.rs
│   ├── tests/                # vetores conhecidos + round-trip
│   └── evepass.udl           # (ou macros uniffi) interface exposta
├── cli/                       # crate evepass-cli (bin)
│   └── src/main.rs           # signup/login/crud contra o Supabase (REST)
└── infra/supabase/
    ├── migrations/
    │   └── 0001_init.sql
    └── README.md             # como subir (supabase start / projeto cloud)
```

## 4. Requisitos funcionais (o core)

O core é **puro**: recebe entradas, faz criptografia e devolve bytes/estruturas. **Não fala com a rede** — a CLI (e depois as UIs) fazem o I/O do Supabase, passando só ciphertext de/para o core.

### 4.1 Criptografia (baseline forte)

- **KDF:** Argon2id. Params default `m = 256 MiB, t = 3, p = 4`, com helper `calibrate_kdf(target_ms) -> KdfParams` para ajustar ao hardware. Salt aleatório de 16 bytes por usuário (não é segredo; guardado no profile).
- **Derivação:** `masterKey = Argon2id(password, salt, params)`; depois HKDF-SHA-256 com separação de domínio: `encKey = HKDF(masterKey, info="eve/enc")`, `authKey = HKDF(masterKey, info="eve/auth")`.
- **Cifra:** XChaCha20-Poly1305 (AEAD, nonce de 24 bytes gerado por operação).
- **Par de chaves (para sharing futuro):** X25519 + Ed25519 gerados no signup; privadas cifradas com `vaultKey`.
- **Aleatoriedade:** CSPRNG do SO (`getrandom`). Todo material sensível implementa `Zeroize` e é zerado no drop.

### 4.2 Formato de envelope (agilidade)

Todo blob cifrado é **auto-descritivo**:

```
envelope = version (1 byte) || alg_id (1 byte) || nonce (24 bytes) || ciphertext+tag
version 1 / alg_id 1 = { Argon2id, HKDF-SHA256, XChaCha20-Poly1305, X25519, Ed25519 }
```

O decrypt lê `version`/`alg_id` e despacha para o conjunto certo. Incluir um **stub de v2** só para provar o caminho de dispatch (não precisa implementar novo algoritmo). Isso é o que permitirá adicionar pós-quântico/Secret Key depois sem reescrever. Como o envelope carrega o nonce, a coluna `ciphertext` no banco guarda o envelope inteiro (não há coluna `nonce` separada).

### 4.3 Contrato de API (exposto via UniFFI)

```rust
// Conta e chaves
struct KdfParams { alg: String, m: u32, t: u32, p: u32 }
struct NewAccount {
    kdf_salt: Vec<u8>,
    kdf_params: KdfParams,
    auth_key_b64: String,             // enviado ao Supabase como "senha"
    wrapped_vault_key: Vec<u8>,
    wrapped_vault_key_recovery: Vec<u8>,
    recovery_code: String,            // exibido UMA vez ao usuário
    public_key: Vec<u8>,
    signing_public_key: Vec<u8>,
    wrapped_private_keys: Vec<u8>,
}
fn create_account(password: String) -> Result<NewAccount>;

// Session detém a vaultKey em memória (zeroizada no drop)
fn unlock(password: String, salt: Vec<u8>, params: KdfParams,
          wrapped_vault_key: Vec<u8>) -> Result<Session>;
fn unlock_with_recovery(recovery_code: String,
                        wrapped_vault_key_recovery: Vec<u8>) -> Result<Session>;
fn auth_key_for_login(password: String, salt: Vec<u8>,
                      params: KdfParams) -> Result<String>; // deriva authKey p/ o Supabase
struct PasswordChange { auth_key_b64: String, wrapped_vault_key: Vec<u8>,
                        wrapped_vault_key_recovery: Vec<u8> }
fn change_password(session: &Session, new_password: String,
                   salt: Vec<u8>, params: KdfParams) -> Result<PasswordChange>;

// Itens (item_json = §4.4)
struct Blob { envelope: Vec<u8> }
fn encrypt_item(session: &Session, item_json: String) -> Result<Blob>;
fn decrypt_item(session: &Session, blob: Blob) -> Result<String>;

// Utilitários
struct GenOptions { length: u32, upper: bool, lower: bool, digits: bool, symbols: bool }
fn generate_password(opts: GenOptions) -> String;
struct TotpCode { code: String, seconds_remaining: u32 }
fn totp_now(otpauth_uri: String) -> Result<TotpCode>;

// Agilidade
fn calibrate_kdf(target_ms: u32) -> KdfParams;
```

### 4.4 Modelo do item (plaintext, antes de cifrar)

```json
{
  "type": "login",
  "title": "Servidor Datasul",
  "username": "diogo.admin",
  "password": "…",
  "url": "datasul.cliente.com",
  "totp": "otpauth://totp/…",
  "notes": "…",
  "folders": ["<uuid>", "<uuid>"],
  "tags": ["crítico", "cliente"],
  "custom_fields": [{ "name": "porta", "value": "22", "hidden": false }]
}
```

Os dados de organização (`folders`, `tags`) vivem **dentro** do blob — o servidor não vê estrutura. O cliente decifra tudo e monta árvore/tags/smart-views em memória.

## 5. Esquema Supabase (migration `0001_init.sql`)

```sql
create extension if not exists pgcrypto;

create table profiles (
  user_id uuid primary key references auth.users(id) on delete cascade,
  kdf_salt bytea not null,
  kdf_params jsonb not null,
  wrapped_vault_key bytea not null,
  wrapped_vault_key_recovery bytea not null,
  public_key bytea not null,
  signing_public_key bytea not null,
  wrapped_private_keys bytea not null,
  created_at timestamptz default now()
);

create table folders (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references auth.users(id) on delete cascade,
  ciphertext bytea not null,            -- envelope auto-descritivo
  revision bigint not null default 1,
  updated_at timestamptz default now(),
  deleted_at timestamptz
);

create table collections (
  id uuid primary key default gen_random_uuid(),
  owner_id uuid not null references auth.users(id),
  ciphertext bytea not null,
  created_at timestamptz default now()
);

create table collection_members (
  collection_id uuid references collections(id) on delete cascade,
  user_id uuid references auth.users(id) on delete cascade,
  wrapped_collection_key bytea not null,
  role text not null default 'member',
  primary key (collection_id, user_id)
);

create table items (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references auth.users(id) on delete cascade,
  collection_id uuid references collections(id),
  ciphertext bytea not null,            -- envelope auto-descritivo
  revision bigint not null default 1,
  updated_at timestamptz default now(),
  deleted_at timestamptz
);

-- RLS
alter table profiles enable row level security;
alter table folders enable row level security;
alter table items enable row level security;
alter table collections enable row level security;
alter table collection_members enable row level security;

create policy profiles_owner on profiles for all
  using (auth.uid() = user_id) with check (auth.uid() = user_id);
create policy folders_owner on folders for all
  using (auth.uid() = user_id) with check (auth.uid() = user_id);
create policy items_owner on items for all
  using (auth.uid() = user_id) with check (auth.uid() = user_id);
create policy items_shared_read on items for select
  using (collection_id in (
    select collection_id from collection_members where user_id = auth.uid()
  ));
create policy members_self on collection_members for select
  using (user_id = auth.uid());
```

## 6. Fluxo zero-knowledge (a CLI de teste)

A CLI (`evepass-cli`) lê `SUPABASE_URL` e `SUPABASE_ANON_KEY` do ambiente e implementa:

- `signup <email>` — pede a senha; chama `core.create_account`; registra no Supabase Auth usando `email + auth_key_b64` como senha (via GoTrue); grava `profiles` (salt, params, wrapped keys, chaves públicas). Exibe o `recovery_code` uma única vez.
- `login <email>` — pede a senha; deriva `auth_key` via `core.auth_key_for_login`; `signInWithPassword(email, auth_key_b64)` → JWT; baixa `profiles`; `core.unlock(...)` → Session.
- `add` / `list` / `get <id>` / `edit <id>` / `rm <id>` — cifra/decifra com o core e faz REST no Supabase (insert/select/update/soft-delete). O que trafega e persiste é **sempre** o envelope.
- `passwd` — `core.change_password`; atualiza a senha do Supabase Auth (novo `auth_key`) e o `profiles`.
- `recover <email>` — `core.unlock_with_recovery(recovery_code, ...)`; permite definir nova senha.

**Invariante a verificar manualmente:** inspecionar as linhas no Postgres e confirmar que não há um único campo legível (nem título, nem usuário, nem nome de pasta).

## 7. Critérios de aceite

- [ ] Vetores conhecidos passam para Argon2id, HKDF-SHA-256, XChaCha20-Poly1305, X25519 e Ed25519 (usar vetores das RFCs/referências oficiais).
- [ ] Round-trip: `encrypt_item` → `decrypt_item` devolve JSON idêntico.
- [ ] Senha errada em `unlock` retorna erro de autenticação AEAD — **sem panic**.
- [ ] Fluxo completo: `signup` → (persistência no Supabase) → **novo processo** `login` → itens decifram e batem com o original.
- [ ] `passwd`: após trocar, o `auth_key` antigo falha e o novo funciona; os itens continuam decifrando (sem re-cifrar itens).
- [ ] `recover`: recupera a `vaultKey` pelo recovery code e permite redefinir senha.
- [ ] Inspeção do Postgres não revela nenhum plaintext.
- [ ] Stub de envelope v2 prova o caminho de dispatch por versão.
- [ ] Nenhum `unwrap`/`expect` na lib; material de chave implementa `Zeroize` e é zerado no drop.

## 8. Convenções

- Erros com `thiserror` num enum `CoreError`; a CLI trata e imprime mensagens claras.
- Toda aleatoriedade via `getrandom`; nunca `rand::thread_rng` para material de chave sem CSPRNG do SO.
- `auth_key` (32 bytes) é codificado em base64 para virar string de senha do Supabase.
- Nunca logar senha, `masterKey`, `encKey`, `vaultKey` ou privadas.

## 9. Bibliotecas (Rust)

`argon2`, `hkdf`, `sha2`, `chacha20poly1305` (feature `xchacha20poly1305`), `x25519-dalek`, `ed25519-dalek`, `getrandom`, `zeroize`, `base64`, `serde` + `serde_json`, `thiserror`, `uniffi`, `totp-rs`. CLI: `reqwest` (REST no Supabase) + `clap` + `rpassword`.

## 10. Checklist de execução (ordem sugerida)

1. Criar o workspace e os três crates (`core`, `cli`, `infra`).
2. Implementar `envelope.rs` + `aead.rs` (com stub v2) e testar round-trip.
3. Implementar `kdf.rs` (Argon2id + `calibrate_kdf`) com vetores conhecidos.
4. Implementar `keys.rs` (HKDF, hierarquia, wrap/unwrap da `vaultKey`).
5. Implementar `account.rs` (`create_account`, `unlock`, `unlock_with_recovery`, `change_password`) + geração do par de chaves + recovery code.
6. Implementar `item.rs`, `generator.rs`, `totp.rs`.
7. Escrever a `evepass.udl` (ou macros) e gerar os bindings.
8. Provisionar o Supabase, aplicar `0001_init.sql`, validar RLS.
9. Escrever a `evepass-cli` (signup/login/crud/passwd/recover).
10. Rodar o fluxo completo e checar todos os critérios de aceite, incluindo a inspeção de plaintext no Postgres.
```
