# EVEPass — Extensão de navegador (Fase 5A)

Autofill no navegador **sem cofre próprio**: a extensão (MV3) conversa com o app
desktop por **native messaging** (IPC local, não rede). O app detém a `Session`
e as chaves; a credencial só cruza para a extensão **no momento do fill**.

> **Estado:** extensão (content/background/popup) + **host de native messaging
> implementado** (bridge `native-host/` + servidor socket no app desktop) +
> **pareamento com aprovação na UI**. Falta apenas a **validação runtime** com o
> Chrome real (carregar a extensão, registrar o host, preencher em uma página).
> Ver `docs/STATUS.md`.

## Arquitetura

```
content.js  ──sendMessage──▶  background.js  ──native messaging──▶  app desktop (core + Session)
 (detecta campos,               (service worker,                     (match_credentials por eTLD+1,
  injeta UI de fill)             porta do host)                       decifra a credencial só no fill)
```

## Protocolo (JSON, ambos os sentidos)

| Requisição | Resposta |
|---|---|
| `{type:"status"}` | `{locked: bool}` |
| `{type:"match", domain}` | `{candidates:[{id,title,username}]}` |
| `{type:"getCredential", id}` | `{username, password}` *(só no fill)* |
| `{type:"saveCredential", domain, username, password}` | `{ok: bool}` |

O `match` reusa `match_credentials` do core (eTLD+1, Fase 3). O framing do native
messaging é: 4 bytes little-endian de tamanho + JSON UTF-8.

## Host de native messaging (implementado)

O host é uma **ponte fina** (`native-host/` no workspace raiz →
`evepass-native-host`) que o Chrome inicia com stdio. Ela **não** guarda cofre:

1. Lê frames stdin (4 bytes len + JSON), responde em stdout.
2. Injeta o `_origin` (a extensão chamadora, `argv[1]` do Chrome) e encaminha o
   JSON por um **socket Unix local** (`~/.evepass/host.sock`) ao app desktop.
3. O app (módulo `apps/desktop/src-tauri/src/host.rs`) atende contra a `Session`
   viva: `status` sempre responde; `match`/`getCredential`/`saveCredential`
   exigem o cofre **destravado** e a origem **pareada**. A credencial só cruza no
   `getCredential` (momento do fill). Se o app não estiver rodando, o bridge
   responde `{locked:true}` (status) ou um erro — degradação graciosa.

**Pareamento:** a 1ª conexão de uma origem desconhecida emite `host-pair-request`;
o app mostra um modal de aprovar/recusar e persiste a origem aprovada em settings
(prompt único por extensão).

### Registro (macOS Chrome)

1. Compile o bridge: `cargo build -p evepass-native-host` (ou `--release`).
2. Edite `native-host/com.evepass.host.json`:
   - `path` → caminho **absoluto** do binário (`.../target/debug/evepass-native-host`
     em dev, ou `/Applications/EVEPass.app/Contents/MacOS/evepass-native-host`
     empacotado).
   - `allowed_origins` → `chrome-extension://<ID real da extensão>/`.
3. Copie esse JSON para
   `~/Library/Application Support/Google/Chrome/NativeMessagingHosts/com.evepass.host.json`.
4. Rode o app EVEPass (destravado) e carregue a extensão (abaixo).

## Carregar em modo dev

Chrome → `chrome://extensions` → Developer mode → "Load unpacked" → esta pasta.
Copie o **ID** gerado para o `allowed_origins` do manifesto do host.

## Segurança (invariantes)

- Cofre e chaves ficam **no app**; a extensão nunca os vê.
- A credencial cruza para a extensão **só no `getCredential`** (momento do fill).
- Pareamento com aprovação; `Session` destravada obrigatória.
- Passkeys (Fase 5D): a extensão pode atuar como autenticador WebAuthn
  interceptando `navigator.credentials.create/get` e delegando `create_passkey`/
  `passkey_sign` ao core via o mesmo host.
