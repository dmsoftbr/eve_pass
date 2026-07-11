# EVEPass — Guia de produção e distribuição

Como empacotar, assinar e distribuir cada camada. Atualizado em 2026-07-08.

---

## Pré-requisito para o CI

O workflow `.github/workflows/desktop-release.yml` roda no GitHub. Para usá-lo:

1. Crie um repositório no GitHub e faça push do projeto.
2. **Settings → Secrets and variables → Actions** → adicione os secrets abaixo.
3. Dispare uma release com uma tag: `git tag v0.1.0 && git push origin v0.1.0`
   (ou rode manualmente em Actions → Desktop Release → Run workflow).

O workflow builda **macOS (universal), Windows e Linux** e cria uma **Release em
rascunho** com os instaladores anexados.

---

## Desktop (Tauri)

### Build local (uma plataforma)

```bash
cd apps/desktop && npm run tauri build
# macOS → src-tauri/target/release/bundle/{macos/EVEPass.app, dmg/*.dmg}
```

### Secrets do CI

| Secret | Para quê |
|---|---|
| `VITE_SUPABASE_URL`, `VITE_SUPABASE_ANON_KEY` | Config do frontend, embutida no bundle |
| `APPLE_CERTIFICATE` | base64 do seu `Developer ID Application.p12` (`base64 -i cert.p12 \| pbcopy`) |
| `APPLE_CERTIFICATE_PASSWORD` | senha do `.p12` |
| `APPLE_SIGNING_IDENTITY` | ex.: `Developer ID Application: Seu Nome (TEAMID)` |
| `APPLE_ID`, `APPLE_PASSWORD` | Apple ID + **app-specific password** (notarização) |
| `APPLE_TEAM_ID` | id do time no Apple Developer |
| `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | assinatura do auto-updater (opcional) |

### macOS — assinatura + notarização

**Status:** configurado. Certificado *Developer ID Application* criado, os secrets
`APPLE_*` do CI estão setados e o job `macos-latest` está ativo no workflow — cada
tag `v*` gera um DMG **universal (Intel + Apple Silicon) assinado e notarizado**.

Sem assinar, o usuário teria que liberar o Gatekeeper na mão
(`xattr -dr com.apple.quarantine /Applications/EVEPass.app`). Com a assinatura+
notarização, o `.dmg` abre com duplo-clique, sem aviso.

#### Build local assinado + notarizado (sem CI)

Pré-requisitos (uma vez): certificado *Developer ID Application* no keychain
(Xcode → Settings → Accounts → Manage Certificates → `+`) e uma **app-specific
password** (appleid.apple.com). Depois:

```bash
cd apps/desktop
export APPLE_SIGNING_IDENTITY="Developer ID Application: Diogo Moreira (WCJ5WCJ3N4)"
export APPLE_ID="<apple-id-email>"
export APPLE_PASSWORD="<app-specific-password>"   # NÃO commitar; é revogável
export APPLE_TEAM_ID="WCJ5WCJ3N4"
npm run tauri build
# → o Tauri assina o .app, notariza (notarytool) e faz staple; gera o .dmg em
#   src-tauri/target/release/bundle/dmg/EVEPass_<versão>_aarch64.dmg
```

O Tauri notariza o **.app**, mas não o **.dmg** em si. Para o download abrir 100%
limpo, notarize+staple o próprio DMG também:

```bash
DMG=src-tauri/target/release/bundle/dmg/EVEPass_<versão>_aarch64.dmg
xcrun notarytool submit "$DMG" --apple-id "<email>" --password "<app-pass>" \
  --team-id WCJ5WCJ3N4 --wait
xcrun stapler staple "$DMG"
# valida:  spctl --assess -t open --context context:primary-signature -vv "$DMG"
#          → "accepted / source=Notarized Developer ID"
```

#### Regenerar o secret do certificado (se rotacionar)

O CI importa o cert de `APPLE_CERTIFICATE` (base64 de um `.p12`) + `APPLE_CERTIFICATE_PASSWORD`:

```bash
security export -k ~/Library/Keychains/login.keychain-db -t identities \
  -f pkcs12 -P "<senha-p12>" -o devid.p12
base64 -i devid.p12 | gh secret set APPLE_CERTIFICATE
printf '%s' "<senha-p12>" | gh secret set APPLE_CERTIFICATE_PASSWORD
rm devid.p12
```

### Windows

O CI gera `.msi`/`.exe`. Para evitar o aviso do SmartScreen, assine com um
certificado de code-signing (configure `bundle.windows.certificateThumbprint` no
`tauri.conf.json` num runner Windows com o cert instalado, ou assine com `signtool`
num passo extra).

### Linux

O CI gera `.deb` e `.AppImage`. O `.AppImage` roda em qualquer distro sem instalar.

### Auto-update (opcional, recomendado p/ time)

1. Gere o par de chaves: `npm run tauri signer generate -- -w ~/.tauri/evepass.key`
2. Adicione ao `tauri.conf.json`:
   ```json
   "plugins": { "updater": { "endpoints": ["https://<seu-host>/latest.json"],
     "pubkey": "<CONTEÚDO DA .pub>" } }
   ```
   e o plugin `tauri-plugin-updater` no backend.
3. Guarde a **chave privada** nos secrets (`TAURI_SIGNING_PRIVATE_KEY`). O CI assina
   os bundles; você hospeda o `latest.json` (ex.: no próprio GitHub Releases).
4. O app checa e aplica updates assinados sozinho.

---

## Mobile

### Estado

O **core Rust roda no app RN** (bridge UniFFI/UBRN validado no simulador iOS). As
**telas do EVEPass** (unlock, cofre, cópia, TOTP, sync, biometria) ainda precisam
ser portadas sobre o bridge — sem isso o app mobile ainda não é usável. Ver
`apps/mobile/README.md` e a task de "Portar telas".

### iOS

- Exige **conta Apple Developer**.
- Build release no Xcode (workspace `apps/mobile/react-native-evepass-core/example/ios`)
  ou `npx react-native run-ios --configuration Release`.
- Distribuição: **TestFlight** (interno/externo — o caminho normal p/ time) ou App Store.
- As extensões de autofill (Fase 3) são targets adicionais no mesmo app.

### Android (mais simples)

- Gere um **keystore** e assine o release:
  ```bash
  cd apps/mobile/react-native-evepass-core/example/android
  ./gradlew assembleRelease   # → app/build/outputs/apk/release/app-release.apk
  ```
- Distribua o **APK direto** (sideload, grátis, sem loja) ou pela Play Store
  (o track *internal testing* é rápido).
- Requer buildar o core para Android antes: `./scripts/build-android.sh`
  (`cargo-ndk` + `ANDROID_NDK_HOME`).

---

## Backend (Supabase)

Já em produção e validado (zero-knowledge + RLS). Para operar de verdade:

- **Backups**: ligue o Point-in-Time Recovery (plano pago).
- **Free tier** pausa com inatividade e tem limites — ok p/ pessoal/time; monitore.
- **Migração p/ self-host** é direta (Postgres + GoTrue), sem tocar no cliente.
- **Nunca** exponha a `service_role` key; a `anon`/`publishable` é pública (ok embutir).

---

## Resumo por público

| Público | Desktop | Mobile |
|---|---|---|
| Só você | DMG local + liberar Gatekeeper | (após portar telas) run local |
| Time | CI assinado/notarizado + auto-update | iOS: TestFlight · Android: APK direto |
| Público geral | Lojas (Mac App Store / MS Store) + notarização | App Store + Play Store |
