# Rust Hosts

App desktop para Linux que gerencia um bloco marcado do `/etc/hosts` com grupos de entradas. A ideia é trocar um ambiente de desenvolvimento ligando/desligando linhas, sem editar o arquivo cru.

```text
# BEGIN rust-hosts
# Dev
127.0.0.1 development.example.com
# 172.25.0.80 hml.example.com   ← entrada desabilitada
# END rust-hosts
```

Tudo fora dos marcadores é preservado byte a byte.

## Recursos

- Grupos colapsáveis com toggle individual e toggle do grupo; Enable/Disable all no rodapé.
- CRUD de entradas (IP, um ou mais hostnames, grupo), busca por IP/hostname/grupo.
- Importação de entradas que já estão fora do bloco, com preview e desmarque de entradas de sistema.
- Diff antes de cada escrita, highlight das linhas alteradas e aviso de hostnames repetidos no Apply.
- Backup timestampado antes de toda escrita (últimos 20) e Restore com diff.
- Aviso no rodapé quando o arquivo muda fora do app, com Show diff e Reload.
- Instalação do helper de elevação a partir do próprio app.

## Requisitos

- Linux com `pkexec`/polkit e Rust 1.85+ (edition 2024).
- Interface gráfica (Wayland ou X11).

## Como rodar

```sh
cargo build --release   # gera os dois binários em target/release/
cargo run --release     # abre a GUI
```

O `cargo run` compila só o binário da GUI; rode `cargo build --release` uma vez para ter também `target/release/rust-hosts-helper`, que é o arquivo que o app instala.

Na primeira execução o app mostra uma faixa dizendo que o helper não está instalado. O botão **Install helper** roda `pkexec /usr/bin/install -Dm755 -o root -g root <helper> /usr/local/libexec/rust-hosts-helper`; o botão ao lado copia o comando equivalente para quem preferir executar à mão. **Copy command** e **Recheck** estão na mesma faixa. Enquanto o helper falta ou responde com outro número de protocolo, o Apply fica desabilitado.

A GUI roda como usuário e se recusa a iniciar como root.

## Como uma aplicação funciona

1. O app serializa o bloco e relê `/etc/hosts`, comparando o hash com o da carga inicial.
2. Mostra o diff; confirmação abre backup em `~/.local/state/rust-hosts/backups/` (mantém os 20 mais recentes).
3. O conteúdo vai no stdin do helper root-owned via `pkexec --apply-block --expected-hash <hex>`; o helper valida e grava com temp + fsync + rename, preservando dono e modo.
4. Se o arquivo mudou no disco, a escrita é bloqueada: Reload ou Diff, nunca sobrescrita silenciosa.

Import e Restore reescrevem o arquivo inteiro com `--restore`; Apply troca apenas o bloco.

## Estrutura

| Arquivo | Papel |
|---|---|
| `src/block.rs` | modelo, parser e serializador do bloco |
| `src/hosts.rs` | leitura, hash, troca do bloco e escrita atômica |
| `src/helper.rs`, `src/bin/rust-hosts-helper.rs` | validação e escrita como root |
| `src/apply.rs` | chamada via `pkexec`, instalação e checagem do helper |
| `src/backup.rs`, `src/import.rs` | backups e importação |
| `src/app.rs`, `src/app/view.rs` | estado, mensagens e layout Iced |
| `tests/parser.rs` | fixtures de aceite do formato |

O parser e a lógica de escrita são UI-free e testáveis; a camada Iced só chama essas funções. Os textos da interface são em inglês; a documentação é em português.

## Desenvolvimento

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

Os testes usam fixtures: nunca leem/escrevem o `/etc/hosts` real e nunca chamam `pkexec`. A verificação visual ainda é manual, na sessão gráfica do usuário.

## Documentação

- `docs/design.md` — comportamento acordado e escopo do MVP.
- `docs/adr/0001`–`0006` — decisões (edição estruturada, pkexec, Iced, bloco, formato e instalação do helper).
- `GLOSSARY.md` — vocabulário do domínio.
- `AGENTS.md` — invariantes de segurança e convenções para agentes.

## Fora de escopo

Windows/macOS, perfis de ambiente, editor de texto cru, undo, tray/autostart, Flatpak, CLI e acessibilidade (o Iced 0.14 stable não oferece). A checagem do helper é pelo número de protocolo: se você recompilar o helper sem mudar esse número, use **Install helper** para atualizar a cópia instalada.
