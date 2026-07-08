import { useState } from "react";
import { AlertTriangle, Check } from "lucide-react";
import { useVault } from "../state/vault";
import {
  guessMapping,
  parseBitwarden,
  parseCsv,
  parseCsvWithMapping,
  type CsvMapping,
  type ParsedImport,
} from "../lib/import";

type Stage = "pick" | "map" | "done";
const FIELDS: (keyof CsvMapping)[] = ["title", "username", "password", "url", "notes", "folder"];
const FIELD_LABEL: Record<keyof CsvMapping, string> = {
  title: "Título",
  username: "Usuário",
  password: "Senha",
  url: "URL",
  notes: "Notas",
  folder: "Pasta",
};

export function ImportModal({ onClose }: { onClose: () => void }) {
  const { importFolders, importItems } = useVault();
  const [stage, setStage] = useState<Stage>("pick");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [count, setCount] = useState(0);

  // CSV mapping state
  const [headers, setHeaders] = useState<string[]>([]);
  const [rows, setRows] = useState<string[][]>([]);
  const [mapping, setMapping] = useState<CsvMapping | null>(null);

  async function onFile(e: React.ChangeEvent<HTMLInputElement>) {
    setError(null);
    const file = e.target.files?.[0];
    if (!file) return;
    const text = await file.text();
    try {
      if (file.name.endsWith(".json") || text.trimStart().startsWith("{")) {
        const p = parseBitwarden(text);
        setCount(p.entries.length);
        await runImport(p);
      } else {
        const r = parseCsv(text);
        if (r.length < 2) throw new Error("CSV vazio");
        setRows(r);
        setHeaders(r[0]);
        setMapping(guessMapping(r[0]));
        setStage("map");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function runImport(p: ParsedImport) {
    setBusy(true);
    try {
      const ids = await importFolders(p.folderNames);
      const byName = new Map(p.folderNames.map((n, i) => [n, ids[i]]));
      const items = p.entries.map((entry) => {
        const item = { ...entry.item };
        if (entry.folder && byName.has(entry.folder)) item.folders = [byName.get(entry.folder)!];
        return JSON.stringify(item);
      });
      await importItems(items);
      setCount(p.entries.length);
      setStage("done");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="fixed inset-0 z-20 grid place-items-center bg-black/50 p-6" onClick={onClose}>
      <div className="card w-full max-w-lg p-5" onClick={(e) => e.stopPropagation()}>
        <h2 className="mb-1 text-lg font-semibold text-white">Importar</h2>

        {stage === "pick" && (
          <>
            <p className="mb-4 text-sm text-neutral-400">
              Bitwarden (JSON) ou NordPass/CSV genérico. O arquivo é lido localmente e cifrado no
              cofre.
            </p>
            <input type="file" accept=".json,.csv,text/csv,application/json" onChange={onFile} className="field" />
          </>
        )}

        {stage === "map" && mapping && (
          <>
            <p className="mb-4 text-sm text-neutral-400">
              Mapeie as colunas do CSV para os campos do EVEPass ({rows.length - 1} linhas).
            </p>
            <div className="space-y-2">
              {FIELDS.map((f) => (
                <div key={f} className="flex items-center gap-3">
                  <span className="w-20 text-xs text-neutral-400">{FIELD_LABEL[f]}</span>
                  <select
                    className="field flex-1"
                    value={mapping[f]}
                    onChange={(e) => setMapping({ ...mapping, [f]: Number(e.target.value) })}
                  >
                    <option value={-1}>(nenhum)</option>
                    {headers.map((h, i) => (
                      <option key={i} value={i}>
                        {h || `coluna ${i + 1}`}
                      </option>
                    ))}
                  </select>
                </div>
              ))}
            </div>
            <div className="mt-5 flex justify-end gap-2">
              <button className="btn-ghost" onClick={() => setStage("pick")}>
                Voltar
              </button>
              <button
                className="btn-primary disabled:opacity-40"
                disabled={busy}
                onClick={() => void runImport(parseCsvWithMapping(rows, mapping))}
              >
                {busy ? "importando…" : "Importar"}
              </button>
            </div>
          </>
        )}

        {stage === "done" && (
          <div className="text-sm text-neutral-300">
            <p className="mb-3 flex items-center gap-2">
              <Check size={16} className="shrink-0 text-emerald-400" />
              {count} itens importados e cifrados no cofre.
            </p>
            <p className="flex items-start gap-2 rounded-lg border border-yellow-600/40 bg-yellow-600/10 p-3 text-yellow-300">
              <AlertTriangle size={16} className="mt-0.5 shrink-0" />
              Apague o arquivo de origem — ele contém suas senhas em texto puro.
            </p>
            <div className="mt-5 flex justify-end">
              <button className="btn-primary" onClick={onClose}>
                Concluir
              </button>
            </div>
          </div>
        )}

        {error && <p className="mt-3 text-sm text-red-400">{error}</p>}

        {stage === "pick" && (
          <div className="mt-5 flex justify-end">
            <button className="btn-ghost" onClick={onClose}>
              Fechar
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
