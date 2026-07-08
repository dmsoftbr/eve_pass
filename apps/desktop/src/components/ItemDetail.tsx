import { useEffect, useState } from "react";
import { Copy, Eye, EyeOff } from "lucide-react";
import { useVault } from "../state/vault";
import { ipc, type TotpLive } from "../lib/ipc";

interface FullItem {
  type: string;
  title: string;
  username?: string;
  password?: string;
  url?: string;
  totp?: string;
  notes?: string;
  tags?: string[];
  custom_fields?: { name: string; value: string; hidden?: boolean }[];
}

export function ItemDetail({ onEdit }: { onEdit: (id: string) => void }) {
  const { selectedId, getItem, copyField, deleteItem } = useVault();
  const [item, setItem] = useState<FullItem | null>(null);
  const [showPw, setShowPw] = useState(false);

  useEffect(() => {
    setShowPw(false);
    if (!selectedId) {
      setItem(null);
      return;
    }
    let alive = true;
    getItem(selectedId)
      .then((json) => alive && setItem(JSON.parse(json)))
      .catch(() => alive && setItem(null));
    return () => {
      alive = false;
    };
  }, [selectedId, getItem]);

  if (!selectedId || !item) {
    return <div className="grid place-items-center text-sm text-neutral-600">selecione um item</div>;
  }

  return (
    <div className="min-h-0 overflow-y-auto p-6">
      <div className="mb-6 flex items-start justify-between">
        <div>
          <h2 className="text-xl font-semibold text-white">{item.title}</h2>
          <span className="text-xs uppercase tracking-wide text-neutral-500">{item.type}</span>
        </div>
        <div className="flex gap-2">
          <button className="btn-ghost" onClick={() => onEdit(selectedId)}>
            Editar
          </button>
          <button
            className="btn-ghost text-red-400"
            onClick={() => {
              if (window.confirm("Excluir este item?")) void deleteItem(selectedId);
            }}
          >
            Excluir
          </button>
        </div>
      </div>

      <div className="space-y-3">
        <Field label="Usuário" value={item.username} onCopy={() => copyField(selectedId, "username")} />
        <Field
          label="Senha"
          value={item.password}
          mono
          masked={!showPw}
          onToggle={() => setShowPw((s) => !s)}
          onCopy={() => copyField(selectedId, "password")}
        />
        <Field label="URL" value={item.url} onCopy={() => copyField(selectedId, "url")} />
        {item.totp && <LiveTotp id={selectedId} onCopy={() => copyField(selectedId, "totp")} />}
        {item.custom_fields?.map((f, i) => (
          <Field key={i} label={f.name} value={f.value} masked={f.hidden} />
        ))}
        {item.notes && (
          <div className="card p-3">
            <div className="mb-1 text-xs text-neutral-500">Notas</div>
            <p className="whitespace-pre-wrap text-sm text-neutral-200">{item.notes}</p>
          </div>
        )}
        {item.tags && item.tags.length > 0 && (
          <div className="flex flex-wrap gap-1 pt-2">
            {item.tags.map((t) => (
              <span key={t} className="rounded-full bg-surface-2 px-2 py-0.5 text-xs text-neutral-300">
                #{t}
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function LiveTotp({ id, onCopy }: { id: string; onCopy: () => void }) {
  const [totp, setTotp] = useState<TotpLive | null>(null);
  const [err, setErr] = useState(false);

  useEffect(() => {
    let alive = true;
    const tick = () =>
      ipc
        .itemTotp(id)
        .then((t) => alive && (setTotp(t), setErr(false)))
        .catch(() => alive && setErr(true));
    tick();
    const h = setInterval(tick, 1000);
    return () => {
      alive = false;
      clearInterval(h);
    };
  }, [id]);

  if (err) return <Field label="TOTP" value="segredo inválido" />;
  if (!totp) return null;

  const pct = (totp.seconds_remaining / 30) * 100;
  return (
    <div className="card flex items-center gap-3 p-3">
      <div className="min-w-0 flex-1">
        <div className="text-xs text-neutral-500">TOTP</div>
        <div className="font-mono text-lg tracking-[0.3em] text-accent-soft">
          {totp.code.replace(/(\d{3})(\d{3})/, "$1 $2")}
        </div>
      </div>
      <div className="relative h-8 w-8">
        <svg viewBox="0 0 36 36" className="h-8 w-8 -rotate-90">
          <circle cx="18" cy="18" r="16" fill="none" stroke="#2a2a35" strokeWidth="3" />
          <circle
            cx="18"
            cy="18"
            r="16"
            fill="none"
            stroke="#6d5efc"
            strokeWidth="3"
            strokeDasharray={100.5}
            strokeDashoffset={100.5 - (pct / 100) * 100.5}
          />
        </svg>
        <span className="absolute inset-0 grid place-items-center text-[10px] text-neutral-400">
          {totp.seconds_remaining}
        </span>
      </div>
      <button className="btn-ghost gap-1 px-2 py-1 text-xs" onClick={onCopy} title="Copiar código">
        <Copy size={14} /> Copiar
      </button>
    </div>
  );
}

function Field({
  label,
  value,
  mono,
  masked,
  onToggle,
  onCopy,
}: {
  label: string;
  value?: string;
  mono?: boolean;
  masked?: boolean;
  onToggle?: () => void;
  onCopy?: () => void;
}) {
  if (!value) return null;
  return (
    <div className="card flex items-center gap-3 p-3">
      <div className="min-w-0 flex-1">
        <div className="text-xs text-neutral-500">{label}</div>
        <div className={`truncate text-sm text-neutral-100 ${mono ? "font-mono" : ""}`}>
          {masked ? "•".repeat(Math.min(value.length, 16)) : value}
        </div>
      </div>
      {onToggle && (
        <button className="btn-ghost px-2 py-1" onClick={onToggle} title={masked ? "Mostrar" : "Ocultar"}>
          {masked ? <Eye size={15} /> : <EyeOff size={15} />}
        </button>
      )}
      {onCopy && (
        <button className="btn-ghost gap-1 px-2 py-1 text-xs" onClick={onCopy} title="Copiar">
          <Copy size={14} /> Copiar
        </button>
      )}
    </div>
  );
}
