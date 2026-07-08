import { useEffect, useState } from 'react';
import { Text, View, StyleSheet } from 'react-native';
import { generatePassword, passwordScore } from 'react-native-evepass-core';

// Proves the RN → Rust core bridge (UniFFI): both calls run in evepass-core.
export default function App() {
  const [pw, setPw] = useState('');
  const [score, setScore] = useState<number | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    try {
      const p = generatePassword({
        length: 20,
        upper: true,
        lower: true,
        digits: true,
        symbols: true,
      });
      setPw(p);
      setScore(passwordScore(p));
    } catch (e) {
      setErr(String(e));
    }
  }, []);

  return (
    <View style={styles.container}>
      <Text style={styles.brand}>🔐 EVEPass</Text>
      <Text style={styles.sub}>core Rust via UniFFI (UBRN)</Text>
      {err ? (
        <Text style={styles.err}>{err}</Text>
      ) : (
        <>
          <Text style={styles.label}>Senha gerada no core:</Text>
          <Text style={styles.mono} selectable>
            {pw}
          </Text>
          <Text style={styles.label}>Força (zxcvbn 0–4)</Text>
          <Text style={styles.score}>{score ?? '…'}</Text>
        </>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, alignItems: 'center', justifyContent: 'center', padding: 24, backgroundColor: '#0b0b0f' },
  brand: { fontSize: 28, fontWeight: '700', color: '#fff' },
  sub: { color: '#8a8a99', marginBottom: 28 },
  label: { color: '#8a8a99', marginTop: 18, fontSize: 13 },
  mono: { color: '#8b7fff', fontFamily: 'Menlo', fontSize: 16, marginTop: 6, textAlign: 'center' },
  score: { color: '#34d399', fontSize: 40, fontWeight: '700', marginTop: 2 },
  err: { color: '#f87171', marginTop: 12, textAlign: 'center' },
});
