// Breach check via HIBP Pwned Passwords with k-anonymity. Only the first 5 hex
// of each password's SHA-1 (computed in Rust) leaves the machine; the full hash
// never does. Rust matches the returned suffixes back to item ids.
import { ipc } from "./ipc";

export async function computeBreached(): Promise<string[]> {
  const prefixes = await ipc.breachPrefixes();
  if (prefixes.length === 0) return [];
  const ranges = await Promise.all(
    prefixes.map(async (prefix) => {
      try {
        const res = await fetch(`https://api.pwnedpasswords.com/range/${prefix}`, {
          headers: { "Add-Padding": "true" },
        });
        return { prefix, body: res.ok ? await res.text() : "" };
      } catch {
        return { prefix, body: "" };
      }
    }),
  );
  return ipc.resolveBreaches(ranges);
}
