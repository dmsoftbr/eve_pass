# Publicar o EVEPass (iOS) na App Store

Automação com [fastlane](https://fastlane.tools). Roda tudo de dentro de
`apps/mobile/react-native-evepass-core/example/ios/`.

## Visão geral

| Etapa | Quem faz | Como |
|-------|----------|------|
| Assinatura do build | Xcode (conta logada) | Automatic signing + `-allowProvisioningUpdates` |
| Upload TestFlight/Store | App Store Connect API key | `.p8` sem 2FA |
| Identidade do app | já configurado | bundle `br.com.dmsoft.evepass`, nome "EVEPass" |

## Pré-requisitos (uma vez)

1. **Conta paga no Apple Developer Program** ✅ (vocês já têm).
2. **Team ID**: https://developer.apple.com/account → *Membership details* → copie o
   *Team ID* (10 caracteres).
3. **App Store Connect API key** (recomendado, evita 2FA):
   App Store Connect → *Users and Access* → *Integrations* → *App Store Connect API*
   → gere uma key com papel *App Manager*. Baixe o `AuthKey_XXXXXX.p8`
   (**só dá pra baixar uma vez**). Anote o *Key ID* e o *Issuer ID*.
4. **Xcode logado**: Xcode → Settings → Accounts → adicione a conta Apple do Team.
   (É o que permite o Automatic signing criar o profile de distribuição.)

## Configurar segredos

```bash
cd apps/mobile/react-native-evepass-core/example/ios
cp fastlane/.env.example fastlane/.env
# edite fastlane/.env: EVEPASS_TEAM_ID, ASC_KEY_ID, ASC_ISSUER_ID
# e coloque o AuthKey_XXXX.p8 em fastlane/AuthKey.p8 (ou aponte ASC_KEY_PATH)
```
`fastlane/.env` e o `.p8` estão no `.gitignore` — não são versionados.

## Instalar

```bash
cd apps/mobile/react-native-evepass-core/example
gem install bundler        # precisa de bundler >= 2
bundle install             # instala o fastlane (via Gemfile)
```

## Fluxo

```bash
cd apps/mobile/react-native-evepass-core/example/ios

# 1) (uma vez) cria o registro do app no App Store Connect
bundle exec fastlane bootstrap

# 2) sobe um build de teste pro TestFlight
bundle exec fastlane beta

# 3) quando estiver pronto pra loja
bundle exec fastlane release
```

- `beta` e `release` primeiro chamam `core` (recompila o `evepass-core` em release e
  regenera o `EvepassCoreFramework.xcframework` com a fatia de **device** `ios-arm64`),
  depois `build` (instala Pods → `gym` Release → `.ipa` assinado em `build/ipa/EVEPass.ipa`),
  depois fazem o upload.
- O `xcframework` é artefato de build (não versionado), então é regenerado a cada
  publicação. A 1ª compilação do core em release leva ~5 min; depois é incremental.
  Pra pular quando já está construído: `EVEPASS_SKIP_CORE=1 bundle exec fastlane beta`.
- O bundle JS é embutido automaticamente no Release (build phase do RN); **não**
  precisa do Metro rodando pra build de release.
- O número de build é auto-incrementado a partir do último no TestFlight.

> Requer Rust (rustup via brew) no ambiente — a lane `core` já injeta o PATH do
> rustup automaticamente. Verificado: build **Release para device (ios-arm64)**
> linka o core e embute o `main.jsbundle`; só falta a assinatura (Team/API key).

## Pendências antes de sair na loja pública

Estas **não** são resolvidas pelo fastlane — são decisões/artefatos que faltam:

1. **Ícone do app** ✅ — resolvido. `AppIcon.appiconset` tem o ícone 1024×1024
   (keyhole roxo, full-bleed, sem alpha — o iOS aplica a máscara arredondada).
   O actool gera os demais tamanhos a partir dele.
2. **Export compliance (criptografia)** ✅ configurado (com obrigação anual) — o
   `Info.plist` declara `ITSAppUsesNonExemptEncryption = true` (o app cifra o cofre;
   não é isento). Isso usa a isenção de **mercado de massa** (License Exception ENC):
   algoritmos padrão, **sem CCATS**. Falta você, **uma vez**, no App Store Connect,
   selecionar a exceção correspondente quando pedir a documentação de export, e depois
   **enviar o relatório anual de auto-classificação** à BIS/NSA (`crypt@bis.doc.gov` +
   `enc@nsa.gov`) — é o mesmo caminho de 1Password/Bitwarden. Sem isso o build não
   distribui na loja.
3. **Política de privacidade** (URL obrigatória) + questionário *App Privacy*.
4. **Screenshots** por tamanho de tela + descrição/categoria (pode preencher no
   App Store Connect na 1ª vez; depois `bundle exec fastlane deliver init` versiona).
5. **AutoFill Credential Provider** (extensão iOS) — ainda não construída
   (`ASCredentialProviderViewController`, ver `docs/STATUS.md`). Dá pra publicar sem,
   e adicionar depois.

## CI (futuro)

Automatic signing depende do Xcode logado localmente. Pra rodar em CI, migre a
assinatura pra [`match`](https://docs.fastlane.tools/actions/match/) (certificados
num repo git privado criptografado) — o resto do Fastfile continua igual.
