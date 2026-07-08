import { useEffect, useState } from "react";
import { useVault } from "../state/vault";
import { PasswordGenerator } from "./PasswordGenerator";

interface CustomField {
  name: string;
  value: string;
  hidden?: boolean;
}

export function ItemForm({ editingId, onClose }: { editingId: string | null; onClose: () => void }) {
  const { getItem, saveItem, folders, select, collections, items } = useVault();
  const [collectionId, setCollectionId] = useState<string | null>(
    editingId ? (items.find((i) => i.id === editingId)?.collection_id ?? null) : null,
  );

  const [type, setType] = useState("login");
  const [title, setTitle] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [url, setUrl] = useState("");
  const [totp, setTotp] = useState("");
  const [notes, setNotes] = useState("");
  const [tags, setTags] = useState("");
  const [selFolders, setSelFolders] = useState<string[]>([]);
  const [custom, setCustom] = useState<CustomField[]>([]);
  const [showGen, setShowGen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!editingId) return;
    getItem(editingId).then((json) => {
      const v = JSON.parse(json);
      setType(v.type ?? "login");
      setTitle(v.title ?? "");
      setUsername(v.username ?? "");
      setPassword(v.password ?? "");
      setUrl(v.url ?? "");
      setTotp(v.totp ?? "");
      setNotes(v.notes ?? "");
      setTags((v.tags ?? []).join(", "));
      setSelFolders(v.folders ?? []);
      setCustom(v.custom_fields ?? []);
    });
  }, [editingId, getItem]);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    if (!title.trim()) {
      setError("título é obrigatório");
      return;
    }
    setBusy(true);
    const item = {
      type,
      title: title.trim(),
      username,
      password,
      url,
      totp,
      notes,
      folders: selFolders,
      tags: tags
        .split(",")
        .map((t) => t.trim())
        .filter(Boolean),
      custom_fields: custom,
    };
    try {
      const id = await saveItem(editingId, JSON.stringify(item), collectionId);
      select(id);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setBusy(false);
    }
  }

  return (
    <div className="fixed inset-0 z-20 grid place-items-center bg-black/50 p-6" onClick={onClose}>
      <form
        onClick={(e) => e.stopPropagation()}
        onSubmit={submit}
        className="card max-h-[90vh] w-full max-w-lg overflow-y-auto p-5"
      >
        <h2 className="mb-4 text-lg font-semibold text-white">
          {editingId ? "Editar item" : "Novo item"}
        </h2>

        <div className="space-y-3">
          <Row label="Título">
            <input className="field" value={title} onChange={(e) => setTitle(e.target.value)} autoFocus />
          </Row>
          <Row label="Usuário">
            <input className="field" value={username} onChange={(e) => setUsername(e.target.value)} />
          </Row>
          <Row label="Senha">
            <div className="flex gap-2">
              <input
                className="field flex-1 font-mono"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
              <button type="button" className="btn-ghost px-2" onClick={() => setShowGen((s) => !s)}>
                Gerar
              </button>
            </div>
          </Row>
          {showGen && (
            <PasswordGenerator
              onPick={(pw) => {
                setPassword(pw);
                setShowGen(false);
              }}
            />
          )}
          <Row label="URL">
            <input className="field" value={url} onChange={(e) => setUrl(e.target.value)} />
          </Row>
          <Row label="TOTP (otpauth:// ou segredo)">
            <input className="field font-mono" value={totp} onChange={(e) => setTotp(e.target.value)} />
          </Row>
          <Row label="Tags (separadas por vírgula)">
            <input className="field" value={tags} onChange={(e) => setTags(e.target.value)} />
          </Row>
          {collections.length > 0 && (
            <Row label="Collection (compartilhada)">
              <select
                className="field"
                value={collectionId ?? ""}
                onChange={(e) => setCollectionId(e.target.value || null)}
              >
                <option value="">Pessoal (só você)</option>
                {collections.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name}
                  </option>
                ))}
              </select>
            </Row>
          )}
          {folders.length > 0 && (
            <Row label="Pastas">
              <div className="flex flex-wrap gap-2">
                {folders.map((f) => {
                  const on = selFolders.includes(f.id);
                  return (
                    <button
                      type="button"
                      key={f.id}
                      onClick={() =>
                        setSelFolders((s) => (on ? s.filter((x) => x !== f.id) : [...s, f.id]))
                      }
                      className={`rounded-full px-2.5 py-1 text-xs ${
                        on ? "bg-accent text-white" : "bg-surface-2 text-neutral-300"
                      }`}
                    >
                      {f.name}
                    </button>
                  );
                })}
              </div>
            </Row>
          )}
          <Row label="Notas">
            <textarea
              className="field min-h-[80px]"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
            />
          </Row>
        </div>

        {error && <p className="mt-3 text-sm text-red-400">{error}</p>}

        <div className="mt-5 flex justify-end gap-2">
          <button type="button" className="btn-ghost" onClick={onClose}>
            Cancelar
          </button>
          <button className="btn-primary disabled:opacity-40" disabled={busy}>
            {busy ? "…" : "Salvar"}
          </button>
        </div>
      </form>
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
