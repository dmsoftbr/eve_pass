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
