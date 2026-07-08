# EVEPass — Status de implementação

> Fonte única de progresso. Atualizado em **2026-07-06**.
> (Fase 3: core mobile + pipeline de build compilando; app RN/extensões em scaffold.
> Fase 4: sharing E2E via HPKE + recuperação — core testado, desktop compila.)
> Legenda: ✅ implementado e compilando · 🟡 pendente de validação runtime · ⬜ não iniciado.
>
> "Validação runtime" = rodar de fato contra um projeto Supabase provisionado e,
> no desktop, com a GUI aberta. Requer ação do usuário (credenciais + display);
> ver `infra/supabase/README.md` e `apps/desktop/README.md`.

## Visão geral

| Fase | Escopo | Código | Validação runtime |
|---|---|---|---|
| 0 | Fundação criptográfica (core Rust + CLI + Supabase) | ✅ | 🟡 |
| 1 | MVP desktop (Tauri v2 + React) | ✅ | 🟡 |
| 2 | Experiência premium (palette, tray, smart views, TOTP, auto-lock, import) | ✅ | 🟡 |
| 3 | Mobile (RN) + autofill nativo | 🟡 parcial | — |
| 4 | Time (collections + HPKE sharing + recovery polido) | ✅ (desktop) | 🟡 |
| 5 | Opcionais (browser ext, pós-quântico, Secret Key, passkeys) | 🟡 parcial | — |

**Testes automatizados:** `evepass-core` — **57/57** passam (vetores RFC:
Argon2id 9106, HKDF 5869, XChaCha20-Poly1305, X25519 7748, Ed25519 8032; matching
eTLD+1; caminho biométrico; sharing HPKE E2E + assinatura + rotação; **pós-quântico
híbrido** X25519+ML-KEM-768; **Secret Key** 2SKD; **passkey** P-256 ES256).
**Builds:** frontend (`tsc + vite`) e backend Tauri (`cargo`) sem warnings; core
compila para **iOS** (xcframework) e **Android** (.so) via UniFFI.

---

## Fase 0 — Fundação criptográfica ✅ (runtime 🟡)

- [x] Workspace `core/` + `cli/` + `infra/`
- [x] `envelope.rs` + `aead.rs` (XChaCha20-Poly1305) com stub de dispatch v2
- [x] `kdf.rs` (Argon2id 256 MiB + `calibrate_kdf`)
- [x] `keys.rs` (hierarquia HKDF + wrap/unwrap da vaultKey)
- [x] `account.rs` (`create_account`/`unlock`/`unlock_with_recovery`/`change_password`)
- [x] `keypair.rs` (X25519 + Ed25519), `recovery.rs` (Recovery Code 128-bit)
- [x] `item.rs`, `folder.rs`, `generator.rs`, `totp.rs`
- [x] Bindings UniFFI (Swift + Kotlin gerados)
- [x] Vetores conhecidos por primitive + round-trip + senha errada sem panic
- [x] Migration `0001_init.sql` (esquema + RLS + `login_params` + Realtime)
- [x] `evepass-cli` (signup/login/logout/add/list/get/edit/rm/passwd/recover/gen)
- [ ] 🟡 Fluxo ZK ponta a ponta contra Supabase real + inspeção de plaintext no Postgres

## Fase 1 — MVP desktop ✅ (runtime 🟡)

- [x] Core: `begin_login`/`complete_login` (Argon2id uma vez no login)
- [x] Scaffold Tauri v2 + Vite/React/TS/Tailwind
- [x] Backend: `state.rs` (Session/keys só no Rust), `cache.rs` (SQLite + fila dirty)
- [x] Comandos: auth, CRUD de itens/pastas, `copy_field` (clipboard no Rust)
- [x] Sync: `apply_remote_changes` (LWW + cópia de conflito), `pending_uploads`, `mark_synced`
- [x] Frontend: onboarding + unlock (Recovery Code uma vez), prelogin dance
- [x] Janela principal: sidebar (pastas/tags) + lista + detalhe + form CRUD + gerador
- [x] Busca fuzzy (⌘K), Realtime (supabase-js), fila offline, travar (manual + ao fechar)
- [ ] 🟡 Aceite runtime: CRUD persiste como envelope, relogin, conflito offline, Realtime, sem plaintext no Postgres

## Fase 2 — Experiência premium ✅ (runtime 🟡)

- [x] Core: `password_score` (zxcvbn) + `sha1_hex`
- [x] `vault_health` (fracas/reutilizadas/sem 2FA)
- [x] Breach HIBP k-anonymity (`breach_prefixes`/`resolve_breaches`; hashes ficam no Rust)
- [x] TOTP ao vivo (`item_totp`) + anel de contador no detalhe
- [x] Command palette (2ª janela sem moldura + hotkey global) + `palette_search`
- [x] Tray/menu bar com estado 🔒/🔓 + esconder ao fechar + iniciar no login
- [x] Auto-lock por inatividade (timer + evento `vault-locked`) + limpeza de clipboard
- [x] Import (Bitwarden JSON / NordPass / CSV genérico com mapeamento)
- [x] Configurações (tema, auto-lock, clipboard, hotkey, autostart) persistidas
- [ ] 🟡 Aceite runtime: hotkey/tray/breach real/TOTP vs autenticador/auto-lock/import

## Fase 3 — Mobile + autofill 🟡 (parcial)

**Fundação verificada de verdade** (artefatos gerados + testes):

- [x] Core: `match_credentials` (eTLD+1 via `psl`), `extract_credential`,
      `session_from_vault_key`, `Session.export_vault_key` — 7 testes passam
      (subdomínios, múltiplas URLs, package sem eTLD+1, round-trip da chave)
- [x] Pipeline de build do core para mobile (`scripts/build-ios.sh`, `build-android.sh`)
- [x] **iOS:** `EvepassCore.xcframework` (device + simulador) + bindings Swift **gerados e compilando**
- [x] **Android:** `libevepass_core.so` (arm64-v8a + x86_64) + bindings Kotlin **gerados e compilando**
- [x] Bindings expõem as 4 novas funções (verificado em Swift e Kotlin)

**Scaffold (código dos pontos de integração; build/run completo precisa de projeto RN + device):**

- [~] App RN: bridge do core (`src/lib/core.ts`), telas unlock/cofre, sync (mirror da Fase 1)
- [~] iOS: extensão `ASCredentialProviderViewController` + `BiometricVault` (Keychain/App Group)
- [~] Android: `AutofillService` + `BiometricVault` (Keystore/BiometricPrompt)
- [ ] ⬜ Gerar o projeto RN bare, linkar xcframework/jniLibs + `uniffi-bindgen-react-native`,
      completar telas, e wire dos targets de extensão (Xcode/Gradle)
- [ ] 🟡 Aceite runtime: autofill em Safari/Chrome + apps, biometria, offline, invalidação por troca de biometria

Detalhes de build e ativação no SO: `apps/mobile/README.md`.

## Fase 4 — Time (collections + recuperação) ✅ desktop (runtime 🟡)

**Core (testado):**

- [x] `create_collection`, `wrap_collection_key_for` (HPKE seal + assinatura Ed25519),
      `load_collection_keys` (verifica assinatura + HPKE open), `encrypt/decrypt_collection_item`,
      `decrypt_collection_name`, `rotate_collection_key`, `public_key_fingerprint`
- [x] `reset_password` (recuperação: nova senha + rotaciona o Recovery Code, preserva acesso às collections)
- [x] HPKE = RFC 9180 (DHKEM-X25519 + HKDF-SHA256 + ChaCha20-Poly1305)
- [x] 4 testes: share E2E entre 2 usuários, rejeição de assinatura forjada, rotação, fingerprint

**SQL (migration 0002):** `public_keys` (legível por autenticados), funções `is_member`/`can_write`/`is_admin`,
políticas RLS de collections/membros/itens compartilhados, `sender_signing_pub` + `wrapped_vault_key_recovery`.

**Desktop (compila):**

- [x] Chaves privadas carregadas na Session no login; collection keys via HPKE no unlock
- [x] Comandos: create/wrap/load/rotate/decrypt_name, fingerprint, reset_password, unlock_with_recovery
- [x] Cache com `collection_id`; itens de collection decifram com a chave certa; health/breach cobrem shared
- [x] UI: seção Collections na sidebar (criar/filtrar/compartilhar), **ShareModal** (email → fingerprint → papel),
      seletor de collection no form, indicador 👥 na lista, fluxo de recuperação por Recovery Code
- [ ] 🟡 Aceite runtime: share entre 2 contas reais, papéis (RLS), rotação, isolamento do cofre pessoal
- [ ] ⬜ Refinos: gestão completa de membros/rotação na UI; kit de emergência em PDF (hoje é o código exibido);
      reset do password no servidor via e-mail (o Supabase exige o fluxo de e-mail quando a senha é esquecida)

## Fase 5 — Opcionais 🟡 (parcial)

Módulos independentes. Os dois habilitados pela camada de agilidade (5B, 5C) e o
core de passkeys (5D) estão **implementados e testados**; a extensão (5A) é scaffold.

- [x] **5B — Pós-quântico híbrido** (`core/src/pq.rs`): `hybrid_wrap`/`hybrid_unwrap`
      com **X25519 + ML-KEM-768** combinados via HKDF, sob nova versão de envelope
      (`0x02`). 4 testes: round-trip, o KEM PQ e o clássico ambos contribuem
      (chave errada de qualquer um falha), dispatch por versão. *Só a camada
      assimétrica muda; a de repouso (XChaCha20) não.* Integração no wrap de
      collection (ML-KEM nos profiles) é o passo restante.
- [x] **5C — Secret Key (2SKD)** (`core/src/keys.rs::derive_enc_auth_with_secret`):
      HKDF com salt = Secret Key de 128 bits. Teste prova que breach do servidor +
      senha-mestra **sem** a Secret Key não desembrulha a vaultKey. Onboarding/UX é
      o passo restante.
- [x] **5D — Passkeys** (`core/src/passkey.rs`): `create_passkey` (par P-256 +
      rpId/userHandle como item cifrado), `passkey_sign` (assertion WebAuthn ES256,
      DER). 2 testes: create→sign→verify + tipo de item. Providers (iOS/Android da
      Fase 3, extensão 5A) são a integração restante.
- [~] **5A — Extensão de navegador** (`apps/browser-extension/`): MV3
      (content/background/popup) + protocolo de native messaging
      (`status/match/getCredential/saveCredential`, reusa `match_credentials`) +
      manifesto do host. Falta o **host no app desktop** + pareamento; build/run
      precisa do Chrome + registro do host.
- [ ] ⬜ Integração viva de 5B/5C/5D no fluxo de conta/UI + 5A host + validação runtime.
