# EVEPass — Extensão de navegador (Fase 5A)

Autofill no navegador **sem cofre próprio**: a extensão (MV3) conversa com o app
desktop por **native messaging** (IPC local, não rede). O app detém a `Session`
e as chaves; a credencial só cruza para a extensão **no momento do fill**.

> **Estado:** scaffold funcional da extensão (content/background/popup) + o
> protocolo e o manifesto do host. Falta implementar o **host de native
> messaging** no app desktop e o **pareamento** — build/run precisa do Chrome e
> do registro do host no SO. Ver `docs/STATUS.md`.

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

## Host de native messaging (a implementar no app)

O host é um executável que o Chrome inicia com stdio. Caminho realista: um modo
do app desktop (`evepass-native-host`) que:

1. Lê frames stdin (len + JSON), responde em stdout.
2. Encaminha para a `Session` viva (via IPC local com o app principal, ou é o
   próprio app rodando).
3. Exige o cofre **destravado**; **pareamento** com aprovação do usuário na
   primeira conexão de cada extensão.

Registro (macOS Chrome): copiar `native-host/com.evepass.host.json` (com o
`path` do binário e o `allowed_origins` = id real da extensão) para
`~/Library/Application Support/Google/Chrome/NativeMessagingHosts/`.

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
