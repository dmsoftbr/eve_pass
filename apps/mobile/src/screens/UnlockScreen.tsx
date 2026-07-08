// EVEPass mobile — Unlock screen (scaffold).
//
// Biometric is the primary action; master password is the fallback. On the
// biometric path the NATIVE module retrieves the vaultKey from the enclave and
// builds the Session (via core.sessionFromVaultKey) — the key never enters this
// JS layer. The password path mirrors the desktop prelogin + begin/complete.
import { useEffect, useState } from "react";
import { NativeModules, Pressable, Text, TextInput, View } from "react-native";
import { evepass, type Session } from "../lib/core";
import { preloginParams, signIn } from "../lib/supabase";

// Native module (Swift/Kotlin) that drives biometrics + enclave and returns a
// ready Session handle. It calls core.sessionFromVaultKey internally.
const { BiometricVault } = NativeModules as {
  BiometricVault: {
    isEnabled(): Promise<boolean>;
    unlockWithBiometrics(): Promise<Session>;
  };
};

export function UnlockScreen({ onUnlocked }: { onUnlocked: (s: Session) => void }) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [bioAvailable, setBioAvailable] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    BiometricVault?.isEnabled().then(setBioAvailable).catch(() => setBioAvailable(false));
  }, []);

  async function unlockBiometric() {
    setError(null);
    try {
      const session = await BiometricVault.unlockWithBiometrics(); // key stays native
      onUnlocked(session);
    } catch {
      setError("biometria falhou — use a senha-mestra");
    }
  }

  async function unlockPassword() {
    setError(null);
    try {
      const { saltB64, params } = await preloginParams(email.trim());
      const salt = Uint8Array.from(atob(saltB64), (c) => c.charCodeAt(0));
      const begin = evepass.beginLogin(password, salt, params); // Argon2id once
      await signIn(email.trim(), begin.authKeyB64);
      // download profile.wrapped_vault_key, then begin.complete(wrapped) → Session
      // (completed in the real screen; omitted in the scaffold).
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <View style={{ flex: 1, justifyContent: "center", padding: 24, gap: 12 }}>
      <Text style={{ fontSize: 22, fontWeight: "600", color: "#fff" }}>EVEPass</Text>

      {bioAvailable && (
        <Pressable onPress={unlockBiometric} style={{ padding: 14, backgroundColor: "#6d5efc", borderRadius: 10 }}>
          <Text style={{ color: "#fff", textAlign: "center" }}>Desbloquear com biometria</Text>
        </Pressable>
      )}

      <TextInput
        placeholder="E-mail"
        autoCapitalize="none"
        value={email}
        onChangeText={setEmail}
        style={{ borderWidth: 1, borderColor: "#2a2a35", borderRadius: 10, padding: 12, color: "#fff" }}
      />
      <TextInput
        placeholder="Senha-mestra"
        secureTextEntry
        value={password}
        onChangeText={setPassword}
        style={{ borderWidth: 1, borderColor: "#2a2a35", borderRadius: 10, padding: 12, color: "#fff" }}
      />
      <Pressable onPress={unlockPassword} style={{ padding: 14, borderRadius: 10, borderWidth: 1, borderColor: "#2a2a35" }}>
        <Text style={{ color: "#fff", textAlign: "center" }}>Destravar</Text>
      </Pressable>

      {error && <Text style={{ color: "#f87171" }}>{error}</Text>}
    </View>
  );
}
