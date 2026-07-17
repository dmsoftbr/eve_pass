// Central vault state: unlock status, the decrypted item/folder views (plaintext
// for display only), and the actions the UI calls. Key material stays in Rust.
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { emit, listen } from "@tauri-apps/api/event";
import { ipc, type FolderView, type HealthReport, type ItemView } from "../lib/ipc";
import { doLock, doLogin, doSignup, rememberedEmail } from "../lib/auth";
import { drainPending, pushSaved, startSync } from "../lib/sync";
import { computeBreached } from "../lib/health";
import * as sb from "../lib/supabase";

type Status = "locked" | "unlocked";

interface VaultContextValue {
  status: Status;
  items: ItemView[];
  folders: FolderView[];
  health: HealthReport | null;
  breachedIds: string[];
  refreshBreached: () => Promise<void>;
  selectedId: string | null;
  select: (id: string | null) => void;
  refresh: () => Promise<void>;
  signup: (email: string, password: string) => Promise<{ recoveryCode: string }>;
  login: (email: string, password: string) => Promise<void>;
  lock: () => Promise<void>;
  saveItem: (id: string | null, itemJson: string, collectionId?: string | null) => Promise<string>;
  deleteItem: (id: string) => Promise<void>;
  collections: { id: string; name: string }[];
  createCollection: (name: string) => Promise<void>;
  deleteCollection: (collectionId: string) => Promise<void>;
  ownSigningPubB64: string | null;
  saveFolder: (id: string | null, name: string, parentId: string | null) => Promise<void>;
  deleteFolder: (id: string) => Promise<void>;
  getItem: (id: string) => Promise<string>;
  copyField: (id: string, field: string) => Promise<void>;
  importFolders: (names: string[]) => Promise<string[]>;
  importItems: (itemsJson: string[]) => Promise<void>;
}

const VaultContext = createContext<VaultContextValue | null>(null);

export function VaultProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<Status>("locked");
  const [items, setItems] = useState<ItemView[]>([]);
  const [folders, setFolders] = useState<FolderView[]>([]);
  const [health, setHealth] = useState<HealthReport | null>(null);
  const [breachedIds, setBreachedIds] = useState<string[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [collections, setCollections] = useState<{ id: string; name: string }[]>([]);
  const [ownSigningPubB64, setOwnSigningPubB64] = useState<string | null>(null);
  const userId = useRef<string | null>(null);
  const ownPub = useRef<string | null>(null);
  const unsub = useRef<(() => void) | null>(null);

  const refresh = useCallback(async () => {
    const [i, f] = await Promise.all([ipc.listItems(), ipc.listFolders()]);
    setItems(i);
    setFolders(f);
    try {
      setHealth(await ipc.vaultHealth());
    } catch {
      /* locked mid-refresh */
    }
  }, []);

  const refreshBreached = useCallback(async () => {
    try {
      setBreachedIds(await computeBreached());
    } catch {
      /* offline / locked */
    }
  }, []);

  const refreshCollections = useCallback(async () => {
    try {
      const rows = await sb.fetchCollections();
      const named = await Promise.all(
        rows.map(async (r) => {
          try {
            return { id: r.id, name: await ipc.decryptCollectionName(r.id, r.nameCtB64) };
          } catch {
            return { id: r.id, name: "(sem acesso)" };
          }
        }),
      );
      setCollections(named);
    } catch {
      /* no collections */
    }
  }, []);

  const onUnlocked = useCallback(
    async (uid: string) => {
      userId.current = uid;
      setStatus("unlocked");
      try {
        const keys = await sb.getMyPublicKeys(uid);
        ownPub.current = keys.publicKeyB64;
        setOwnSigningPubB64(keys.signingPublicKeyB64);
      } catch {
        /* keys optional */
      }
      unsub.current = await startSync(uid, () => void refresh());
      await refresh();
      await refreshCollections();
    },
    [refresh, refreshCollections],
  );

  const createCollection = useCallback(
    async (name: string) => {
      const nc = await ipc.createCollection(name);
      await sb.insertCollection(nc.collection_id, nc.name_ciphertext_b64);
      // Add self as admin: wrap the collection key for our own public key.
      if (userId.current && ownPub.current && ownSigningPubB64) {
        const wrapped = await ipc.wrapCollectionKeyFor(nc.collection_id, ownPub.current);
        await sb.upsertCollectionMember(
          nc.collection_id,
          userId.current,
          wrapped,
          ownSigningPubB64,
          "admin",
        );
      }
      await refreshCollections();
    },
    [ownSigningPubB64, refreshCollections],
  );

  const deleteCollection = useCallback(
    async (collectionId: string) => {
      await sb.deleteCollectionServer(collectionId); // items + collection (server)
      await ipc.deleteCollectionCache(collectionId); // its items in the local cache
      await refresh();
      await refreshCollections();
    },
    [refresh, refreshCollections],
  );

  const signup = useCallback(
    async (email: string, password: string) => {
      const { userId: uid, recoveryCode } = await doSignup(email, password);
      await onUnlocked(uid);
      return { recoveryCode };
    },
    [onUnlocked],
  );

  const login = useCallback(
    async (email: string, password: string) => {
      const uid = await doLogin(email, password);
      await onUnlocked(uid);
    },
    [onUnlocked],
  );

  const resetLockedUI = useCallback(() => {
    unsub.current?.();
    unsub.current = null;
    userId.current = null;
    ownPub.current = null;
    setItems([]);
    setFolders([]);
    setHealth(null);
    setBreachedIds([]);
    setCollections([]);
    setOwnSigningPubB64(null);
    setSelectedId(null);
    setStatus("locked");
  }, []);

  const lock = useCallback(async () => {
    await doLock();
    resetLockedUI();
  }, [resetLockedUI]);

  const saveItem = useCallback(
    async (id: string | null, itemJson: string, collectionId: string | null = null) => {
      const saved = await ipc.saveItem(id, itemJson, collectionId);
      await refresh();
      if (userId.current) await pushSaved(userId.current, "item", saved);
      return saved.id;
    },
    [refresh],
  );

  const deleteItem = useCallback(
    async (id: string) => {
      const saved = await ipc.deleteItem(id);
      if (selectedId === id) setSelectedId(null);
      await refresh();
      if (userId.current) await pushSaved(userId.current, "item", saved);
    },
    [refresh, selectedId],
  );

  const saveFolder = useCallback(
    async (id: string | null, name: string, parentId: string | null) => {
      const saved = await ipc.saveFolder(id, name, parentId);
      await refresh();
      if (userId.current) await pushSaved(userId.current, "folder", saved);
    },
    [refresh],
  );

  const deleteFolder = useCallback(
    async (id: string) => {
      const saved = await ipc.deleteFolder(id);
      await refresh();
      if (userId.current) await pushSaved(userId.current, "folder", saved);
    },
    [refresh],
  );

  const getItem = useCallback((id: string) => ipc.getItem(id), []);
  const copyField = useCallback((id: string, field: string) => ipc.copyField(id, field), []);

  // Import: create folders (returns their new ids, in order) then items. Both
  // are pushed to the server via the sync queue.
  const importFolders = useCallback(
    async (names: string[]) => {
      if (names.length === 0) return [];
      const saved = await ipc.saveFoldersBatch(names.map((n) => [n, null] as [string, null]));
      if (userId.current) for (const s of saved) await pushSaved(userId.current, "folder", s);
      await refresh();
      return saved.map((s) => s.id);
    },
    [refresh],
  );

  const importItems = useCallback(
    async (itemsJson: string[]) => {
      if (itemsJson.length === 0) return;
      const saved = await ipc.saveItemsBatch(itemsJson);
      if (userId.current) for (const s of saved) await pushSaved(userId.current, "item", s);
      await refresh();
    },
    [refresh],
  );

  // Lock on window close (best-effort).
  useEffect(() => {
    const handler = () => void lock();
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, [lock]);

  // Auto-lock: the Rust timer emits `vault-locked`; reset the UI to unlock.
  useEffect(() => {
    const un = listen("vault-locked", () => {
      void sb.signOut();
      resetLockedUI();
    });
    return () => {
      void un.then((f) => f());
    };
  }, [resetLockedUI]);

  // The command palette (other window) asks us to reveal an item.
  useEffect(() => {
    const un = listen<string>("palette-open-item", (e) => setSelectedId(e.payload));
    return () => {
      void un.then((f) => f());
    };
  }, []);

  // The browser-extension native host saved a credential straight into the
  // cache (dirty); flush it to the server and refresh the list.
  useEffect(() => {
    const un = listen("host-item-saved", async () => {
      if (userId.current) await drainPending(userId.current);
      await refresh();
    });
    return () => {
      void un.then((f) => f());
    };
  }, [refresh]);

  // The command palette can unlock the vault inline. It has no network context
  // of its own, so it forwards the master password here (in-process event) and
  // we run the normal login with the remembered e-mail. The password never
  // touches the server as-is — begin_login turns it into the authKey in Rust.
  useEffect(() => {
    const un = listen<{ password: string }>("palette-unlock", async (e) => {
      const email = rememberedEmail();
      if (!email) {
        void emit("palette-unlock-error", {
          message: "Ative “Lembrar e-mail” e faça login uma vez para destravar pelo palette.",
        });
        return;
      }
      try {
        await login(email, e.payload.password);
        void emit("vault-unlocked", {});
      } catch (err) {
        void emit("palette-unlock-error", {
          message: err instanceof Error ? err.message : String(err),
        });
      }
    });
    return () => {
      void un.then((f) => f());
    };
  }, [login]);

  // Activity heartbeat (throttled) so the Rust auto-lock knows we're active.
  useEffect(() => {
    if (status !== "unlocked") return;
    let last = 0;
    const onActivity = () => {
      const now = Date.now();
      if (now - last > 5000) {
        last = now;
        void ipc.pingActivity();
      }
    };
    window.addEventListener("mousemove", onActivity);
    window.addEventListener("keydown", onActivity);
    return () => {
      window.removeEventListener("mousemove", onActivity);
      window.removeEventListener("keydown", onActivity);
    };
  }, [status]);

  const value = useMemo<VaultContextValue>(
    () => ({
      status,
      items,
      folders,
      health,
      breachedIds,
      refreshBreached,
      selectedId,
      select: setSelectedId,
      refresh,
      signup,
      login,
      lock,
      saveItem,
      deleteItem,
      saveFolder,
      deleteFolder,
      getItem,
      copyField,
      importFolders,
      importItems,
      collections,
      createCollection,
      deleteCollection,
      ownSigningPubB64,
    }),
    [
      status,
      items,
      folders,
      health,
      breachedIds,
      refreshBreached,
      selectedId,
      refresh,
      signup,
      login,
      lock,
      saveItem,
      deleteItem,
      saveFolder,
      deleteFolder,
      getItem,
      copyField,
      importFolders,
      importItems,
      collections,
      createCollection,
      deleteCollection,
      ownSigningPubB64,
    ],
  );

  return <VaultContext.Provider value={value}>{children}</VaultContext.Provider>;
}

export function useVault(): VaultContextValue {
  const ctx = useContext(VaultContext);
  if (!ctx) throw new Error("useVault must be used within VaultProvider");
  return ctx;
}
