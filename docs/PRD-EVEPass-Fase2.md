# PRD — EVEPass · Fase 2: Experiência premium

> **Status (2026-07-06): ✅ implementado e compilando.** Command palette (2ª janela + hotkey global), tray/menu bar com estado + esconder ao fechar + autostart, smart views (fracas/reutilizadas/sem 2FA via `vault_health`; vazadas via HIBP k-anonymity com hashes ficando no Rust), TOTP ao vivo, auto-lock + limpeza de clipboard, configurações e import (Bitwarden/NordPass/CSV). 🟡 Pendente a validação runtime da GUI. Progresso em [`STATUS.md`](./STATUS.md).

> Terceiro PRD da série. Adiciona a camada que faz o EVEPass sair de "funciona" para "prazeroso de usar", mais higiene de segurança e import. Consumir com Claude Code.

## 1. Objetivo

Transformar o MVP da Fase 1 num app com acabamento de produto: acesso instantâneo por **command palette + atalho global**, presença permanente na **bandeja/menu bar**, **smart views** (incl. detecção de senhas vazadas), **TOTP ao vivo**, e higiene de segurança (**auto-lock** + **limpeza de clipboard**). Fecha com **import** do Bitwarden/NordPass/CSV para você migrar de verdade.

## 2. Pré-requisitos

Fase 1 concluída: unlock, CRUD, pastas/tags, cache local, sync via Realtime, `Session` no backend Rust. Todas as regras de segurança da Fase 1 continuam valendo (chaves só no Rust; plaintext atravessa a fronteira só para exibir).

## 3. Escopo

**Dentro:** command palette + hotkey global; tray/menu bar + rodar em background + iniciar no login; smart views (fracas, reutilizadas, sem 2FA, vazadas); TOTP ao vivo (código + contador); auto-lock por inatividade/sono; limpeza automática de clipboard; import (Bitwarden JSON, NordPass CSV, CSV genérico); tela de configurações.

**Fora (Fases 3–5):** app mobile e autofill (Fase 3); sharing/collections e recovery polido (Fase 4); autofill de desktop, extensão de navegador, pós-quântico, passkeys (Fase 5).

## 4. Novos comandos e eventos

Estendem o contrato da Fase 1. Invariante mantida: senhas e hashes de senha **não** cruzam para o JS — o que cruza no breach são apenas prefixos k-anônimos.

```rust
// Saúde do cofre (calculada no Rust; senhas nunca saem)
struct HealthReport { weak: Vec<String>, reused: Vec<Vec<String>>, no_totp: Vec<String> }
fn vault_health() -> HealthReport;

// Breach via HIBP k-anonymity
fn breach_prefixes() -> Vec<String>;                 // SHA-1 dos passwords, só os 5 primeiros hex (únicos)
struct Range { prefix: String, body: String }        // body = resposta do HIBP p/ aquele prefixo
fn resolve_breaches(ranges: Vec<Range>) -> Vec<String>; // Rust casa sufixos → ids vazados

// TOTP ao vivo
struct TotpCode { code: String, seconds_remaining: u32 }
fn item_totp(id: String) -> Result<TotpCode>;        // detalhe e palette dão poll a cada 1s

// Import (o JS faz o parse; o Rust cifra)
fn save_items_batch(items_json: Vec<String>) -> Result<Vec<Saved>>;
fn save_folders_batch(folders: Vec<(String, Option<String>)>) -> Result<Vec<Saved>>; // (name, parent_id)

// Palette
struct PaletteHit { id: String, title: String, username: String, has_totp: bool }
fn palette_search(query: String) -> Vec<PaletteHit>; // fuzzy sobre o cofre destravado

// Clipboard com auto-clear (estende o copy_field da Fase 1)
fn copy_field(id: String, field: String) -> Result<()>; // agenda limpeza conforme settings; só limpa se inalterado

// Configurações
struct Settings { auto_lock_minutes: u32, clipboard_clear_seconds: u32,
                  launch_at_login: bool, global_hotkey: String, theme: String }
fn get_settings() -> Settings;
fn set_settings(s: Settings) -> Result<()>;          // re-registra hotkey/autostart quando muda
```

**Eventos (Rust → JS):** `vault-locked` (emitido no auto-lock, para a UI voltar ao unlock).

## 5. Command palette + atalho global

- **Janela dedicada:** uma segunda janela Tauri, pequena, sem moldura, centralizada, sempre-no-topo, mostrada/escondida pelo hotkey — independente de a janela principal estar aberta ou o app estar na bandeja.
- **Hotkey global:** via `tauri-plugin-global-shortcut`. Default configurável (ex.: `Alt+Space` ou `Ctrl+Shift+P` — evitar conflito com Spotlight). Registrado no boot; re-registrado quando muda nas configurações.
- **Comportamento destravado:** digita → `palette_search` (fuzzy) → resultados; setas navegam; **Enter copia a senha** (via `copy_field`, dentro do Rust); **⌘/Ctrl+Enter abre o item na janela principal**; um atalho copia o usuário e outro o TOTP; **ESC** esconde.
- **Comportamento travado:** o hotkey traz a janela principal focada no unlock (uma quick-unlock nativa na palette é *stretch* desta fase).

## 6. Tray / menu bar

- **Ícone persistente** (menu bar no macOS, tray no Windows/Linux) com estado **travado/destravado** (ícones distintos).
- **Menu:** abrir janela principal · abrir command palette · travar/destravar · iniciar no login (toggle) · sair.
- **Rodar em background:** fechar a janela principal a **esconde na bandeja** (não encerra o app); sair só pelo menu da bandeja.
- **Iniciar no login:** via `tauri-plugin-autostart`, refletindo o toggle das configurações.

## 7. Smart views

Calculadas no Rust sobre os itens decifrados (`vault_health`), expostas como filtros na sidebar (substituem os placeholders da Fase 1):

- **Senhas fracas:** score do `zxcvbn` < 3 **ou** comprimento < 12.
- **Reutilizadas:** a mesma senha (por hash interno) aparece em ≥ 2 itens de login.
- **Sem 2FA:** item do tipo login sem campo `totp`.
- **Vazadas:** aparece no HIBP Pwned Passwords, checado por **k-anonymity**:

```mermaid
sequenceDiagram
  participant JS as React
  participant R as Rust (core + Session)
  participant H as HIBP
  JS->>R: breach_prefixes()
  R-->>JS: [prefix5...]   (SHA-1 dos passwords, só os 5 primeiros hex)
  loop cada prefixo
    JS->>H: GET /range/{prefix5}
    H-->>JS: sufixos:contagem
  end
  JS->>R: resolve_breaches(ranges)
  R-->>JS: [ids vazados]   (Rust casa sufixos com os hashes que guardou; hashes nunca saem)
```

Resultado do breach cacheado com timestamp; recalcular sob demanda ou periodicamente.

## 8. TOTP ao vivo

No detalhe do item (e na palette), o campo TOTP mostra o **código atual + anel/contador de segundos**, dando poll em `item_totp(id)` a cada 1 s. Botão de copiar o código (via Rust). O segredo `otpauth://` continua sendo só um campo do item; o cálculo é no core (`totp-rs`).

## 9. Auto-lock + limpeza de clipboard

- **Auto-lock:** após `auto_lock_minutes` de inatividade (o front envia pings de atividade; um timer no Rust dispara `lock()` + emite `vault-locked`). Também travar no **sono/lock do sistema** (best-effort via eventos do SO). Travar descarta a `Session`.
- **Limpar clipboard:** depois de `copy_field`, o Rust agenda a limpeza após `clipboard_clear_seconds` (ex.: 30 s) e **só limpa se o conteúdo ainda for o valor copiado** (não sobrescreve se o usuário copiou outra coisa).

## 10. Import

O JS faz o parse (o arquivo já é plaintext, fornecido pelo usuário); o Rust cifra via `save_items_batch`/`save_folders_batch`.

- **Bitwarden (JSON):** mapear `folders[]` → pastas; `items[].login.{username,password,uris,totp}` → campos; `notes`, `fields` (custom), `folderId` → pertencimento.
- **NordPass (CSV):** colunas `name,url,username,password,note,folder,…` → campos correspondentes.
- **CSV genérico:** tela de **mapeamento de colunas** → campos do EVEPass; preview antes de confirmar.
- MVP não faz dedupe: cria itens novos e o usuário revisa. **Avisar o usuário para apagar o arquivo de origem** (contém senhas em claro).

## 11. Configurações

Tela com: tema (claro/escuro/sistema), tempo de auto-lock, tempo de limpeza de clipboard, iniciar no login, atalho global (com captura de tecla), e ponto de entrada do import. Persistidas em `settings` (tabela local ou `tauri-plugin-store`) — não são sensíveis. Alterações re-aplicam hotkey e autostart na hora.

## 12. Segurança da fase

- Palette copia via `copy_field` **no Rust**; valores não passam pelo JS.
- Breach: só prefixos k-anônimos e dados públicos do HIBP cruzam a fronteira; hashes completos ficam no Rust.
- Import: senhas em claro só transitam na memória durante o parse no JS e são imediatamente cifradas pelo Rust; alertar sobre o arquivo de origem.
- Auto-lock e limpeza de clipboard reduzem a janela de exposição.

## 13. Critérios de aceite

- [ ] Hotkey global abre a palette com o app em background/bandeja; busca, copia (senha/usuário/TOTP) e ESC fecham; ⌘/Ctrl+Enter abre na janela principal.
- [ ] Fechar a janela principal mantém o app na bandeja; ícone reflete travado/destravado; iniciar-no-login funciona.
- [ ] Smart views listam corretamente fracas, reutilizadas e sem 2FA.
- [ ] Breach detecta uma senha sabidamente vazada (ex.: "password") e **nenhum hash completo** sai para o JS/rede (só o prefixo de 5 hex).
- [ ] TOTP mostra código + contador que atualizam a cada segundo e o código bate com um app autenticador de referência.
- [ ] Auto-lock trava após o tempo configurado e no sono do sistema; a UI volta ao unlock.
- [ ] Clipboard é limpo após o tempo configurado, mas **não** se o usuário copiou outra coisa nesse meio-tempo.
- [ ] Import de um export do Bitwarden e de um CSV traz itens e pastas corretos, com senhas cifradas no cofre.

## 14. Bibliotecas (novas)

- **Tauri plugins:** `tauri-plugin-global-shortcut`, `tauri-plugin-autostart`, `tauri-plugin-clipboard-manager`, `tauri-plugin-store`. (Tray/menu bar é nativo do Tauri v2.)
- **Rust (core):** `zxcvbn` (força de senha), `sha1` (prefixo HIBP). `totp-rs` já vem da Fase 0.
- **JS:** parser de CSV (`papaparse`); fetch ao HIBP (`api.pwnedpasswords.com/range/{prefix}`).

## 15. Checklist de execução (ordem sugerida)

1. Tray/menu bar + rodar em background (esconder ao fechar) + iniciar no login.
2. Segunda janela (palette) + hotkey global + `palette_search` + ações de cópia/abrir/ESC.
3. `vault_health` + smart views na sidebar (fracas/reutilizadas/sem 2FA).
4. Breach: `breach_prefixes` + fetch HIBP no JS + `resolve_breaches` + view "vazadas".
5. TOTP ao vivo (`item_totp`) no detalhe e na palette.
6. Auto-lock (pings de atividade + timer + evento + sono do SO) e limpeza de clipboard no `copy_field`.
7. Import: parsers Bitwarden/NordPass/CSV genérico + mapeamento + `save_*_batch`.
8. Tela de configurações + persistência + re-aplicação de hotkey/autostart.
9. Passar por todos os critérios de aceite, com atenção ao teste de que hashes não vazam no breach.
```
