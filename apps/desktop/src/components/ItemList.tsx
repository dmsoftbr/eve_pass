import { KeyRound, User, Users } from "lucide-react";
import { useVault } from "../state/vault";
import type { ItemView } from "../lib/ipc";

function monogramColor(seed: string): string {
  let h = 0;
  for (const c of seed) h = (h * 31 + c.charCodeAt(0)) % 360;
  return `hsl(${h} 45% 40%)`;
}

export function ItemList({ items, onEdit }: { items: ItemView[]; onEdit: (id: string) => void }) {
  const { selectedId, select, copyField } = useVault();

  if (items.length === 0) {
    return <div className="grid flex-1 place-items-center text-sm text-neutral-600">nenhum item</div>;
  }

  return (
    <ul className="min-h-0 flex-1 overflow-y-auto">
      {items.map((it) => (
        <li
          key={it.id}
          onClick={() => select(it.id)}
          onDoubleClick={() => onEdit(it.id)}
          className={`group flex cursor-default items-center gap-3 border-b border-line/50 px-3 py-2.5 ${
            selectedId === it.id ? "bg-surface-2" : "hover:bg-surface-1"
          }`}
        >
          <div
            className="grid h-8 w-8 shrink-0 place-items-center rounded-md text-xs font-semibold text-white"
            style={{ background: monogramColor(it.title || "?") }}
          >
            {(it.title || "?").slice(0, 1).toUpperCase()}
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-1.5">
              <span className="truncate text-sm text-neutral-100">{it.title || "(sem título)"}</span>
              {it.collection_id && (
                <span title="Compartilhado" className="shrink-0 text-neutral-500">
                  <Users size={13} />
                </span>
              )}
            </div>
            <div className="truncate text-xs text-neutral-500">{it.username || it.url || "—"}</div>
          </div>
          {it.username && (
            <button
              className="hidden rounded p-1.5 text-neutral-400 hover:bg-surface-3 hover:text-neutral-200 group-hover:block"
              title="Copiar usuário"
              onClick={(e) => {
                e.stopPropagation();
                void copyField(it.id, "username");
              }}
            >
              <User size={15} />
            </button>
          )}
          <button
            className="hidden rounded p-1.5 text-neutral-400 hover:bg-surface-3 hover:text-neutral-200 group-hover:block"
            title="Copiar senha"
            onClick={(e) => {
              e.stopPropagation();
              void copyField(it.id, "password");
            }}
          >
            <KeyRound size={15} />
          </button>
        </li>
      ))}
    </ul>
  );
}
