import { useMemo, useState } from "react";
import { Download, Lock, Settings, Users } from "lucide-react";
import { useVault } from "../state/vault";
import type { FolderView } from "../lib/ipc";

export type SmartView = "weak" | "reused" | "no_totp" | "breached";

export type Filter =
  | { kind: "all" }
  | { kind: "folder"; value: string }
  | { kind: "tag"; value: string }
  | { kind: "smart"; value: SmartView }
  | { kind: "collection"; value: string; folder?: string };

const SMART_VIEWS: { key: SmartView; label: string }[] = [
  { key: "weak", label: "Senhas fracas" },
  { key: "reused", label: "Reutilizadas" },
  { key: "no_totp", label: "Sem 2FA" },
  { key: "breached", label: "Vazadas" },
];

export function Sidebar({
  filter,
  onFilter,
  onLock,
  onOpenSettings,
  onOpenImport,
  onShareCollection,
}: {
  filter: Filter;
  onFilter: (f: Filter) => void;
  onLock: () => void;
  onOpenSettings: () => void;
  onOpenImport: () => void;
  onShareCollection: (collectionId: string) => void;
}) {
  const {
    folders,
    items,
    saveFolder,
    deleteFolder,
    health,
    breachedIds,
    refreshBreached,
    collections,
    createCollection,
  } = useVault();

  const smartCount = (key: SmartView): number => {
    if (key === "weak") return health?.weak.length ?? 0;
    if (key === "no_totp") return health?.no_totp.length ?? 0;
    if (key === "reused") return (health?.reused ?? []).reduce((n, g) => n + g.length, 0);
    return breachedIds.length;
  };

  const tags = useMemo(() => {
    const set = new Set<string>();
    for (const i of items) for (const t of i.tags) set.add(t);
    return [...set].sort();
  }, [items]);

  const childrenOf = useMemo(() => {
    const map = new Map<string | null, FolderView[]>();
    for (const f of folders) {
      const key = f.parent_id ?? null;
      (map.get(key) ?? map.set(key, []).get(key)!).push(f);
    }
    return map;
  }, [folders]);

  const active = (f: Filter) =>
    filter.kind === f.kind && (f.kind === "all" || (filter as any).value === (f as any).value);

  // Inline name input (the Tauri webview implements neither window.prompt nor
  // window.alert, so errors are shown in-app).
  const [adding, setAdding] = useState<null | "folder" | "collection">(null);
  const [newName, setNewName] = useState("");
  const [addError, setAddError] = useState<string | null>(null);
  const [expandedColls, setExpandedColls] = useState<Record<string, boolean>>({});

  // Folders (and their ancestors) that contain items of a given collection.
  const collectionFolderIds = (collId: string): Set<string> => {
    const folderById = new Map(folders.map((f) => [f.id, f]));
    const all = new Set<string>();
    for (const it of items) {
      if (it.collection_id !== collId) continue;
      for (const fid of it.folders) {
        let cur: string | null = fid;
        while (cur && !all.has(cur)) {
          all.add(cur);
          cur = folderById.get(cur)?.parent_id ?? null;
        }
      }
    }
    return all;
  };

  const renderCollFolders = (collId: string, parent: string | null, relevant: Set<string>, depth: number) => {
    const nodes = (childrenOf.get(parent) ?? []).filter((f) => relevant.has(f.id));
    if (nodes.length === 0 && depth === 0) {
      return <div className="px-6 py-1 text-xs text-neutral-600">sem pastas</div>;
    }
    return nodes.map((f) => {
      const isActive =
        filter.kind === "collection" && filter.value === collId && filter.folder === f.id;
      return (
        <div key={f.id}>
          <button
            onClick={() => onFilter({ kind: "collection", value: collId, folder: f.id })}
            className={`block w-full truncate rounded-lg py-1 text-left text-xs ${
              isActive ? "bg-surface-3 text-white" : "text-neutral-400 hover:bg-surface-2"
            }`}
            style={{ paddingLeft: 30 + depth * 12 }}
          >
            {f.name}
          </button>
          {renderCollFolders(collId, f.id, relevant, depth + 1)}
        </div>
      );
    });
  };

  async function commitAdd() {
    const name = newName.trim();
    const kind = adding;
    setAdding(null);
    setNewName("");
    setAddError(null);
    if (!name || !kind) return;
    try {
      if (kind === "folder") await saveFolder(null, name, null);
      else await createCollection(name);
    } catch (e) {
      setAddError(
        `Falha ao criar ${kind === "folder" ? "pasta" : "collection"}: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
  }

  const nameInput = (kind: "folder" | "collection") =>
    adding === kind ? (
      <input
        autoFocus
        className="field mx-2 my-1"
        placeholder={kind === "folder" ? "Nome da pasta — Enter" : "Nome da collection — Enter"}
        value={newName}
        onChange={(e) => setNewName(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") commitAdd();
          if (e.key === "Escape") {
            setAdding(null);
            setNewName("");
          }
        }}
        onBlur={commitAdd}
      />
    ) : null;

  return (
    <aside className="flex min-h-0 flex-col border-r border-line bg-surface-1">
      <div className="flex items-center justify-between px-4 py-3">
        <div className="flex items-center gap-2">
          <div className="grid h-7 w-7 place-items-center rounded-md bg-accent text-sm font-bold text-white">
            E
          </div>
          <span className="font-semibold text-white">EVEPass</span>
        </div>
        <button className="btn-ghost px-2 py-1.5" onClick={onLock} title="Travar">
          <Lock size={15} />
        </button>
      </div>

      <nav className="min-h-0 flex-1 overflow-y-auto px-2 pb-4 text-sm">
        {addError && (
          <div
            className="mx-2 mb-2 cursor-pointer rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-2 text-xs text-red-300"
            onClick={() => setAddError(null)}
            title="Clique para dispensar"
          >
            {addError}
          </div>
        )}
        <button
          className={`flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left ${
            active({ kind: "all" }) ? "bg-surface-3 text-white" : "hover:bg-surface-2"
          }`}
          onClick={() => onFilter({ kind: "all" })}
        >
          Todos os itens
          <span className="ml-auto text-xs text-neutral-500">{items.length}</span>
        </button>

        <Section title="Visões inteligentes">
          {SMART_VIEWS.map((v) => (
            <button
              key={v.key}
              className={`flex w-full items-center rounded-lg px-3 py-1.5 text-left ${
                active({ kind: "smart", value: v.key }) ? "bg-surface-3 text-white" : "hover:bg-surface-2"
              }`}
              onClick={() => {
                if (v.key === "breached") void refreshBreached();
                onFilter({ kind: "smart", value: v.key });
              }}
            >
              {v.label}
              <span className="ml-auto text-xs text-neutral-500">{smartCount(v.key)}</span>
            </button>
          ))}
        </Section>

        <Section
          title="Pastas"
          action={
            <button
              className="text-xs text-neutral-500 hover:text-accent-soft"
              onClick={() => {
                setNewName("");
                setAdding("folder");
              }}
            >
              +
            </button>
          }
        >
          {nameInput("folder")}
          <FolderTree
            parent={null}
            childrenOf={childrenOf}
            filter={filter}
            onFilter={onFilter}
            onDelete={(id) => void deleteFolder(id)}
          />
        </Section>

        <Section
          title="Collections"
          action={
            <button
              className="text-xs text-neutral-500 hover:text-accent-soft"
              onClick={() => {
                setNewName("");
                setAdding("collection");
              }}
            >
              +
            </button>
          }
        >
          {nameInput("collection")}
          {collections.length === 0 && (
            <div className="px-3 py-1.5 text-xs text-neutral-600">nenhuma collection</div>
          )}
          {collections.map((c) => {
            const expanded = expandedColls[c.id];
            const collActive =
              filter.kind === "collection" && filter.value === c.id && !filter.folder;
            return (
              <div key={c.id}>
                <div
                  className={`group flex items-center gap-1 rounded-lg px-2 py-1.5 ${
                    collActive ? "bg-surface-3 text-white" : "hover:bg-surface-2"
                  }`}
                >
                  <button
                    className="w-4 shrink-0 text-xs text-neutral-500"
                    onClick={() => setExpandedColls((s) => ({ ...s, [c.id]: !s[c.id] }))}
                  >
                    {expanded ? "▾" : "▸"}
                  </button>
                  <Users size={13} className="shrink-0 text-neutral-500" />
                  <button
                    className="flex-1 truncate text-left"
                    onClick={() => onFilter({ kind: "collection", value: c.id })}
                  >
                    {c.name}
                  </button>
                  <button
                    className="hidden text-xs text-neutral-500 hover:text-accent-soft group-hover:block"
                    title="Compartilhar / membros"
                    onClick={() => onShareCollection(c.id)}
                  >
                    ⋯
                  </button>
                </div>
                {expanded && renderCollFolders(c.id, null, collectionFolderIds(c.id), 0)}
              </div>
            );
          })}
        </Section>

        <Section title="Tags">
          {tags.length === 0 && <div className="px-3 py-1.5 text-xs text-neutral-600">nenhuma tag</div>}
          {tags.map((t) => (
            <button
              key={t}
              className={`block w-full rounded-lg px-3 py-1.5 text-left ${
                active({ kind: "tag", value: t }) ? "bg-surface-3 text-white" : "hover:bg-surface-2"
              }`}
              onClick={() => onFilter({ kind: "tag", value: t })}
            >
              #{t}
            </button>
          ))}
        </Section>
      </nav>

      <div className="flex items-center gap-1 border-t border-line px-2 py-2 text-xs">
        <button className="btn-ghost flex-1 gap-1.5 py-1.5" onClick={onOpenImport}>
          <Download size={14} /> Importar
        </button>
        <button className="btn-ghost flex-1 gap-1.5 py-1.5" onClick={onOpenSettings}>
          <Settings size={14} /> Configurações
        </button>
      </div>
    </aside>
  );
}

function Section({
  title,
  action,
  children,
}: {
  title: string;
  action?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="mt-4">
      <div className="flex items-center justify-between px-3 pb-1">
        <span className="text-xs font-medium uppercase tracking-wide text-neutral-500">{title}</span>
        {action}
      </div>
      {children}
    </div>
  );
}

function FolderTree({
  parent,
  childrenOf,
  filter,
  onFilter,
  onDelete,
  depth = 0,
}: {
  parent: string | null;
  childrenOf: Map<string | null, FolderView[]>;
  filter: Filter;
  onFilter: (f: Filter) => void;
  onDelete: (id: string) => void;
  depth?: number;
}) {
  const nodes = childrenOf.get(parent) ?? [];
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  return (
    <>
      {nodes.map((f) => {
        const kids = childrenOf.get(f.id) ?? [];
        const isActive = filter.kind === "folder" && filter.value === f.id;
        return (
          <div key={f.id}>
            <div
              className={`group flex items-center gap-1 rounded-lg px-2 py-1.5 ${
                isActive ? "bg-surface-3 text-white" : "hover:bg-surface-2"
              }`}
              style={{ paddingLeft: 8 + depth * 12 }}
            >
              {kids.length > 0 ? (
                <button
                  className="w-4 text-xs text-neutral-500"
                  onClick={() => setCollapsed((c) => ({ ...c, [f.id]: !c[f.id] }))}
                >
                  {collapsed[f.id] ? "▸" : "▾"}
                </button>
              ) : (
                <span className="w-4" />
              )}
              <button className="flex-1 text-left" onClick={() => onFilter({ kind: "folder", value: f.id })}>
                {f.name}
              </button>
              <button
                className="hidden text-xs text-neutral-600 hover:text-red-400 group-hover:block"
                onClick={() => {
                  if (window.confirm(`Excluir a pasta "${f.name}"?`)) onDelete(f.id);
                }}
              >
                ×
              </button>
            </div>
            {!collapsed[f.id] && (
              <FolderTree
                parent={f.id}
                childrenOf={childrenOf}
                filter={filter}
                onFilter={onFilter}
                onDelete={onDelete}
                depth={depth + 1}
              />
            )}
          </div>
        );
      })}
    </>
  );
}
