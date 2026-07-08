import { useEffect, useState } from "react";
import { ipc } from "../lib/ipc";

export function PasswordGenerator({ onPick }: { onPick: (pw: string) => void }) {
  const [length, setLength] = useState(20);
  const [upper, setUpper] = useState(true);
  const [lower, setLower] = useState(true);
  const [digits, setDigits] = useState(true);
  const [symbols, setSymbols] = useState(true);
  const [value, setValue] = useState("");

  async function regen() {
    try {
      setValue(await ipc.genPassword(length, upper, lower, digits, symbols));
    } catch {
      /* no class selected */
    }
  }

  useEffect(() => {
    void regen();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [length, upper, lower, digits, symbols]);

  return (
    <div className="card space-y-3 p-3">
      <div className="flex items-center gap-2">
        <input readOnly value={value} className="field flex-1 font-mono text-sm" />
        <button type="button" className="btn-ghost px-2" onClick={() => void regen()} title="Gerar outra">
          ↻
        </button>
      </div>
      <div className="flex items-center gap-2 text-xs text-neutral-400">
        <span>Comprimento</span>
        <input
          type="range"
          min={8}
          max={64}
          value={length}
          onChange={(e) => setLength(Number(e.target.value))}
          className="flex-1"
        />
        <span className="w-6 text-right font-mono">{length}</span>
      </div>
      <div className="flex flex-wrap gap-3 text-xs text-neutral-300">
        <Toggle label="A-Z" on={upper} set={setUpper} />
        <Toggle label="a-z" on={lower} set={setLower} />
        <Toggle label="0-9" on={digits} set={setDigits} />
        <Toggle label="!@#" on={symbols} set={setSymbols} />
      </div>
      <button type="button" className="btn-primary w-full" onClick={() => onPick(value)}>
        Usar esta senha
      </button>
    </div>
  );
}

function Toggle({ label, on, set }: { label: string; on: boolean; set: (v: boolean) => void }) {
  return (
    <label className="flex items-center gap-1">
      <input type="checkbox" checked={on} onChange={(e) => set(e.target.checked)} />
      {label}
    </label>
  );
}
