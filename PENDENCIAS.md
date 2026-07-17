# EVEPass — Pendências

> Checklist acionável. Complementa `docs/STATUS.md` (fonte canônica de progresso).
> Última atualização: **2026-07-17**.
> Legenda: ✅ feito · 🟡 pendente de validação com recurso real · ⬜ não iniciado.

O núcleo do produto (Fases 0–4 + mobile M1–M4) está **implementado e compilando**.
O que resta é: (A) melhorias de UX pontuais, (B) código dos opcionais da Fase 5,
e (C) validação runtime que depende de recursos reais (Supabase provisionado,
2ª conta, aparelho físico, Chrome + host).

---

## A. Melhorias de UX (desktop)

- [x] **Duplo clique no ícone da tray abre o EVEPass** — antes só via botão
      direito → "Abrir EVEPass". `on_tray_icon_event` → `DoubleClick` →
      `show_main` (`src-tauri/src/lib.rs`). Botão direito ainda abre o menu.
- [x] **Destravar direto no command palette (Alt+Space)** — com o cofre travado,
      o palette mostra um campo de senha-mestra e destrava inline (encaminha a
      senha para a janela principal, que faz o login normal com o e-mail
      lembrado). Requer "Lembrar e-mail" ligado; senão orienta a destravar na
      janela principal. (`Palette.tsx`, `state/vault.tsx`, `lib/auth.ts`)
- [x] **Vazamento de itens no palette travado (corrigido)** — o palette exibia
      títulos/usuários decifrados mesmo após travar (o `catch` do search não
      limpava os hits). Agora limpa em `catch`, ao receber `vault-locked` e troca
      para a UI de unlock.
- [ ] 🟡 Aceite runtime das três acima com a GUI aberta (duplo clique abre;
      destravar pelo palette com senha certa/errada; nenhum título visível ao
      travar por inatividade).

## B. Código faltante — Fase 5 (opcionais)

- [x] **5A — Extensão de navegador (host + pareamento)**: bridge fino
      `native-host/` (`evepass-native-host`) + servidor socket no app
      (`src-tauri/src/host.rs`) contra a `Session` viva + `HostPairModal` de
      aprovação. Bridge testado ponta a ponta. Falta só validação runtime:
  - [ ] 🟡 Carregar a extensão no Chrome + registrar o host (ver
        `apps/browser-extension/README.md`), parear e preencher numa página real.
- [x] **Wire dos primitivos 5B/5C/5D no core + comandos desktop + testes** (61
      testes passam). Restam as pontas de shell/SQL/UX + validação runtime:
  - **5B pós-quântico** ✅ wired no compartilhamento (ML-KEM por conta +
    `wrap_collection_key_for_pq` + dispatch por versão, HPKE v1 / híbrido v2).
    - [ ] 🟡 SQL: coluna `mlkem_public_key` em `public_keys`/`profiles` + o shell
      buscar o ek do destinatário para usar o wrap PQ ao vivo.
  - **5C Secret Key (2SKD)** ✅ wired opt-in (enable/set/has + login usa o
    `secret.key` local automaticamente).
    - [ ] 🟡 UX: ativar nas configs (mostrar no kit), importar em novo device,
      atualizar authKey/wrapped no Supabase. Limitação: 1 secret por instalação.
  - **5D Passkeys** ✅ wired (`passkey_assert` + comandos create/list/sign).
    - [ ] 🟡 Cerimônia WebAuthn no navegador (extensão intercepta
      `navigator.credentials`) / providers iOS/Android.

## C. Código faltante — mobile iOS

- [ ] ⬜ **iOS autofill**: extensão `ASCredentialProviderViewController`.
- [ ] ⬜ **iOS biometria**: `BiometricVault` (Keychain / App Group).
- [ ] ⬜ **iOS device**: assinatura Apple para build em aparelho físico + App Store
      (hoje roda no simulador).

## D. Refinos Fase 4 (time)

- [ ] ⬜ Gestão completa de membros/rotação na UI (desktop).
- [ ] ⬜ Kit de emergência em PDF (hoje é o código exibido uma vez).
- [ ] ⬜ Reset de senha no servidor via e-mail (Supabase exige o fluxo de e-mail
      quando a senha é esquecida; a parte local — Recovery Code → vault key →
      re-wrap — já funciona).

## E. Validação runtime (precisa de recurso real, não código)

- [ ] 🟡 **ZK ponta a ponta** contra um Supabase real + inspeção do Postgres
      (nenhum título/usuário/pasta/tag legível). Cobre Fases 0–2 e 4.
- [ ] 🟡 **Desktop**: CRUD persiste como envelope, relogin, conflito offline,
      Realtime, hotkey/tray, breach real (HIBP), TOTP vs autenticador, auto-lock,
      import. Precisa de display + credenciais.
- [ ] 🟡 **Biometria física** (Android): digital/rosto reais + invalidação ao
      trocar a biometria do aparelho (emulador já passou).
- [ ] 🟡 **Autofill visual** (Android): preencher em **app nativo** ou no Chrome
      com autofill de terceiros ligado (serviço já registrado no SO).
- [ ] 🟡 **Sharing com 2ª conta real**: compartilhar coleção via HPKE + papéis
      (RLS) + rotação + isolamento do cofre pessoal. Precisa de 2 contas/devices.
