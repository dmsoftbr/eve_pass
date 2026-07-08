# EVEPass — Mobile (Fase 3)

App React Native sobre o **mesmo core Rust** (via UniFFI) + extensões **nativas**
de autofill (iOS AutoFill Credential Provider, Android AutofillService).

> **Estado:** o **core compila para iOS e Android** e suas funções de mobile
> (`match_credentials`, `extract_credential`, `session_from_vault_key`,
> `export_vault_key`) estão implementadas e testadas. O app RN e as extensões
> nativas deste diretório são um **scaffold** dos pontos de integração críticos —
> exigem um projeto RN bare (Xcode/Gradle) e um device/simulador para build e
> execução completos. Ver `docs/STATUS.md`.

## Arquitetura (recap do PRD)

- **Cripto + Session no core Rust**; **rede/Realtime no JS** (`@supabase/supabase-js`).
- A `vaultKey` **nunca** chega à camada JS do RN. No caminho biométrico ela vive
  no **enclave** (iOS Keychain / Android Keystore) e é manipulada só pelo módulo
  nativo, que chama `export_vault_key`/`session_from_vault_key` do core.
- Dois processos compartilham o cofre: **app principal** (destrava/sincroniza/cache)
  e **extensão de autofill** (lê o cache offline e devolve a credencial).

## 1. Build do core para mobile

```bash
# iOS → apps/mobile/native/ios/EvepassCore.xcframework + Swift bindings
./scripts/build-ios.sh

# Android → apps/mobile/native/android/jniLibs/*.so + Kotlin bindings
#   requer ANDROID_NDK_HOME e `cargo install cargo-ndk`
./scripts/build-android.sh
```

## 2. App React Native (bare)

O projeto RN bare ainda precisa ser gerado (`npx @react-native-community/cli init`
ou Expo prebuild) e então:

1. Linkar o `EvepassCore.xcframework` (iOS) e os `jniLibs` (Android).
2. Gerar o bridge JS do core com `uniffi-bindgen-react-native` a partir do
   `core/` e apontar `src/lib/core.ts` para o módulo gerado.
3. Telas em `src/screens/` (unlock, cofre, detalhe, edição) consomem o bridge.
4. `src/lib/supabase.ts` + sync (Realtime) espelham a Fase 1.

## 3. iOS — AutoFill Credential Provider

- Target de extensão `ASCredentialProviderViewController` (`native/ios/CredentialProviderViewController.swift`).
- **App Group** (`group.com.evepass`) para o arquivo de cache compartilhado.
- **Keychain access group** para a `vaultKey`.
- Provisão: `LAContext` (biometria) → `vaultKey` do Keychain → `session_from_vault_key`
  → decifra o cache → `match_credentials(domínio)` → `extract_credential(id)`
  → `ASPasswordCredential`.
- QuickType: popular `ASCredentialIdentityStore` com `ASPasswordCredentialIdentity`.

**Ativar no SO:** Ajustes › Senhas › AutoFill › EVEPass.

## 4. Android — AutofillService

- `native/android/EvepassAutofillService.kt` declarado no manifest.
- `vaultKey` no Android Keystore com `setUserAuthenticationRequired(true)`;
  `BiometricPrompt` (androidx.biometric) antes de recuperar a chave.
- `onFillRequest` → parse dos campos + package/domínio → biometria → `match_credentials`
  → `Dataset`/`FillResponse`. `onSaveRequest` → `save_item`.

**Ativar no SO:** Ajustes › Serviço de preenchimento automático › EVEPass.

## 5. Biometria (app principal)

Primeiro login por senha-mestra (fluxo da Fase 1). Ao ativar biometria: o módulo
nativo chama `export_vault_key` e guarda a `vaultKey` no enclave com controle
biométrico (`kSecAccessControlBiometryCurrentSet` / `setUserAuthenticationRequired`),
num grupo compartilhado com a extensão. Trocar a biometria do aparelho invalida a
chave e força re-login por senha-mestra.

## Segurança (invariantes desta fase)

- `vaultKey` só na `Session` (Rust) e no enclave; **nunca** no JS do RN.
- Extensão de autofill é **read-only** sobre o cache e exige biometria a cada uso.
- Matching por **eTLD+1** (implementado e testado no core), múltiplas URLs por item.
- Offline: a extensão usa só o cache local.

## Arquivos deste scaffold

```
apps/mobile/
├── src/lib/core.ts        # bridge JS → core (UniFFI RN)
├── src/screens/*          # unlock / cofre (RN)
└── native/
    ├── ios/               # extensão Swift + módulo biométrico + xcframework
    └── android/           # AutofillService Kotlin + Keystore + jniLibs
```
