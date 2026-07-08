# EVEPass — Desktop (Fase 1)

MVP de desktop: Tauri v2 (backend Rust) + React/Vite/Tailwind. O backend Rust
consome o `evepass-core` **direto** (sem UniFFI) e mantém a `Session`/`vaultKey`
e o cache local cifrado. O React faz o I/O do Supabase (auth, REST, Realtime) e
**nunca** recebe material de chave — só ciphertext e, para exibir/editar, o
plaintext de um item.

## Pré-requisitos

- Node 20+ e Rust (o mesmo toolchain do repositório).
- Um projeto Supabase com o esquema da Fase 0 aplicado (`infra/supabase`).

## Configuração

```bash
cd apps/desktop
cp .env.example .env      # preencha VITE_SUPABASE_URL e VITE_SUPABASE_ANON_KEY
npm install
```

## Rodar

```bash
npm run tauri dev         # sobe o Vite + a janela nativa
```

Primeira execução: **Criar conta** → guarde o Recovery Code (exibido uma vez) →
o cofre abre. Nas próximas, **Destravar** com e-mail + senha-mestra.

## Build de produção

```bash
npm run tauri build       # gera o binário/instalador em src-tauri/target
```

## Fronteira de segurança (invariante da fase)

- `vaultKey` e `encKey` vivem **só no Rust** (`src-tauri/src/state.rs`).
- `copy_field` decifra e escreve no clipboard **dentro do Rust** — o valor não
  passa pelo JS.
- O cache local (`~/Library/Application Support/.../evepass/cache-<user>.sqlite`)
  guarda **envelopes** (ciphertext AEAD), não plaintext. SQLCipher entra na Fase 2.
- Travar (botão 🔒 ou fechar a janela) descarta a `Session` (zeroize).

## Estrutura

- `src/lib/ipc.ts` — wrappers tipados dos comandos Tauri.
- `src/lib/supabase.ts` — único ponto que fala com o Supabase (conversões bytea↔base64).
- `src/lib/auth.ts` — signup/login (prelogin + begin/complete_login, Argon2 uma vez).
- `src/lib/sync.ts` — warm-up, Realtime e fila de upload offline.
- `src/state/vault.tsx` — estado do cofre + ações.
- `src/components/*` — AuthScreen, Sidebar, ItemList, ItemDetail, ItemForm, PasswordGenerator.
- `src-tauri/src/` — `commands.rs` (fronteira), `state.rs`, `cache.rs` (SQLite).
