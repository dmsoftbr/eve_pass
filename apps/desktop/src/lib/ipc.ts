// Typed wrappers over the Tauri command surface. Keys never cross this boundary
// — only ciphertext envelopes (base64) and, for display/edit, item plaintext.
import { invoke } from "@tauri-apps/api/core";

export interface KdfParams {
  alg: string;
  m: number;
  t: number;
  p: number;
}

export interface NewAccountJs {
  kdf_salt_b64: string;
  kdf_params: KdfParams;
  auth_key_b64: string;
  wrapped_vault_key_b64: string;
  wrapped_vault_key_recovery_b64: string;
  recovery_code: string;
  public_key_b64: string;
  signing_public_key_b64: string;
  wrapped_private_keys_b64: string;
}

export interface BeginLoginJs {
  auth_key_b64: string;
  login_token: string;
}

export interface ItemView {
  id: string;
  type: string;
  title: string;
  username: string;
  url: string;
  has_totp: boolean;
  folders: string[];
  tags: string[];
  revision: number;
  updated_at: string;
  collection_id: string | null;
}

export interface FolderView {
  id: string;
  name: string;
  parent_id: string | null;
  revision: number;
}

export interface Saved {
  id: string;
  envelope_b64: string;
  revision: number;
  deleted: boolean;
  collection_id: string | null;
}

export interface RemoteRow {
  kind: "item" | "folder";
  id: string;
  envelope_b64: string;
  revision: number;
  updated_at: string;
  deleted: boolean;
  collection_id: string | null;
}

export interface SyncResult {
  updated: string[];
  conflicts: string[];
}

export interface PendingRow {
  kind: "item" | "folder";
  id: string;
  envelope_b64: string;
  revision: number;
  deleted: boolean;
  collection_id: string | null;
}

export interface NewCollectionJs {
  collection_id: string;
  name_ciphertext_b64: string;
}

export interface MemberRowJs {
  collection_id: string;
  wrapped_collection_key_b64: string;
  sender_signing_pub_b64: string;
}

export interface PasswordResetJs {
  auth_key_b64: string;
  wrapped_vault_key_b64: string;
  wrapped_vault_key_recovery_b64: string;
  recovery_code: string;
}

export interface HealthReport {
  weak: string[];
  reused: string[][];
  no_totp: string[];
}

export interface TotpLive {
  code: string;
  seconds_remaining: number;
}

export interface PaletteHit {
  id: string;
  title: string;
  username: string;
  has_totp: boolean;
}

export interface Settings {
  auto_lock_minutes: number;
  clipboard_clear_seconds: number;
  launch_at_login: boolean;
  global_hotkey: string;
  theme: "light" | "dark" | "system";
}

export const ipc = {
  vaultStatus: () => invoke<string>("vault_status"),
  createAccount: (password: string) => invoke<NewAccountJs>("create_account", { password }),
  beginLogin: (password: string, saltB64: string, params: KdfParams) =>
    invoke<BeginLoginJs>("begin_login", { password, saltB64, params }),
  completeLogin: (
    loginToken: string,
    wrappedVaultKeyB64: string,
    wrappedPrivateKeysB64: string,
    userId: string,
  ) => invoke<void>("complete_login", { loginToken, wrappedVaultKeyB64, wrappedPrivateKeysB64, userId }),
  lock: () => invoke<void>("lock"),

  listItems: () => invoke<ItemView[]>("list_items"),
  getItem: (id: string) => invoke<string>("get_item", { id }),
  saveItem: (id: string | null, itemJson: string, collectionId: string | null = null) =>
    invoke<Saved>("save_item", { id, itemJson, collectionId }),
  deleteItem: (id: string) => invoke<Saved>("delete_item", { id }),
  markSynced: (kind: "item" | "folder", id: string, revision: number) =>
    invoke<void>("mark_synced", { kind, id, revision }),
  copyField: (id: string, field: string) => invoke<void>("copy_field", { id, field }),

  listFolders: () => invoke<FolderView[]>("list_folders"),
  saveFolder: (id: string | null, name: string, parentId: string | null) =>
    invoke<Saved>("save_folder", { id, name, parentId }),
  deleteFolder: (id: string) => invoke<Saved>("delete_folder", { id }),

  applyRemoteChanges: (rows: RemoteRow[]) => invoke<SyncResult>("apply_remote_changes", { rows }),
  pendingUploads: () => invoke<PendingRow[]>("pending_uploads"),

  genPassword: (length: number, upper: boolean, lower: boolean, digits: boolean, symbols: boolean) =>
    invoke<string>("gen_password", { length, upper, lower, digits, symbols }),

  // Fase 2
  vaultHealth: () => invoke<HealthReport>("vault_health"),
  breachPrefixes: () => invoke<string[]>("breach_prefixes"),
  resolveBreaches: (ranges: { prefix: string; body: string }[]) =>
    invoke<string[]>("resolve_breaches", { ranges }),
  itemTotp: (id: string) => invoke<TotpLive>("item_totp", { id }),
  paletteSearch: (query: string) => invoke<PaletteHit[]>("palette_search", { query }),
  saveItemsBatch: (itemsJson: string[]) => invoke<Saved[]>("save_items_batch", { itemsJson }),
  saveFoldersBatch: (folders: [string, string | null][]) =>
    invoke<Saved[]>("save_folders_batch", { folders }),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<void>("set_settings", { settings }),
  pingActivity: () => invoke<void>("ping_activity"),

  // Fase 4 (collections + recovery)
  createCollection: (name: string) => invoke<NewCollectionJs>("create_collection", { name }),
  loadCollectionKeys: (members: MemberRowJs[]) =>
    invoke<void>("load_collection_keys", { members }),
  wrapCollectionKeyFor: (collectionId: string, recipientPubB64: string) =>
    invoke<string>("wrap_collection_key_for", { collectionId, recipientPubB64 }),
  decryptCollectionName: (collectionId: string, nameCtB64: string) =>
    invoke<string>("decrypt_collection_name", { collectionId, nameCtB64 }),
  rotateCollectionKey: (collectionId: string, name: string) =>
    invoke<string>("rotate_collection_key", { collectionId, name }),
  publicKeyFingerprint: (pubKeyB64: string) =>
    invoke<string>("public_key_fingerprint", { pubKeyB64 }),
  deleteCollectionCache: (collectionId: string) =>
    invoke<void>("delete_collection_cache", { collectionId }),
  resetPassword: (newPassword: string, saltB64: string, params: KdfParams) =>
    invoke<PasswordResetJs>("reset_password", { newPassword, saltB64, params }),
  unlockWithRecovery: (recoveryCode: string, wrappedVaultKeyRecoveryB64: string) =>
    invoke<void>("unlock_with_recovery", { recoveryCode, wrappedVaultKeyRecoveryB64 }),
};
