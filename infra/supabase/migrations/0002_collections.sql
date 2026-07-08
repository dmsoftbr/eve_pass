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
