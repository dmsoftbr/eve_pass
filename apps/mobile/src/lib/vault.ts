// Vault helpers for the mobile screens (scaffold).
//
// On mobile the local cache reuses the same SQLite schema as the desktop
// (`cache_items`/`cache_folders`), stored in the app's shared container so the
// autofill extension can read it offline. A thin native/JS layer reads the
// envelopes and the core `Session` decrypts them — the vaultKey never leaves Rust.
import type { Session } from "./core";
import { evepass } from "./core";

export interface ItemView {
  id: string;
  title: string;
  username: string;
  url: string;
  hasTotp: boolean;
}

/** Read cached envelopes and decrypt to view models via the Session. */
export async function listItemViews(session: Session): Promise<ItemView[]> {
  const envelopes = await readCachedItemEnvelopes(); // native cache read (scaffold)
  return envelopes.map(({ id, envelope }) => {
    const json = evepass.decryptItem(session, envelope);
    const v = JSON.parse(json);
    return {
      id,
      title: v.title ?? "",
      username: v.username ?? "",
      url: v.url ?? "",
      hasTotp: Boolean(v.totp),
    };
  });
}

// Provided by the native cache module (App Group container / internal storage).
declare function readCachedItemEnvelopes(): Promise<{ id: string; envelope: Uint8Array }[]>;
