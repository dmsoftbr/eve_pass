import { useState } from "react";
import { Check, Trash2 } from "lucide-react";
import { useVault } from "../state/vault";
import { ipc } from "../lib/ipc";
import { getPublicKeyByEmail, upsertCollectionMember } from "../lib/supabase";

type Role = "admin" | "writer" | "reader";

export function ShareModal({ collectionId, onClose }: { collectionId: string; onClose: () => void }) {
  const { ownSigningPubB64, deleteCollection } = useVault();
  const [email, setEmail] = useState("");
  const [role, setRole] = useState<Role>("reader");
  const [recipient, setRecipient] = useState<{ userId: string; pubB64: string; fingerprint: string } | null>(
    null,
  );
  const [confirmed, setConfirmed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [deleting, setDeleting] = useState(false);

  async function remove() {
    setDeleting(true);
    setError(null);
    try {
      await deleteCollection(collectionId);
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setDeleting(false);
      setConfirmDelete(false);
    }
  }

  async function lookup() {
    setError(null);
    setRecipient(null);
    setConfirmed(false);
    try {
      const pk = await getPublicKeyByEmail(email.trim());
      const fingerprint = await ipc.publicKeyFingerprint(pk.publicKeyB64);
      setRecipient({ userId: pk.userId, pubB64: pk.publicKeyB64, fingerprint });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function share() {
    if (!recipient || !ownSigningPubB64) return;
    setBusy(true);
    setError(null);
    try {
      // HPKE-wrap the collection key for the recipient (in Rust) + sign it.
      const wrapped = await ipc.wrapCollectionKeyFor(collectionId, recipient.pubB64);
      await upsertCollectionMember(collectionId, recipient.userId, wrapped, ownSigningPubB64, role);
      setDone(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="fixed inset-0 z-20 grid place-items-center bg-black/50 p-6" onClick={onClose}>
      <div className="card w-full max-w-md p-5" onClick={(e) => e.stopPropagation()}>
        <h2 className="mb-1 text-lg font-semibold text-white">Compartilhar collection</h2>
        <p className="mb-4 text-sm text-neutral-400">
          O destinatário precisa já ter uma conta EVEPass. Confira o fingerprint com ele por um canal
          externo antes de compartilhar (protege contra troca de chave pelo servidor).
        </p>

        {done ? (
          <div className="text-sm text-neutral-300">
            <div className="flex items-start gap-2">
              <Check size={16} className="mt-0.5 shrink-0 text-emerald-400" />
              <span>
                Compartilhado com <strong>{email}</strong> como <strong>{role}</strong>. Ele passa a
                decifrar após o próximo sync.
              </span>
            </div>
            <div className="mt-5 flex justify-end">
              <button className="btn-primary" onClick={onClose}>
                Concluir
              </button>
            </div>
          </div>
        ) : (
          <>
            <div className="flex gap-2">
              <input
                className="field flex-1"
                type="email"
                placeholder="email@do.colega"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
              />
              <button className="btn-ghost" onClick={() => void lookup()}>
                Buscar
              </button>
            </div>

            {recipient && (
              <div className="mt-4 space-y-3">
                <div className="card p-3">
                  <div className="text-xs text-neutral-500">Fingerprint da chave pública</div>
                  <div className="font-mono text-sm text-accent-soft">{recipient.fingerprint}</div>
                </div>
                <div>
                  <label className="mb-1 block text-xs text-neutral-400">Papel</label>
                  <select className="field" value={role} onChange={(e) => setRole(e.target.value as Role)}>
                    <option value="reader">Reader (só lê)</option>
                    <option value="writer">Writer (lê e escreve)</option>
                    <option value="admin">Admin (gere membros)</option>
                  </select>
                </div>
                <label className="flex items-center gap-2 text-sm text-neutral-300">
                  <input type="checkbox" checked={confirmed} onChange={(e) => setConfirmed(e.target.checked)} />
                  Confirmei o fingerprint com o destinatário.
                </label>
              </div>
            )}

            {error && <p className="mt-3 text-sm text-red-400">{error}</p>}

            <div className="mt-5 flex justify-end gap-2">
              <button className="btn-ghost" onClick={onClose}>
                Cancelar
              </button>
              <button
                className="btn-primary disabled:opacity-40"
                disabled={!recipient || !confirmed || busy}
                onClick={() => void share()}
              >
                {busy ? "…" : "Compartilhar"}
              </button>
            </div>

            <div className="mt-4 flex items-center justify-between border-t border-line pt-4">
              {confirmDelete ? (
                <>
                  <span className="text-sm text-red-300">Excluir e apagar seus itens?</span>
                  <div className="flex gap-2">
                    <button className="btn-ghost text-xs" onClick={() => setConfirmDelete(false)}>
                      Cancelar
                    </button>
                    <button
                      className="btn text-xs bg-red-600 text-white hover:bg-red-500 disabled:opacity-40"
                      disabled={deleting}
                      onClick={() => void remove()}
                    >
                      {deleting ? "…" : "Excluir"}
                    </button>
                  </div>
                </>
              ) : (
                <button
                  className="flex items-center gap-1.5 text-xs text-neutral-500 hover:text-red-400"
                  onClick={() => setConfirmDelete(true)}
                >
                  <Trash2 size={14} /> Excluir esta collection
                </button>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
