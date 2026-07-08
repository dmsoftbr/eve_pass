// Orchestrates signup/login/recovery across the Rust core (via ipc) and Supabase.
// Resolves the prelogin chicken-and-egg and runs Argon2id exactly once per login
// (begin_login → sign in → complete_login).
import { ipc } from "./ipc";
import * as sb from "./supabase";

/** Establish an unlocked session; returns the Supabase user id. */
async function establishSession(email: string, password: string): Promise<string> {
  const { saltB64, params } = await sb.preloginParams(email);
  const begin = await ipc.beginLogin(password, saltB64, params); // Argon2id once
  const userId = await sb.signIn(email, begin.auth_key_b64);
  const { wrappedVaultKeyB64, wrappedPrivateKeysB64 } = await sb.getProfileKeys();
  await ipc.completeLogin(begin.login_token, wrappedVaultKeyB64, wrappedPrivateKeysB64, userId);

  // Load this user's collection keys (HPKE-unwrapped in Rust) so shared items
  // decrypt. Non-fatal if there are none.
  try {
    const members = await sb.fetchMyCollectionMembers(userId);
    if (members.length) {
      await ipc.loadCollectionKeys(
        members.map((m) => ({
          collection_id: m.collectionId,
          wrapped_collection_key_b64: m.wrappedCollectionKeyB64,
          sender_signing_pub_b64: m.senderSigningPubB64,
        })),
      );
    }
  } catch {
    /* collections optional */
  }
  return userId;
}

export async function doSignup(
  email: string,
  password: string,
): Promise<{ userId: string; recoveryCode: string }> {
  const acc = await ipc.createAccount(password); // core derives all keys
  const userId = await sb.signUp(email, acc.auth_key_b64); // GoTrue account
  await sb.insertLoginParams(email, acc); // prelogin row (+ recovery wrap)
  await sb.insertProfile(userId, acc); // wrapped keys
  await sb.insertPublicKeys(userId, email, acc); // shareable public keys
  const sessionUserId = await establishSession(email, password); // unlock
  return { userId: sessionUserId, recoveryCode: acc.recovery_code };
}

export async function doLogin(email: string, password: string): Promise<string> {
  return establishSession(email, password);
}

export async function doLock(): Promise<void> {
  await ipc.lock();
  await sb.signOut();
}

/**
 * Recovery (Fase 4 §9): recover the vault key from the Recovery Code, set a new
 * password + rotate the code, and return the new emergency-kit code.
 *
 * NOTE: this recovers the vault locally and updates the local wrapped keys. The
 * server-side auth password (the base64 authKey) still requires Supabase's
 * email-based reset to change it when the old password is unknown — deferred to
 * that flow. The core capability — Recovery Code → vault key → new keys with
 * collection access preserved — is what runs here.
 */
export async function doRecover(email: string, recoveryCode: string, newPassword: string): Promise<string> {
  const { saltB64, params, wrappedVaultKeyRecoveryB64 } = await sb.preloginParams(email);
  if (!wrappedVaultKeyRecoveryB64) throw new Error("recovery não disponível para esta conta");
  await ipc.unlockWithRecovery(recoveryCode, wrappedVaultKeyRecoveryB64);
  const reset = await ipc.resetPassword(newPassword, saltB64, params);
  return reset.recovery_code; // new emergency kit
}
