// Sync glue: warm-up pull, Realtime subscription, and the offline upload queue.
// Reconciliation itself (LWW + conflict copy) happens in Rust; this only shuttles
// ciphertext between Supabase and the cache and confirms uploads.
import { ipc, type Saved } from "./ipc";
import * as sb from "./supabase";

/** Push everything still marked dirty in the cache (e.g. edits made offline). */
export async function drainPending(userId: string): Promise<void> {
  const pending = await ipc.pendingUploads();
  for (const p of pending) {
    const table = p.kind === "item" ? "items" : "folders";
    try {
      if (p.deleted) await sb.softDeleteRow(table, p.id);
      else await sb.upsertRow(table, userId, p.id, p.envelope_b64, p.revision, p.collection_id);
      await ipc.markSynced(p.kind, p.id, p.revision);
    } catch {
      // Still offline / transient — leave dirty for the next drain.
      return;
    }
  }
}

/** Push a single just-saved row; on failure it stays dirty for a later drain. */
export async function pushSaved(userId: string, kind: "item" | "folder", saved: Saved): Promise<void> {
  const table = kind === "item" ? "items" : "folders";
  try {
    if (saved.deleted) await sb.softDeleteRow(table, saved.id);
    else await sb.upsertRow(table, userId, saved.id, saved.envelope_b64, saved.revision, saved.collection_id);
    await ipc.markSynced(kind, saved.id, saved.revision);
  } catch {
    /* offline — drainPending will retry */
  }
}

/** Warm-up pull + live subscription. Returns an unsubscribe fn. */
export async function startSync(userId: string, onChanged: () => void): Promise<() => void> {
  const rows = await sb.fetchAllRows();
  if (rows.length) await ipc.applyRemoteChanges(rows);
  await drainPending(userId);
  onChanged();

  return sb.subscribeRealtime(userId, async (row) => {
    await ipc.applyRemoteChanges([row]);
    onChanged();
  });
}
