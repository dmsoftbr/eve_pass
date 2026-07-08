-- EVEPass · Fase 0 — esquema zero-knowledge + RLS.
--
-- O servidor guarda apenas linhas opacas: envelopes AEAD auto-descritivos
-- (version||alg_id||nonce||ciphertext+tag) na coluna `ciphertext`. Não há
-- coluna `nonce` separada — o nonce viaja dentro do envelope. Nenhum campo aqui
-- é legível: nem título, nem usuário, nem nome de pasta/tag.

create extension if not exists pgcrypto;

-- ── Prelogin (refinamento herdado da Fase 1, incluído já aqui) ─────────────
-- Para derivar o authKey ANTES de autenticar, o cliente precisa do salt/params,
-- que vivem num `profiles` protegido por RLS (ovo-e-galinha). Esta tabela é
-- lida sem auth, por e-mail. Trade-off aceito: permite enumeração de e-mails
-- cadastrados — irrelevante nesta escala (mesmo padrão do prelogin do Bitwarden).
create table login_params (
  email      text primary key,
  kdf_salt   bytea not null,
  kdf_params jsonb not null           -- {alg:"argon2id", m, t, p}
);

-- ── Chaves por usuário ─────────────────────────────────────────────────────
create table profiles (
  user_id                    uuid primary key references auth.users(id) on delete cascade,
  kdf_salt                   bytea not null,
  kdf_params                 jsonb not null,
  wrapped_vault_key          bytea not null,   -- vaultKey cifrada com encKey
  wrapped_vault_key_recovery bytea not null,   -- vaultKey cifrada com o recovery code
  public_key                 bytea not null,   -- X25519 (sharing)
  signing_public_key         bytea not null,   -- Ed25519
  wrapped_private_keys       bytea not null,   -- privadas cifradas com vaultKey
  created_at                 timestamptz default now()
);

-- ── Pastas cifradas ({nome, parent_id} dentro do envelope) ─────────────────
create table folders (
  id         uuid primary key default gen_random_uuid(),
  user_id    uuid not null references auth.users(id) on delete cascade,
  ciphertext bytea not null,                   -- envelope auto-descritivo
  revision   bigint not null default 1,
  updated_at timestamptz default now(),
  deleted_at timestamptz                       -- soft delete p/ sync
);

-- ── Collections (compartilhamento de time; usadas de fato na Fase 4) ───────
create table collections (
  id         uuid primary key default gen_random_uuid(),
  owner_id   uuid not null references auth.users(id),
  ciphertext bytea not null,                   -- {nome}
  created_at timestamptz default now()
);

create table collection_members (
  collection_id          uuid references collections(id) on delete cascade,
  user_id                uuid references auth.users(id) on delete cascade,
  wrapped_collection_key bytea not null,       -- collectionKey cifrada p/ a public_key do membro (HPKE)
  role                   text not null default 'member',   -- 'admin' | 'member'
  primary key (collection_id, user_id)
);

-- ── Itens (blob AEAD: {tipo, título, usuário, senha, notas, totp, url, folders[], tags[]}) ──
create table items (
  id            uuid primary key default gen_random_uuid(),
  user_id       uuid not null references auth.users(id) on delete cascade,
  collection_id uuid references collections(id),   -- opcional (item de time)
  ciphertext    bytea not null,                    -- envelope auto-descritivo
  revision      bigint not null default 1,         -- para sync/conflito
  updated_at    timestamptz default now(),
  deleted_at    timestamptz                        -- soft delete p/ sync
);

-- ─────────────────────────────────────────────────────────────────────────
-- Row Level Security — o que garante o isolamento no servidor.
-- ─────────────────────────────────────────────────────────────────────────
alter table login_params      enable row level security;
alter table profiles          enable row level security;
alter table folders           enable row level security;
alter table items             enable row level security;
alter table collections       enable row level security;
alter table collection_members enable row level security;

-- login_params: leitura anônima por e-mail (prelogin); escrita só pelo dono.
create policy login_params_read on login_params for select using (true);
create policy login_params_insert on login_params for insert
  with check (auth.uid() is not null);

create policy profiles_owner on profiles for all
  using (auth.uid() = user_id) with check (auth.uid() = user_id);

create policy folders_owner on folders for all
  using (auth.uid() = user_id) with check (auth.uid() = user_id);

create policy items_owner on items for all
  using (auth.uid() = user_id) with check (auth.uid() = user_id);

-- Leitura de itens compartilhados via collection (a escrita continua do dono).
create policy items_shared_read on items for select
  using (collection_id in (
    select collection_id from collection_members where user_id = auth.uid()
  ));

create policy collections_owner on collections for all
  using (auth.uid() = owner_id) with check (auth.uid() = owner_id);

create policy members_self on collection_members for select
  using (user_id = auth.uid());

-- ── Realtime: cada dispositivo assina as próprias linhas de items/folders ──
alter publication supabase_realtime add table items;
alter publication supabase_realtime add table folders;
