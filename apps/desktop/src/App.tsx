import { useVault } from "./state/vault";
import { AuthScreen } from "./components/AuthScreen";
import { MainWindow } from "./components/MainWindow";
import { HostPairModal } from "./components/HostPairModal";
import { isConfigured } from "./lib/supabase";

export default function App() {
  const { status } = useVault();

  if (!isConfigured()) {
    return (
      <div className="grid h-full place-items-center p-8">
        <div className="card max-w-md p-6 text-center">
          <h1 className="mb-2 text-lg font-semibold text-white">Configuração necessária</h1>
          <p className="text-sm text-neutral-400">
            Crie um arquivo <code className="text-accent-soft">.env</code> em{" "}
            <code>apps/desktop</code> com <code>VITE_SUPABASE_URL</code> e{" "}
            <code>VITE_SUPABASE_ANON_KEY</code> (veja <code>.env.example</code>) e reinicie o app.
          </p>
        </div>
      </div>
    );
  }

  return (
    <>
      {status === "unlocked" ? <MainWindow /> : <AuthScreen />}
      {/* Only meaningful while unlocked, but the listener is cheap and harmless. */}
      <HostPairModal />
    </>
  );
}
