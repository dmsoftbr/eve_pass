// Browser-extension pairing approval (Fase 5A). The native-messaging host in the
// Rust backend emits `host-pair-request` with the extension's origin the first
// time an unpaired extension connects. We ask the user to approve it; approval
// is persisted (in settings) so it's a one-time prompt per extension.
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ipc } from "../lib/ipc";

export function HostPairModal() {
  const [origin, setOrigin] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const un = listen<string>("host-pair-request", (e) => {
      // Ignore repeats for an origin already being decided.
      setOrigin((cur) => cur ?? e.payload);
    });
    return () => {
      void un.then((f) => f());
    };
  }, []);

  if (!origin) return null;

  async function decide(approved: boolean) {
    if (!origin) return;
    setBusy(true);
    try {
      await ipc.setHostPairing(origin, approved);
    } finally {
      setBusy(false);
      setOrigin(null);
    }
  }

  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-black/50 p-8">
      <div className="card w-full max-w-md p-6">
        <h2 className="text-lg font-semibold text-white">Parear extensão do navegador</h2>
        <p className="mt-2 text-sm text-neutral-400">
          Uma extensão do navegador quer usar o EVEPass para preencher credenciais. Aprove apenas se
          você acabou de instalar a extensão oficial do EVEPass.
        </p>
        <pre className="mt-3 select-text overflow-x-auto rounded-lg border border-line bg-surface-0 p-3 text-xs text-neutral-300">
          {origin}
        </pre>
        <p className="mt-3 rounded-lg border border-yellow-600/40 bg-yellow-600/10 p-3 text-xs text-yellow-300">
          A extensão nunca recebe suas chaves. Uma senha só cruza para o navegador no momento do
          preenchimento, e apenas com o cofre destravado.
        </p>
        <div className="mt-5 flex gap-3">
          <button
            className="btn-primary flex-1 disabled:opacity-40"
            disabled={busy}
            onClick={() => void decide(true)}
          >
            Aprovar
          </button>
          <button
            className="btn flex-1 disabled:opacity-40"
            disabled={busy}
            onClick={() => void decide(false)}
          >
            Recusar
          </button>
        </div>
      </div>
    </div>
  );
}
