import { useState } from "react";
import { Eye, EyeOff } from "lucide-react";
import { useVault } from "../state/vault";
import { doRecover, LAST_EMAIL_KEY } from "../lib/auth";

type Mode = "login" | "signup" | "recover";

// Only the e-mail is remembered — never the master password.
const REMEMBER_KEY = "evepass:rememberEmail";

function strength(pw: string): { score: number; label: string } {
  let s = 0;
  if (pw.length >= 8) s++;
  if (pw.length >= 12) s++;
  if (/[a-z]/.test(pw) && /[A-Z]/.test(pw)) s++;
  if (/\d/.test(pw)) s++;
  if (/[^\w]/.test(pw)) s++;
  const label = ["muito fraca", "fraca", "ok", "boa", "forte", "excelente"][s] ?? "";
  return { score: s, label };
}

export function AuthScreen() {
  const { login, signup } = useVault();
  const [mode, setMode] = useState<Mode>("login");
  const [remember, setRemember] = useState(() => localStorage.getItem(REMEMBER_KEY) === "1");
  const [email, setEmail] = useState(() =>
    localStorage.getItem(REMEMBER_KEY) === "1" ? (localStorage.getItem(LAST_EMAIL_KEY) ?? "") : "",
  );
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [recoveryCodeInput, setRecoveryCodeInput] = useState("");
  const [recoveryCode, setRecoveryCode] = useState<string | null>(null);
  const [fromRecovery, setFromRecovery] = useState(false);
  const [ack, setAck] = useState(false);
  const [showPw, setShowPw] = useState(false);

  const st = strength(password);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    if ((mode === "signup" || mode === "recover") && password !== confirm) {
      setError("as senhas não coincidem");
      return;
    }
    setBusy(true);
    try {
      if (mode === "signup") {
        const { recoveryCode } = await signup(email.trim(), password);
        setFromRecovery(false);
        setRecoveryCode(recoveryCode); // shown once; unlock already happened
      } else if (mode === "recover") {
        const newCode = await doRecover(email.trim(), recoveryCodeInput.trim(), password);
        setFromRecovery(true);
        setRecoveryCode(newCode); // new emergency kit
      } else {
        await login(email.trim(), password);
      }
      // Persist (or clear) the remembered e-mail only after a successful auth.
      if (remember) {
        localStorage.setItem(REMEMBER_KEY, "1");
        localStorage.setItem(LAST_EMAIL_KEY, email.trim());
      } else {
        localStorage.removeItem(REMEMBER_KEY);
        localStorage.removeItem(LAST_EMAIL_KEY);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  // Recovery-code gate: the vault is already unlocked, but we hold the UI here
  // until the user confirms they saved the code (it is shown exactly once).
  if (recoveryCode) {
    return (
      <div className="grid h-full place-items-center p-8">
        <div className="card w-full max-w-lg p-6">
          <h1 className="text-lg font-semibold text-white">Seu kit de emergência</h1>
          <p className="mt-1 text-sm text-neutral-400">
            Este é o seu <strong>Recovery Code</strong>. Guarde-o offline agora — ele é exibido{" "}
            <strong>uma única vez</strong>. Sem ele, esquecer a senha-mestra significa perda total
            dos dados.
          </p>
          <pre className="mt-4 select-text rounded-lg border border-line bg-surface-0 p-4 text-center font-mono text-lg tracking-widest text-accent-soft">
            {recoveryCode}
          </pre>
          {fromRecovery && (
            <p className="mt-3 rounded-lg border border-yellow-600/40 bg-yellow-600/10 p-3 text-xs text-yellow-300">
              Cofre recuperado e novo kit gerado. Para concluir a redefinição da senha no servidor,
              use o link de redefinição por e-mail do Supabase (a senha local já foi re-embrulhada).
            </p>
          )}
          <label className="mt-4 flex items-center gap-2 text-sm text-neutral-300">
            <input type="checkbox" checked={ack} onChange={(e) => setAck(e.target.checked)} />
            Guardei o Recovery Code em local seguro.
          </label>
          <button
            className="btn-primary mt-4 w-full disabled:opacity-40"
            disabled={!ack}
            onClick={() => {
              setRecoveryCode(null);
              setAck(false);
              if (fromRecovery) setMode("login");
            }}
          >
            {fromRecovery ? "Voltar ao login" : "Continuar para o cofre"}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="grid h-full place-items-center p-8">
      <form onSubmit={submit} className="card w-full max-w-sm p-6">
        <div className="mb-6 flex items-center gap-2">
          <div className="grid h-9 w-9 place-items-center rounded-lg bg-accent font-bold text-white">
            E
          </div>
          <div>
            <div className="font-semibold text-white">EVEPass</div>
            <div className="text-xs text-neutral-500">cofre zero-knowledge</div>
          </div>
        </div>

        <label className="mb-1 block text-xs text-neutral-400">E-mail</label>
        <input
          className="field mb-3"
          type="email"
          autoFocus={!email}
          required
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          placeholder="voce@exemplo.com"
        />

        {mode === "recover" && (
          <>
            <label className="mb-1 block text-xs text-neutral-400">Recovery Code</label>
            <input
              className="field mb-3 font-mono"
              required
              value={recoveryCodeInput}
              onChange={(e) => setRecoveryCodeInput(e.target.value)}
              placeholder="XXXXX-XXXXX-…"
            />
          </>
        )}

        <label className="mb-1 block text-xs text-neutral-400">
          {mode === "recover" ? "Nova senha-mestra" : "Senha-mestra"}
        </label>
        <div className="relative">
          <input
            className="field pr-10"
            type={showPw ? "text" : "password"}
            autoFocus={mode === "login" && !!email}
            required
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="••••••••••••"
          />
          <button
            type="button"
            tabIndex={-1}
            onClick={() => setShowPw((s) => !s)}
            title={showPw ? "Ocultar" : "Mostrar"}
            className="absolute inset-y-0 right-0 grid w-10 place-items-center text-neutral-500 hover:text-neutral-300"
          >
            {showPw ? <EyeOff size={18} /> : <Eye size={18} />}
          </button>
        </div>

        {(mode === "signup" || mode === "recover") && (
          <>
            {mode === "signup" && (
              <div className="mt-2 flex items-center gap-2">
                <div className="h-1 flex-1 overflow-hidden rounded bg-surface-3">
                  <div
                    className="h-full bg-accent transition-all"
                    style={{ width: `${(st.score / 5) * 100}%` }}
                  />
                </div>
                <span className="w-20 text-right text-xs text-neutral-500">{st.label}</span>
              </div>
            )}
            <label className="mb-1 mt-3 block text-xs text-neutral-400">Confirmar senha</label>
            <input
              className="field"
              type={showPw ? "text" : "password"}
              required
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              placeholder="••••••••••••"
            />
          </>
        )}

        {mode === "login" && (
          <label className="mt-3 flex items-center gap-2 text-xs text-neutral-400">
            <input
              type="checkbox"
              checked={remember}
              onChange={(e) => setRemember(e.target.checked)}
            />
            Lembrar e-mail
          </label>
        )}

        {error && <p className="mt-3 text-sm text-red-400">{error}</p>}

        <button className="btn-primary mt-5 w-full disabled:opacity-40" disabled={busy}>
          {busy ? "…" : mode === "signup" ? "Criar conta" : mode === "recover" ? "Recuperar" : "Destravar"}
        </button>

        <button
          type="button"
          className="mt-3 w-full text-center text-xs text-neutral-500 hover:text-neutral-300"
          onClick={() => {
            setMode(mode === "signup" ? "login" : "signup");
            setError(null);
          }}
        >
          {mode === "signup" ? "Já tenho conta — entrar" : "Criar uma nova conta"}
        </button>

        {mode !== "recover" && (
          <button
            type="button"
            className="mt-1 w-full text-center text-xs text-neutral-600 hover:text-neutral-400"
            onClick={() => {
              setMode("recover");
              setError(null);
            }}
          >
            Esqueci a senha — usar Recovery Code
          </button>
        )}
      </form>
    </div>
  );
}
