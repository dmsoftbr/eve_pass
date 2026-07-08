// Command palette — runs in its own frameless window, toggled by the global
// hotkey. Shares the Rust vault state (it queries palette_search / copy_field
// directly); it has no React vault context of its own. Copy happens in Rust.
import { useEffect, useRef, useState } from "react";
import { getCurrentWindow, Window } from "@tauri-apps/api/window";
import { emit } from "@tauri-apps/api/event";
import { ipc, type PaletteHit } from "../lib/ipc";

export function Palette() {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<PaletteHit[]>([]);
  const [sel, setSel] = useState(0);
  const [locked, setLocked] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const self = getCurrentWindow();

  // Refocus the input whenever the window is shown.
  useEffect(() => {
    const un = self.onFocusChanged(({ payload }) => {
      if (payload) {
        setQuery("");
        setSel(0);
        inputRef.current?.focus();
      }
    });
    return () => {
      void un.then((f) => f());
    };
  }, [self]);

  useEffect(() => {
    let alive = true;
    ipc
      .paletteSearch(query)
      .then((r) => alive && (setHits(r), setLocked(false), setSel(0)))
      .catch(() => alive && setLocked(true));
    return () => {
      alive = false;
    };
  }, [query]);

  async function hide() {
    await self.hide();
  }

  async function openInMain(id: string) {
    const main = await Window.getByLabel("main");
    await main?.show();
    await main?.setFocus();
    await emit("palette-open-item", id);
    await hide();
  }

  async function onKey(e: React.KeyboardEvent) {
    if (e.key === "Escape") return void hide();
    if (e.key === "ArrowDown") {
      e.preventDefault();
      return setSel((s) => Math.min(s + 1, hits.length - 1));
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      return setSel((s) => Math.max(s - 1, 0));
    }
    const hit = hits[sel];
    if (!hit) return;
    if (e.key === "Enter") {
      e.preventDefault();
      if (e.metaKey || e.ctrlKey) return void openInMain(hit.id);
      await ipc.copyField(hit.id, "password");
      return void hide();
    }
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "u") {
      e.preventDefault();
      await ipc.copyField(hit.id, "username");
      return void hide();
    }
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "t") {
      e.preventDefault();
      await ipc.copyField(hit.id, "totp");
      return void hide();
    }
  }

  return (
    <div className="flex h-screen flex-col overflow-hidden rounded-xl border border-line bg-surface-1/95 backdrop-blur">
      <input
        ref={inputRef}
        autoFocus
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={onKey}
        placeholder={locked ? "Cofre travado — destrave na janela principal" : "Buscar no cofre…"}
        className="w-full bg-transparent px-4 py-3 text-base outline-none placeholder:text-neutral-600"
      />
      <div className="min-h-0 flex-1 overflow-y-auto border-t border-line">
        {hits.map((h, i) => (
          <div
            key={h.id}
            onMouseEnter={() => setSel(i)}
            onClick={() => {
              void ipc.copyField(h.id, "password").then(hide);
            }}
            className={`flex items-center gap-3 px-4 py-2.5 ${i === sel ? "bg-accent/20" : ""}`}
          >
            <div className="min-w-0 flex-1">
              <div className="truncate text-sm text-neutral-100">{h.title}</div>
              <div className="truncate text-xs text-neutral-500">{h.username}</div>
            </div>
            {h.has_totp && <span className="text-xs text-neutral-500">TOTP</span>}
          </div>
        ))}
        {!locked && hits.length === 0 && (
          <div className="px-4 py-6 text-center text-sm text-neutral-600">nada encontrado</div>
        )}
      </div>
      <div className="flex items-center gap-3 border-t border-line px-4 py-1.5 text-[11px] text-neutral-600">
        <span>↵ copiar senha</span>
        <span>⌘↵ abrir</span>
        <span>⌘U usuário</span>
        <span>⌘T TOTP</span>
        <span className="ml-auto">esc fechar</span>
      </div>
    </div>
  );
}
