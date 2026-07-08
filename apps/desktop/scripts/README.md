# Geradores de ícone

Scripts Node (sem dependências) que desenham os ícones do EVEPass como PNG.

## `gen-icon.mjs` — ícone do app

Squircle roxo com gradiente + fechadura branca (1024×1024 RGBA, com o padding da
grade da Apple ~80%). Gera o PNG de origem; use o `tauri icon` para derivar o
conjunto completo (macOS/iOS/Android/Windows):

```bash
node scripts/gen-icon.mjs /tmp/icon-src.png
npx tauri icon /tmp/icon-src.png          # popula src-tauri/icons/*
```

## `gen-tray.mjs` — ícones da menu bar (tray)

Cadeado monocromático **template** (preto + alpha; o macOS tinge conforme a
barra), em dois estados: fechado (travado) e aberto (destravado). Grava direto
em `src-tauri/icons/`:

```bash
node scripts/gen-tray.mjs src-tauri/icons  # tray-locked.png + tray-unlocked.png
```

Depois de regenerar, reinicie o app (`npm run tauri dev`) para reembutir os ícones.
