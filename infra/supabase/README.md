# Supabase (Fase 0)

Backend zero-knowledge do EVEPass: Auth (GoTrue) + Postgres com RLS. O servidor
guarda **apenas ciphertext** (envelopes AEAD auto-descritivos). Nenhuma coluna
aqui é legível.

## Opção A — projeto na nuvem (free tier)

1. Crie um projeto em <https://supabase.com/dashboard>.
2. **Desligue a confirmação de e-mail** (a CLI de teste precisa de sessão
   imediata no signup): Authentication → Providers → Email → **Confirm email = OFF**.
3. Aplique o esquema: SQL Editor → cole `migrations/0001_init.sql` → Run.
   (Ou via CLI do Supabase: `supabase db push` com o projeto linkado.)
4. Pegue as chaves em Project Settings → API:
   - `Project URL`  → `SUPABASE_URL`
   - `anon public`  → `SUPABASE_ANON_KEY`

## Opção B — local (Docker)

```bash
supabase init          # se ainda não houver ./supabase
supabase start         # sobe Postgres/GoTrue/etc. em Docker
supabase db reset      # aplica as migrations em infra/supabase/migrations
```

Ajuste `supabase/config.toml` para `enable_confirmations = false` em `[auth.email]`.
O `supabase start` imprime a `API URL` e a `anon key`.

## Variáveis de ambiente para a CLI

```bash
export SUPABASE_URL="https://<ref>.supabase.co"      # ou http://127.0.0.1:54321
export SUPABASE_ANON_KEY="<anon key>"
```

## Fluxo de validação ponta a ponta

Do raiz do repositório (a CLI está no crate `evepass-cli`, binário `evepass`):

```bash
# 1. Signup — mostra o Recovery Code UMA vez. Guarde-o.
cargo run -q -p evepass-cli -- signup voce@exemplo.com

# 2. CRUD
cargo run -q -p evepass-cli -- add --title "Servidor Datasul" \
    --username diogo.admin --password 's3nh4' --url datasul.cliente.com
cargo run -q -p evepass-cli -- list
cargo run -q -p evepass-cli -- get <id>
cargo run -q -p evepass-cli -- edit <id> --password 'nova-senha'
cargo run -q -p evepass-cli -- rm <id>

# 3. Novo processo: login (prova persistência + prelogin)
cargo run -q -p evepass-cli -- logout
cargo run -q -p evepass-cli -- login voce@exemplo.com
cargo run -q -p evepass-cli -- list        # itens reaparecem e decifram

# 4. Troca de senha (itens NÃO são re-cifrados)
cargo run -q -p evepass-cli -- passwd

# 5. Recuperação pelo Recovery Code
cargo run -q -p evepass-cli -- recover voce@exemplo.com
```

Para rodar sem digitar a senha toda hora (útil em scripts de aceite), exporte
`EVEPASS_PASSWORD` — a CLI usa esse valor no lugar do prompt.

## Invariante a checar manualmente (critério de aceite)

Inspecione as linhas no Postgres e confirme que **não há nenhum plaintext**
(nem título, nem usuário, nem nome de pasta/tag):

```sql
select id, encode(ciphertext, 'hex') from items limit 5;
select user_id, encode(wrapped_vault_key, 'hex') from profiles;
```

Todo `ciphertext` deve começar com `01 01` (version 1 / alg 1) seguido do nonce
de 24 bytes e do texto cifrado — nada humanamente legível.
