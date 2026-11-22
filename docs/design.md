# Rust Hosts — Design

## Objetivo

App desktop pessoal para Linux que gerencia um bloco do `/etc/hosts` com entradas agrupadas, permitindo alternar ambientes com um clique sem editar o arquivo cru.

## Não-objetivos do MVP

Windows/macOS; perfis de ambiente; editor de texto cru; histórico/undo separado (backups são a recuperação); tray/autostart; múltiplos arquivos; hosts remotos; Flatpak; acessibilidade (Iced 0.14 stable não tem).

## Modelo do arquivo

O app só altera o bloco:

```text
# BEGIN rust-hosts
# Dev Environment
127.0.0.1 development.example.com
127.0.0.1 penpot.example.com
# Homolog
# 172.25.0.80 hml.example.com
# END rust-hosts
```

- O bloco é único; múltiplos blocos ou marcadores malformados são erro.
- Conteúdo fora do bloco é preservado byte a byte.
- Dentro do bloco o texto é normalizado ao aplicar: um espaço entre IP e hostnames, `# ` nas entradas desabilitadas e o EOL dominante do arquivo.
- Grupos são comentários que não são entradas desabilitadas; todo comentário nessa condição é um cabeçalho de grupo, e grupos vazios persistem.
- Entrada desabilitada é uma linha de entrada comentada; habilitada é a linha normal.
- Toda entrada pertence a um grupo; um grupo só pode ser excluído quando vazio.
- O bloco novo é criado no fim do arquivo.

## Importação

- Sem bloco, ou pelo menu Import: ler entradas fora do bloco e agrupá-las pelos comentários.
- Preview com checkboxes; grupos com hostnames de sistema (`localhost`, `ip6-*`) desmarcados por padrão.
- Entradas sem grupo aparecem como "Ungrouped" e, se selecionadas, entram num grupo "Ungrouped".
- A importação move as entradas selecionadas, removendo-as do local original; não copia.
- Comentários e comentários de linha são preservados quando possível.
- Diff completo, backup e aplicação via helper.

## Aplicar

1. Serializa o bloco.
2. Relê `/etc/hosts` e compara o hash com o carregado; se mudou, bloqueia e oferece Reload/Diff.
3. Mostra o diff do bloco; em import/restore, mostra o diff do arquivo inteiro.
4. Cria backup timestamped em `~/.local/state/rust-hosts/backups/` (últimos 20).
5. Chama `pkexec /usr/local/libexec/rust-hosts-helper --apply-block --expected-hash <hex>` com o novo bloco no stdin.
6. O helper valida, substitui o bloco, preserva dono/modo e faz escrita atômica (temp + fsync + rename); falha se o hash não bater.
7. Recarrega e mostra sucesso/erro.

## Restaurar

- Lista backups com timestamp e tamanho; diff contra o atual; confirmação.
- Faz backup do estado atual antes; o helper `--restore` grava o conteúdo completo do backup após validação.
- Restaurar e importar são as operações que reescrevem o arquivo inteiro (o helper recebe o conteúdo no stdin, com `--restore`); o diff deixa isso explícito.

## Conflito externo

Qualquer mudança no `/etc/hosts` desde o carregamento bloqueia o Apply. A UI oferece Reload (com confirmação se houver edições não salvas) ou Diff. Nunca sobrescrever em silêncio.

## Segurança

- A GUI roda como usuário; se `geteuid() == 0` sem subcomando de helper, recusa iniciar.
- Helper root-owned em `/usr/local/libexec/rust-hosts-helper`, instalado pelo próprio app via `pkexec install`; a GUI roda de qualquer caminho.
- O helper só escreve `/etc/hosts`; valida UTF-8, tamanho, marcadores e hash; preserva modo/dono; escrita atômica.
- `pkexec` é usado apenas para o helper; cancelamento e erro viram mensagem na UI.
- Validação: IP via `IpAddr`; hostnames sem whitespace/controle; grupo não vazio e sem marcadores; duplicatas são avisadas mas permitidas.

## UI

Textos da UI em inglês; documentação em português.

- Janela 900x600, título "Rust Hosts", tema padrão do Iced.
- Topo: busca por IP/hostname/grupo, Import, Restore.
- Lista por grupos: cabeçalho com toggle, nome, contagem, + Entry, renomear/excluir (excluir só vazio); linhas com toggle, IP, hostnames, editar/excluir.
- Rodapé: estado dirty + Apply.
- "Unmanaged entries" colapsável e somente leitura.
- Estados: sem bloco (Import/Create block), bloco vazio, arquivo alterado, erro.
- Modais: Add/Edit Entry (IP, hostnames, grupo), Rename, Import, Diff/Apply, Restore, Confirm Delete/Reload/Close. Erros e o resultado do Apply aparecem numa faixa no rodapé.
- Busca filtra entradas; grupo que casa mostra todas; grupo sem matches some.
- Toggle de grupo: se todas habilitadas, desabilita todas; senão habilita todas.
- Fechar com alterações não salvas pede confirmação.

## Empacotamento

- `cargo build --release` gera a GUI e o helper; o app instala o helper root-owned via `pkexec` e mostra o comando equivalente para copiar.
- `packaging/` traz `.desktop` e ícone para quem quiser instalar a GUI no menu.
- Desenvolvimento: `cargo run --release`; o helper é instalado uma vez.
- Sem Flatpak (o sandbox não escreve no `/etc` do host sem escape). AppImage só no futuro, com helper root-owned.

## Testes e aceitação

- Parser round-trip com fixtures: bloco no fim/topo, sem bloco, múltiplos/malformados, grupos vazios, disabled, comentários de linha, CRLF, IPv6, tabs e sem newline final.
- Importação: move grupos, preserva sistema/comentários, diff/backup.
- Helper: hash errado, conteúdo inválido, marcadores duplicados, modo/dono e atomicidade em testes unitários; integração manual.
- UI: smoke manual em Wayland/GNOME: CRUD, toggle, busca, import, apply, cancelar pkexec, restore e dirty close.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo build --release`.

## Futuro

Perfis de ambiente; export/import de fragmentos; tema/atalhos; acessibilidade quando o Iced suportar; AppImage se portabilidade for necessária.

## Decisões registradas

`GLOSSARY.md` e ADRs `0001`–`0005`.
