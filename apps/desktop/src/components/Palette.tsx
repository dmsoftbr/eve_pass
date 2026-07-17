// Command palette — runs in its own frameless window, toggled by the global
// hotkey. Shares the Rust vault state (it queries palette_search / copy_field
// directly); it has no React vault context of its own. Copy happens in Rust.
//
// When the vault is locked it renders an inline unlock field instead of the
// search box: the master password is forwarded to the main window (which owns
// the network + remembered e-mail) via a `palette-unlock` event, and the main
// window runs the normal login. This lets the user unlock straight from the
// hotkey without switching to the main window first.
import { useEffect, useRef, useState } from "react";
import { getCurrentWindow, Window } from "@tauri-apps/api/window";
import { emit, listen } from "@tauri-apps/api/event";
import { ipc, type PaletteHit } from "../lib/ipc";

export function Palette() {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<PaletteHit[]>([]);
  const [sel, setSel] = useState(0);
  const [locked, setLocked] = useState(false);
  const [reload, setReload] = useState(0);
  // Inline-unlock state (only used while locked).
  const [pw, setPw] = useState("");
  const [unlocking, setUnlocking] = useState(false);
  const [unlockError, setUnlockError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const self = getCurrentWindow();

  // Refocus the input whenever the window is shown.
  useEffect(() => {
    const un = self.onFocusChanged(({ payload }) => {
      if (payload) {
        setQuery("");
        setSel(0);
        setPw(""); // don't let the master password linger between opens
        setUnlockError(null);
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
      // Locked (or errored): clear any previously loaded hits so decrypted
      // titles/usernames never linger on screen after the vault locks.
      .catch(() => alive && (setHits([]), setLocked(true)));
    return () => {
      alive = false;
    };
  }, [query, reload]);

  // The main window tells us the outcome of an inline unlock, and the Rust
  // auto-lock timer broadcasts `vault-locked`. Both must clear decrypted state.
  useEffect(() => {
    const subs = [
      listen("vault-unlocked", () => {
        setUnlocking(false);
        setUnlockError(null);
        setPw("");
        setLocked(false);
        setReload((n) => n + 1); // re-run the (now authorized) search
      }),
      listen<{ message: string }>("palette-unlock-error", (e) => {
        setUnlocking(false);
        setUnlockError(e.payload.message);
      }),
      listen("vault-locked", () => {
        setLocked(true);
        setHits([]);
        setQuery("");
      }),
    ];
    return () => {
      for (const s of subs) void s.then((f) => f());
    };
  }, []);

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

  function submitUnlock() {
    if (!pw || unlocking) return;
    setUnlocking(true);
    setUnlockError(null);
    void emit("palette-unlock", { password: pw });
  }

  async function onKey(e: React.KeyboardEvent) {
    if (e.key === "Escape") return void hide();

    if (locked) {
      if (e.key === "Enter") {
        e.preventDefault();
        submitUnlock();
      }
      return;
    }

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
        type={locked ? "password" : "text"}
        value={locked ? pw : query}
        onChange={(e) => (locked ? setPw(e.target.value) : setQuery(e.target.value))}
        onKeyDown={onKey}
        disabled={unlocking}
        placeholder={
          locked
            ? unlocking
              ? "Destravando…"
              : "Cofre travado — digite a senha-mestra e ↵ para destravar"
            : "Buscar no cofre…"
        }
        className="w-full bg-transparent px-4 py-3 text-base outline-none placeholder:text-neutral-600"
      />
      <div className="min-h-0 flex-1 overflow-y-auto border-t border-line">
        {locked ? (
          <div className="px-4 py-6 text-center text-sm">
            {unlockError ? (
              <span className="text-red-400">{unlockError}</span>
            ) : (
              <span className="text-neutral-600">
                a senha-mestra destrava o cofre inteiro (mesma sessão do app)
              </span>
            )}
          </div>
        ) : (
          <>
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
            {hits.length === 0 && (
              <div className="px-4 py-6 text-center text-sm text-neutral-600">nada encontrado</div>
            )}
          </>
        )}
      </div>
      <div className="flex items-center gap-3 border-t border-line px-4 py-1.5 text-[11px] text-neutral-600">
        {locked ? (
          <span>↵ destravar</span>
        ) : (
          <>
            <span>↵ copiar senha</span>
            <span>⌘↵ abrir</span>
            <span>⌘U usuário</span>
            <span>⌘T TOTP</span>
          </>
        )}
        <span className="ml-auto">esc fechar</span>
      </div>
    </div>
  );
}
