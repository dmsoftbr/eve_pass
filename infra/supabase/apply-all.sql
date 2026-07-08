-- EVEPass — esquema completo (migrations 0001 + 0002). Cole no SQL Editor do Supabase e Run.

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

-- ========================================================================

-- EVEPass · Fase 4 — compartilhamento por collections (E2E via HPKE).
--
-- Adiciona chaves públicas legíveis (para embrulhar a collectionKey), funções
-- auxiliares de pertencimento/papel, e as políticas RLS de collections, membros
-- e itens compartilhados. O servidor continua sem ver nenhuma chave em claro —
-- só embrulhos HPKE e ciphertext.

-- ── Chaves públicas legíveis (separadas do profiles, que é só do dono) ───────
create table public_keys (
  user_id            uuid primary key references auth.users(id) on delete cascade,
  email              text unique not null,
  public_key         bytea not null,          -- X25519 (para HPKE)
  signing_public_key bytea not null,          -- Ed25519 (autentica quem compartilha)
  created_at         timestamptz default now()
);
alter table public_keys enable row level security;

-- Qualquer usuário autenticado lê chaves públicas (para compartilhar por email).
create policy public_keys_read on public_keys for select
  using (auth.role() = 'authenticated');
create policy public_keys_self on public_keys for insert
  with check (auth.uid() = user_id);

-- ── Funções auxiliares (SECURITY DEFINER evita recursão de RLS) ──────────────
create or replace function is_member(cid uuid) returns boolean
  language sql security definer stable as $$
  select exists(select 1 from collection_members
                where collection_id = cid and user_id = auth.uid());
$$;

create or replace function can_write(cid uuid) returns boolean
  language sql security definer stable as $$
  select exists(select 1 from collection_members
                where collection_id = cid and user_id = auth.uid()
                  and role in ('admin','writer'));
$$;

create or replace function is_admin(cid uuid) returns boolean
  language sql security definer stable as $$
  select exists(select 1 from collection_members
                where collection_id = cid and user_id = auth.uid()
                  and role = 'admin');
$$;

-- ── Itens compartilhados (RLS por collection, além do dono da Fase 0) ────────
create policy items_collection_read on items for select
  using (collection_id is not null and is_member(collection_id));
create policy items_collection_write on items for all
  using (collection_id is not null and can_write(collection_id))
  with check (collection_id is not null and can_write(collection_id));

-- ── Collections ──────────────────────────────────────────────────────────────
create policy collections_read on collections for select
  using (owner_id = auth.uid() or is_member(id));
-- (a policy collections_owner de escrita já veio na 0001)

-- ── Membros (cada um vê o próprio; admin gere todos) ─────────────────────────
create policy members_read on collection_members for select
  using (user_id = auth.uid() or is_admin(collection_id));
create policy members_admin on collection_members for all
  using (is_admin(collection_id)) with check (is_admin(collection_id));

-- Coluna para autenticar o compartilhador (Ed25519 pub de quem embrulhou).
alter table collection_members
  add column if not exists sender_signing_pub bytea;

-- Recuperação: a vaultKey embrulhada pelo Recovery Code precisa ser legível
-- ANTES de autenticar (o usuário esqueceu a senha) — vive no login_params
-- público, ao lado do salt. Popular no signup.
alter table login_params
  add column if not exists wrapped_vault_key_recovery bytea;

-- Realtime também para collections/membros (sync de compartilhamento).
alter publication supabase_realtime add table collections;
alter publication supabase_realtime add table collection_members;

-- ========================================================================

-- EVEPass · Fix — permitir que o DONO da collection gerencie membros.
--
-- A política `members_admin` (0002) exige is_admin(collection_id) para inserir em
-- collection_members. Mas ao CRIAR uma collection, o dono precisa inserir a
-- própria linha de admin (com a chave embrulhada pra ele) — e nesse momento
-- ainda não existe nenhum membro, então is_admin() é falso e o insert é
-- bloqueado, quebrando a criação. Adicionamos is_owner() ao gate.

create or replace function is_owner(cid uuid) returns boolean
  language sql security definer stable as $$
  select exists(select 1 from collections where id = cid and owner_id = auth.uid());
$$;

drop policy if exists members_admin on collection_members;
create policy members_admin on collection_members for all
  using (is_owner(collection_id) or is_admin(collection_id))
  with check (is_owner(collection_id) or is_admin(collection_id));
