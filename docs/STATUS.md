# EVEPass — Status de implementação

> Fonte única de progresso. Atualizado em **2026-07-09**.
> (Mobile Android M1–M4 construídos e validados no emulador: cofre online + biometria + autofill
> (serviço registrado) + coleções/HPKE. APK release roda no device. Aceites finais no aparelho físico:
> biometria real, autofill em app nativo, sharing com 2ª conta.)
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

**M1 — App RN usável (online) ✅ APK gerado e roda no device/emulador (arm64-v8a):**

O app real vive no projeto que builda de fato: `apps/mobile/react-native-evepass-core/example/`
(turbo module UBRN + bindings gerados do core). Telas em `example/src/`:

- [x] Camada de rede Supabase no JS (`lib/supabase.ts`): prelogin, signup, login, profiles,
      items CRUD (upsert/soft-delete), Realtime — mirror da Fase 1, RN-flavored (AsyncStorage + url-polyfill)
- [x] Bridge do core (`lib/core.ts`) sobre os bindings UBRN reais; base64↔ArrayBuffer (`lib/bytes.ts`)
- [x] Auth (`lib/auth.ts`): `createAccount`/`deriveAuthKey`/`unlock` — vaultKey fica na `Session` (Rust)
- [x] Telas: **AuthScreen** (login/signup), **RecoveryCodeScreen** (código único), **VaultListScreen**
      (busca + quick-copy), **ItemDetailScreen** (revelar/copiar + TOTP ao vivo), **ItemFormScreen**
      (add/editar + gerador inline), **GeneratorScreen**
- [x] Estado (`state/vault.tsx`): itens decifrados em memória, Realtime, auto-lock em background,
      clipboard com auto-clear (`lib/clipboard.ts`)
- [x] `tsc` limpo (0 erros no app) · `assembleRelease` OK · APK instalado no emulador, AuthScreen renderiza
- [x] APK: `apps/mobile/react-native-evepass-core/example/android/app/build/outputs/apk/release/app-release.apk`
- Desvio consciente do mobile: sem `copy_field` nos bindings → o valor cruza o JS até o clipboard nativo
      (mitigado com auto-clear). No desktop a cópia acontece dentro do Rust.
- [x] **iOS:** app compila e roda no **simulador** (`xcodebuild` OK, AuthScreen renderiza no iPhone 17 Pro).
      Destravado o codegen RN do iOS: `includesGeneratedCode: true` faz o build da app pular a lib, então os
      artefatos de codegen precisam ser versionados — só existiam para Android. Gerados em `ios/generated/`
      (`react-native/scripts/generate-codegen-artifacts.js -t ios -s library`), `.gitignore` ajustado para
      versioná-los, e `EvepassCore.podspec` passou a incluir `.cpp` no glob de `ios/generated`.
      Falta: assinatura Apple para build em device físico + publicação na App Store.

**M2 — Biometria (Android) ✅ código completo, compila/linka/carrega no device:**

- [x] Módulo nativo Kotlin `BiometricVault` (`android/app/src/main/java/evepasscore/example/biometric/`):
      chave AES no Android Keystore com `setUserAuthenticationRequired(true)` +
      `setInvalidatedByBiometricEnrollment(true)`; cifra/decifra a vaultKey sob `BiometricPrompt`
      (CryptoObject); trata `KeyPermanentlyInvalidatedException` → limpa e força re-login por senha
- [x] Package registrado no `MainApplication`; dep `androidx.biometric:1.1.0`; `assembleRelease` OK
- [x] JS: `lib/biometric.ts` + `core.exportVaultKeyB64`/`sessionFromVaultKeyB64`; telas
      `EnableBiometricScreen` (oferta pós-login) + botão na `AuthScreen` + fases no `App.tsx`
- [x] **Validado no emulador (API 31, digital simulada):** signup → código de recuperação → cofre →
      ativar biometria (BiometricPrompt real + toque) → adicionar item (cifra + grava no Supabase) →
      **travar → cold start → desbloquear por biometria → item vem do Supabase e decifra**. Fluxo M1+M2 fecha.
- [x] **Bug corrigido:** `lib/bytes.ts` `b64ToBytes` truncava 1–2 bytes (subtraía padding de um comprimento
      que já o excluía) → salt/blobs corrompidos → "Invalid login credentials". Sem isso, login nunca funcionava.
- [x] `lock()` mantém a sessão do Supabase (só descarta a vaultKey) para o unlock biométrico reaproveitá-la.
- [ ] 🟡 Aceite final **no aparelho físico**: digital/rosto reais + invalidação ao trocar biometria do aparelho
- Desvio (escopo mobile): a vaultKey cruza o JS 1x no enable e 1x no unlock (a `Session` UBRN não é
      acessível do código nativo); nunca é persistida em claro nem logada.

**M3 — Autofill (Android) ✅ construído, compila, serviço registrado no SO:**

- [x] `EvepassAutofillService` (Kotlin): detecta campos usuário/senha + domínio/package (heurística
      hints/inputType), responde com autenticação → `AutofillAuthActivity`
- [x] `AutofillAuthActivity`: BiometricPrompt → `VaultKeystore.open` → `sessionFromVaultKey` →
      decifra o cache → `matchCredentials` (eTLD+1) → `extractCredential` → `Dataset` de volta
- [x] Decifra **no processo do autofill** via bindings **UniFFI Kotlin + JNA** (`libevepass_core.so`),
      coexistindo com o UBRN (que linka o Rust estático em `libappmodules.so`) — sem colisão
- [x] Cache offline de ciphertext (`AutofillCacheModule` RN → `filesDir/vault_cache.json`, gravado
      pelo `VaultProvider.refresh`); `VaultCache` lê e decifra; `VaultKeystore` compartilhado com o M2
- [x] Manifest: `<service BIND_AUTOFILL_SERVICE>` + activity translúcida; `assembleRelease` OK
- [x] **Verificado no emulador:** APK instala, `dumpsys autofill` mostra
      `Component: EvepassAutofillService` como serviço ativo; cofre destrava e popula o cache
- [ ] 🟡 Aceite do preenchimento visual: o Chrome (v149) usa o gerenciador do Google por padrão e não
      delega a serviços de terceiros → testar em **app nativo** (sempre usa o autofill do Android) ou
      com a opção de terceiros do Chrome ligada, no aparelho físico

**M4 — Coleções / compartilhamento (HPKE) ✅ construído e validado no emulador:**

- [x] `core.ts`: wrappers de `loadPrivateKeys`, `loadCollectionKeys`, `createCollection`,
      `wrapCollectionKeyFor`, `decryptCollectionName`, `encryptCollectionItem`, `decryptCollectionItem`,
      `publicKeyFingerprint` (base64 ↔ ArrayBuffer)
- [x] `supabase.ts`: `getMyPublicKeys`, `getPublicKeyByEmail`, `fetchMyCollectionMembers`,
      `fetchCollections`, `insertCollection`, `upsertCollectionMember`, `upsertItem` com `collection_id`
- [x] `auth.ts`: `hydrateCollections` (loadPrivateKeys + loadCollectionKeys) após unlock — nos dois
      caminhos (senha e biometria)
- [x] `vault.tsx`: itens de coleção decifrados via `decryptCollectionItem`; estado de coleções;
      `createCollection` (auto-membro admin), `lookupRecipient`, `shareCollection`
- [x] UI: chips de filtro (Pessoal + coleções) na lista, seletor de coleção no form,
      `CollectionsScreen` (criar + compartilhar com verificação de fingerprint + papel reader/writer/admin)
- [x] **Validado ao vivo no emulador:** criar coleção "Equipe" (nome cifrado, RLS `is_owner` no insert do
      membro) → adicionar item na coleção (`encrypt_collection_item`) → **travar → destravar por biometria →
      `hydrateCollections` reabre a chave via HPKE → item de coleção decifra**. Round-trip Fase 4 fecha.
- [ ] 🟡 Compartilhar com um **segundo usuário** real (mesmo código de `wrapCollectionKeyFor` +
      `upsertCollectionMember` que a auto-associação já exercita) — precisa de 2 contas/dispositivos

- [ ] ⬜ **M3 Autofill:** `AutofillService` (Kotlin) + cache offline compartilhado
- [ ] ⬜ **M4 Coleções:** telas de compartilhamento (core já expõe HPKE/coleções)
- [ ] ⬜ iOS: extensão `ASCredentialProviderViewController` + `BiometricVault` (Keychain/App Group)
- [ ] 🟡 Aceite runtime: signup/login live contra o Supabase no aparelho, autofill, biometria, offline
- Nota: as telas antigas em `apps/mobile/src/` são um scaffold anterior (imports para módulos inexistentes);
      o app vivo é o do `example/`.

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
