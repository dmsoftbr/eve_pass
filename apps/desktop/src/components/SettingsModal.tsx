import { useEffect, useState } from "react";
import { ipc, type Settings } from "../lib/ipc";

export function SettingsModal({ onClose }: { onClose: () => void }) {
  const [s, setS] = useState<Settings | null>(null);
  const [capturing, setCapturing] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    ipc.getSettings().then(setS);
  }, []);

  if (!s) return null;

  function set<K extends keyof Settings>(key: K, value: Settings[K]) {
    setS((prev) => (prev ? { ...prev, [key]: value } : prev));
  }

  function captureHotkey(e: React.KeyboardEvent) {
    e.preventDefault();
    if (["Control", "Meta", "Alt", "Shift"].includes(e.key)) return;
    const parts: string[] = [];
    if (e.ctrlKey || e.metaKey) parts.push("CmdOrCtrl");
    if (e.altKey) parts.push("Alt");
    if (e.shiftKey) parts.push("Shift");
    const key = e.key === " " ? "Space" : e.key.length === 1 ? e.key.toUpperCase() : e.key;
    parts.push(key);
    set("global_hotkey", parts.join("+"));
    setCapturing(false);
  }

  async function save() {
    if (!s) return;
    setBusy(true);
    try {
      await ipc.setSettings(s);
      onClose();
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="fixed inset-0 z-20 grid place-items-center bg-black/50 p-6" onClick={onClose}>
      <div className="card w-full max-w-md p-5" onClick={(e) => e.stopPropagation()}>
        <h2 className="mb-4 text-lg font-semibold text-white">Configurações</h2>

        <div className="space-y-4 text-sm">
          <Row label="Tema">
            <select
              className="field"
              value={s.theme}
              onChange={(e) => set("theme", e.target.value as Settings["theme"])}
            >
              <option value="system">Sistema</option>
              <option value="dark">Escuro</option>
              <option value="light">Claro</option>
            </select>
          </Row>

          <Row label="Auto-lock (minutos, 0 = nunca)">
            <input
              type="number"
              min={0}
              className="field"
              value={s.auto_lock_minutes}
              onChange={(e) => set("auto_lock_minutes", Math.max(0, Number(e.target.value)))}
            />
          </Row>

          <Row label="Limpar clipboard após (segundos, 0 = nunca)">
            <input
              type="number"
              min={0}
              className="field"
              value={s.clipboard_clear_seconds}
              onChange={(e) => set("clipboard_clear_seconds", Math.max(0, Number(e.target.value)))}
            />
          </Row>

          <Row label="Atalho global da palette">
            <input
              readOnly
              className="field cursor-pointer font-mono"
              value={capturing ? "pressione a combinação…" : s.global_hotkey}
              onFocus={() => setCapturing(true)}
              onBlur={() => setCapturing(false)}
              onKeyDown={captureHotkey}
            />
          </Row>

          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={s.launch_at_login}
              onChange={(e) => set("launch_at_login", e.target.checked)}
            />
            Iniciar no login
          </label>
        </div>

        <div className="mt-5 flex justify-end gap-2">
          <button className="btn-ghost" onClick={onClose}>
            Cancelar
          </button>
          <button className="btn-primary disabled:opacity-40" disabled={busy} onClick={save}>
            {busy ? "…" : "Salvar"}
          </button>
        </div>
      </div>
    </div>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <label className="mb-1 block text-xs text-neutral-400">{label}</label>
      {children}
    </div>
  );
}
