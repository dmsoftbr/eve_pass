// The ONLY module that talks to Supabase. It moves ciphertext (bytea envelopes)
// and auth around; it never sees key material. Postgres `bytea` values travel as
// PostgREST hex literals (`\x…`); helpers below convert to/from the base64 the
// Rust backend speaks.
import { createClient } from "@supabase/supabase-js";
import type { KdfParams, NewAccountJs, RemoteRow } from "./ipc";

const url = import.meta.env.VITE_SUPABASE_URL;
const anon = import.meta.env.VITE_SUPABASE_ANON_KEY;

export const supabase = createClient(url ?? "", anon ?? "", {
  auth: { persistSession: true, autoRefreshToken: true },
});

export function isConfigured(): boolean {
  return Boolean(url && anon);
}

// ── bytea <-> base64 ────────────────────────────────────────────────────────

function b64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}
function bytesToB64(bytes: Uint8Array): string {
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}
function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}
function hexToBytes(hex: string): Uint8Array {
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.substr(i * 2, 2), 16);
  return out;
}

export function b64ToBytea(b64: string): string {
  return "\\x" + bytesToHex(b64ToBytes(b64));
}
export function byteaToB64(bytea: string): string {
  const hex = bytea.startsWith("\\x") ? bytea.slice(2) : bytea;
  return bytesToB64(hexToBytes(hex));
}

// ── prelogin ────────────────────────────────────────────────────────────────

export async function preloginParams(
  email: string,
): Promise<{ saltB64: string; params: KdfParams; wrappedVaultKeyRecoveryB64: string | null }> {
  const { data, error } = await supabase
    .from("login_params")
    .select("kdf_salt,kdf_params,wrapped_vault_key_recovery")
    .eq("email", email)
    .maybeSingle();
  if (error) throw new Error(error.message);
  if (!data) throw new Error("conta não encontrada");
  return {
    saltB64: byteaToB64(data.kdf_salt as string),
    params: data.kdf_params as KdfParams,
    wrappedVaultKeyRecoveryB64: data.wrapped_vault_key_recovery
      ? byteaToB64(data.wrapped_vault_key_recovery as string)
      : null,
  };
}

// ── signup ──────────────────────────────────────────────────────────────────

export async function signUp(email: string, authKeyB64: string): Promise<string> {
  const { data, error } = await supabase.auth.signUp({ email, password: authKeyB64 });
  if (error) throw new Error(error.message);
  if (!data.session || !data.user) {
    throw new Error(
      "signup sem sessão — desligue a confirmação de e-mail no Supabase (Auth → Email → Confirm email OFF)",
    );
  }
  return data.user.id;
}

export async function insertLoginParams(email: string, acc: NewAccountJs): Promise<void> {
  const { error } = await supabase.from("login_params").insert({
    email,
    kdf_salt: b64ToBytea(acc.kdf_salt_b64),
    kdf_params: acc.kdf_params,
    // Readable pre-auth so the recovery flow can reach it (PRD Fase 4 §9).
    wrapped_vault_key_recovery: b64ToBytea(acc.wrapped_vault_key_recovery_b64),
  });
  if (error) throw new Error(error.message);
}

export async function insertProfile(userId: string, acc: NewAccountJs): Promise<void> {
  const { error } = await supabase.from("profiles").insert({
    user_id: userId,
    kdf_salt: b64ToBytea(acc.kdf_salt_b64),
    kdf_params: acc.kdf_params,
    wrapped_vault_key: b64ToBytea(acc.wrapped_vault_key_b64),
    wrapped_vault_key_recovery: b64ToBytea(acc.wrapped_vault_key_recovery_b64),
    public_key: b64ToBytea(acc.public_key_b64),
    signing_public_key: b64ToBytea(acc.signing_public_key_b64),
    wrapped_private_keys: b64ToBytea(acc.wrapped_private_keys_b64),
  });
  if (error) throw new Error(error.message);
}

/// Publish the user's public keys (readable by any authenticated user) so others
/// can HPKE-wrap collection keys for them.
export async function insertPublicKeys(userId: string, email: string, acc: NewAccountJs): Promise<void> {
  const { error } = await supabase.from("public_keys").insert({
    user_id: userId,
    email,
    public_key: b64ToBytea(acc.public_key_b64),
    signing_public_key: b64ToBytea(acc.signing_public_key_b64),
  });
  if (error) throw new Error(error.message);
}

export async function getMyPublicKeys(
  userId: string,
): Promise<{ publicKeyB64: string; signingPublicKeyB64: string }> {
  const { data, error } = await supabase
    .from("public_keys")
    .select("public_key,signing_public_key")
    .eq("user_id", userId)
    .single();
  if (error) throw new Error(error.message);
  return {
    publicKeyB64: byteaToB64(data.public_key as string),
    signingPublicKeyB64: byteaToB64(data.signing_public_key as string),
  };
}

export async function getPublicKeyByEmail(
  email: string,
): Promise<{ userId: string; publicKeyB64: string; signingPublicKeyB64: string }> {
  const { data, error } = await supabase
    .from("public_keys")
    .select("user_id,public_key,signing_public_key")
    .eq("email", email)
    .maybeSingle();
  if (error) throw new Error(error.message);
  if (!data) throw new Error("usuário não encontrado (precisa ter conta)");
  return {
    userId: data.user_id as string,
    publicKeyB64: byteaToB64(data.public_key as string),
    signingPublicKeyB64: byteaToB64(data.signing_public_key as string),
  };
}

// ── login ───────────────────────────────────────────────────────────────────

export async function signIn(email: string, authKeyB64: string): Promise<string> {
  const { data, error } = await supabase.auth.signInWithPassword({ email, password: authKeyB64 });
  if (error) throw new Error(error.message);
  if (!data.user) throw new Error("login sem usuário");
  return data.user.id;
}

export async function getProfileKeys(): Promise<{
  wrappedVaultKeyB64: string;
  wrappedPrivateKeysB64: string;
}> {
  const { data, error } = await supabase
    .from("profiles")
    .select("wrapped_vault_key,wrapped_private_keys")
    .single();
  if (error) throw new Error(error.message);
  return {
    wrappedVaultKeyB64: byteaToB64(data.wrapped_vault_key as string),
    wrappedPrivateKeysB64: byteaToB64(data.wrapped_private_keys as string),
  };
}

// ── collections ──────────────────────────────────────────────────────────────

export async function insertCollection(collectionId: string, nameCiphertextB64: string): Promise<void> {
  const { data: auth } = await supabase.auth.getUser();
  const { error } = await supabase.from("collections").insert({
    id: collectionId,
    owner_id: auth.user?.id,
    ciphertext: b64ToBytea(nameCiphertextB64),
  });
  if (error) throw new Error(error.message);
}

export async function upsertCollectionMember(
  collectionId: string,
  userId: string,
  wrappedCollectionKeyB64: string,
  senderSigningPubB64: string,
  role: "admin" | "writer" | "reader",
): Promise<void> {
  const { error } = await supabase.from("collection_members").upsert({
    collection_id: collectionId,
    user_id: userId,
    wrapped_collection_key: b64ToBytea(wrappedCollectionKeyB64),
    sender_signing_pub: b64ToBytea(senderSigningPubB64),
    role,
  });
  if (error) throw new Error(error.message);
}

export interface CollectionMemberRow {
  collectionId: string;
  wrappedCollectionKeyB64: string;
  senderSigningPubB64: string;
}

/// The current user's own collection_members rows (for loading collection keys).
export async function fetchMyCollectionMembers(userId: string): Promise<CollectionMemberRow[]> {
  const { data, error } = await supabase
    .from("collection_members")
    .select("collection_id,wrapped_collection_key,sender_signing_pub")
    .eq("user_id", userId);
  if (error) throw new Error(error.message);
  return (data ?? []).map((r) => ({
    collectionId: r.collection_id as string,
    wrappedCollectionKeyB64: byteaToB64(r.wrapped_collection_key as string),
    senderSigningPubB64: byteaToB64((r.sender_signing_pub ?? "\\x") as string),
  }));
}

export async function fetchCollections(): Promise<{ id: string; nameCtB64: string }[]> {
  const { data, error } = await supabase.from("collections").select("id,ciphertext");
  if (error) throw new Error(error.message);
  return (data ?? []).map((r) => ({ id: r.id as string, nameCtB64: byteaToB64(r.ciphertext as string) }));
}

/// Delete a collection: its items first (FK), then the collection (members
/// cascade). Only the owner/admins pass RLS.
export async function deleteCollectionServer(collectionId: string): Promise<void> {
  const items = await supabase.from("items").delete().eq("collection_id", collectionId);
  if (items.error) throw new Error(items.error.message);
  const coll = await supabase.from("collections").delete().eq("id", collectionId);
  if (coll.error) throw new Error(coll.error.message);
}

export async function signOut(): Promise<void> {
  await supabase.auth.signOut();
}

// ── item / folder writes ─────────────────────────────────────────────────────

export async function upsertRow(
  table: "items" | "folders",
  userId: string,
  id: string,
  envelopeB64: string,
  revision: number,
  collectionId: string | null = null,
): Promise<void> {
  const row: Record<string, unknown> = {
    id,
    user_id: userId,
    ciphertext: b64ToBytea(envelopeB64),
    revision,
    updated_at: new Date().toISOString(),
  };
  if (table === "items") row.collection_id = collectionId;
  const { error } = await supabase.from(table).upsert(row);
  if (error) throw new Error(error.message);
}

export async function softDeleteRow(table: "items" | "folders", id: string): Promise<void> {
  const { error } = await supabase
    .from(table)
    .update({ deleted_at: new Date().toISOString() })
    .eq("id", id);
  if (error) throw new Error(error.message);
}

// ── warm-up + realtime ────────────────────────────────────────────────────────

function rowToRemote(kind: "item" | "folder", r: Record<string, unknown>): RemoteRow {
  return {
    kind,
    id: r.id as string,
    envelope_b64: byteaToB64(r.ciphertext as string),
    revision: (r.revision as number) ?? 1,
    updated_at: (r.updated_at as string) ?? "",
    deleted: r.deleted_at != null,
    collection_id: (r.collection_id as string) ?? null,
  };
}

export async function fetchAllRows(): Promise<RemoteRow[]> {
  const out: RemoteRow[] = [];
  // items include shared ones (RLS lets members read collection items).
  const items = await supabase.from("items").select("id,ciphertext,revision,updated_at,deleted_at,collection_id");
  if (items.error) throw new Error(items.error.message);
  for (const r of items.data ?? []) out.push(rowToRemote("item", r as Record<string, unknown>));

  const folders = await supabase.from("folders").select("id,ciphertext,revision,updated_at,deleted_at");
  if (folders.error) throw new Error(folders.error.message);
  for (const r of folders.data ?? []) out.push(rowToRemote("folder", r as Record<string, unknown>));
  return out;
}

export function subscribeRealtime(userId: string, onRow: (row: RemoteRow) => void): () => void {
  const channel = supabase.channel(`vault-${userId}`);
  for (const [table, kind] of [
    ["items", "item"],
    ["folders", "folder"],
  ] as const) {
    channel.on(
      "postgres_changes",
      { event: "*", schema: "public", table, filter: `user_id=eq.${userId}` },
      (payload) => {
        const r = (payload.new ?? payload.old) as Record<string, unknown>;
        if (r && r.id) onRow(rowToRemote(kind, r));
      },
    );
  }
  channel.subscribe();
  return () => {
    supabase.removeChannel(channel);
  };
}
