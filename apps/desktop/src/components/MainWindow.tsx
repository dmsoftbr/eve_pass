import { useEffect, useMemo, useRef, useState } from "react";
import Fuse from "fuse.js";
import { useVault } from "../state/vault";
import { Sidebar, type Filter } from "./Sidebar";
import { ItemList } from "./ItemList";
import { ItemDetail } from "./ItemDetail";
import { ItemForm } from "./ItemForm";
import { SettingsModal } from "./SettingsModal";
import { ImportModal } from "./ImportModal";
import { ShareModal } from "./ShareModal";

export function MainWindow() {
  const { items, lock, health, breachedIds } = useVault();
  const [filter, setFilter] = useState<Filter>({ kind: "all" });
  const [query, setQuery] = useState("");
  const [formOpen, setFormOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [shareCollectionId, setShareCollectionId] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  // ⌘K / Ctrl+K focuses search (command-palette proper is Fase 2).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        searchRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const byFilter = useMemo(() => {
    if (filter.kind === "all") return items;
    if (filter.kind === "folder") return items.filter((i) => i.folders.includes(filter.value));
    if (filter.kind === "tag") return items.filter((i) => i.tags.includes(filter.value));
    if (filter.kind === "collection")
      return items.filter(
        (i) => i.collection_id === filter.value && (!filter.folder || i.folders.includes(filter.folder)),
      );
    // smart view
    let ids: string[];
    if (filter.value === "weak") ids = health?.weak ?? [];
    else if (filter.value === "no_totp") ids = health?.no_totp ?? [];
    else if (filter.value === "reused") ids = (health?.reused ?? []).flat();
    else ids = breachedIds;
    const set = new Set(ids);
    return items.filter((i) => set.has(i.id));
  }, [items, filter, health, breachedIds]);

  const fuse = useMemo(
    () => new Fuse(byFilter, { keys: ["title", "username", "url", "tags"], threshold: 0.4 }),
    [byFilter],
  );
  const visible = query.trim() ? fuse.search(query).map((r) => r.item) : byFilter;

  function openCreate() {
    setEditingId(null);
    setFormOpen(true);
  }
  function openEdit(id: string) {
    setEditingId(id);
    setFormOpen(true);
  }

  return (
    <div className="grid h-full grid-cols-[240px_320px_1fr]">
      <Sidebar
        filter={filter}
        onFilter={setFilter}
        onLock={() => void lock()}
        onOpenSettings={() => setSettingsOpen(true)}
        onOpenImport={() => setImportOpen(true)}
        onShareCollection={(id) => setShareCollectionId(id)}
      />

      <div className="flex min-h-0 flex-col border-r border-line">
        <div className="flex items-center gap-2 border-b border-line p-3">
          <input
            ref={searchRef}
            className="field"
            placeholder="Buscar  (⌘K)"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <button className="btn-primary shrink-0" onClick={openCreate} title="Novo item">
            +
          </button>
        </div>
        <ItemList items={visible} onEdit={openEdit} />
      </div>

      <ItemDetail onEdit={openEdit} />

      {formOpen && <ItemForm editingId={editingId} onClose={() => setFormOpen(false)} />}
      {settingsOpen && <SettingsModal onClose={() => setSettingsOpen(false)} />}
      {importOpen && <ImportModal onClose={() => setImportOpen(false)} />}
      {shareCollectionId && (
        <ShareModal collectionId={shareCollectionId} onClose={() => setShareCollectionId(null)} />
      )}
    </div>
  );
}
