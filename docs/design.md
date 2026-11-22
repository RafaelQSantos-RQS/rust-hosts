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
- Entrada antes de qualquer cabeçalho é erro de parse: o app abre na tela de erro e oferece Reload ou Restore.
- O bloco novo é criado no fim do arquivo.

## Importação

- Sem bloco, ou pelo menu Import: ler entradas fora do bloco e agrupá-las pelos comentários.
- Preview com checkboxes; entradas de sistema (`localhost`, `ip6-*`) começam desmarcadas.
- Entradas sem grupo aparecem como "Ungrouped" e, se selecionadas, entram num grupo "Ungrouped".
- A importação move as entradas selecionadas, removendo-as do local original; não copia.
- Linhas com comentário depois dos hostnames não são importáveis e ficam onde estão; os demais comentários são preservados.
- Diff completo, backup e aplicação via helper.

## Aplicar

1. Serializa o bloco.
2. Relê `/etc/hosts` e compara o hash com o carregado; se mudou, bloqueia e oferece Reload/Diff.
3. Mostra o diff do bloco; em import/restore, mostra o diff do arquivo inteiro. No Apply, avisa se algum hostname aparece em mais de uma entrada habilitada — o primeiro casamento vence e os demais ficam sem efeito.
4. Cria backup timestamped em `~/.local/state/rust-hosts/backups/` (últimos 20).
5. Chama `pkexec /usr/local/libexec/rust-hosts-helper --apply-block --expected-hash <hex>` com o novo bloco no stdin.
6. O helper valida, substitui o bloco, preserva dono/modo e faz escrita atômica (temp + fsync + rename); falha se o hash não bater.
7. Recarrega e mostra sucesso/erro.

## Restaurar

- Lista backups com timestamp e tamanho; diff contra o atual; confirmação.
- Faz backup do estado atual antes; o helper `--restore` grava o conteúdo completo do backup após validação.
- Restaurar e importar são as operações que reescrevem o arquivo inteiro (o helper recebe o conteúdo no stdin, com `--restore`); o diff deixa isso explícito.

## Conflito externo

Qualquer mudança no `/etc/hosts` desde o carregamento bloqueia o Apply. Uma checagem do hash ao focar a janela ou ao digitar avisa no rodapé sem interromper a edição, com Show diff (somente leitura) e Reload (com confirmação se houver edições não salvas). A confirmação antes de escrever reconfere o hash. Nunca sobrescrever em silêncio.

## Segurança

- A GUI roda como usuário e recusa iniciar se `geteuid() == 0`.
- Helper root-owned em `/usr/local/libexec/rust-hosts-helper`, instalado pelo próprio app via `pkexec install`; a GUI roda de qualquer caminho.
- O helper só escreve `/etc/hosts`; valida UTF-8, tamanho, marcadores e hash; preserva modo/dono; escrita atômica.
- O helper responde `--protocol`; se o binário instalado faltar ou divergir, o app desabilita o Apply em vez de gravar com contrato incompatível.
- `pkexec` é usado apenas para o helper e para a sua instalação; cancelamento e erro viram mensagem na UI.
- Validação: IP via `IpAddr`; hostnames sem whitespace/controle; grupo não vazio e sem marcadores; duplicatas são permitidas.

## UI

Textos da UI em inglês; documentação em português.

- Janela 900x600, título "Rust Hosts", tema padrão do Iced.
- Topo: busca por IP/hostname/grupo, + Group, Import, Restore.
- Lista por grupos: cabeçalho com toggle, nome, contagem, + Entry, renomear/excluir (excluir só vazio); linhas com toggle, IP, hostnames, editar/excluir.
- Rodapé: estado dirty, mensagem de status (clique dispensa), Enable/Disable all, Reload e Apply como única ação primária; Apply fica desabilitado sem alterações.
- "Unmanaged entries" colapsável e somente leitura.
- Se o helper instalado falta ou é de outra versão, uma faixa no rodapé desabilita o Apply e oferece Install helper (`pkexec install`), Copy command e Recheck.
- Se o arquivo mudou no disco, outra faixa oferece Show diff e Reload; ela permanece até uma recarga.
- Estados: sem bloco (Import/Create block), bloco vazio (New group/Import), arquivo alterado, erro de leitura (Reload/Restore).
- Criar grupo: New group no bloco vazio, + Group no topo quando já há grupos. O grupo só entra no documento ao confirmar o nome e não pode repetir um nome existente; o modal é o mesmo do Rename.
- Modais: Add/Edit Entry (IP, hostnames, grupo escolhido numa lista), Rename, Import, Restore, Diff/Apply, Confirm Delete/Reload/Close, Conflito externo. O diff é lado a lado e destaca as linhas alteradas.
- Erros e o resultado do Apply aparecem numa faixa no rodapé; na tela de erro a faixa aparece sob a mensagem.
- Busca filtra entradas; grupo que casa mostra todas; grupo sem matches some; busca sem resultado avisa e oferece Clear search.
- Toggle de grupo: se todas habilitadas, desabilita todas; senão habilita todas. O mesmo vale para o bloco inteiro no rodapé.
- Enter envia o formulário; Esc fecha o modal, nunca durante a escrita; Ctrl+S abre o diff do Apply.
- Fechar com alterações não salvas pede confirmação.

## Empacotamento

- `cargo build --release` gera a GUI e o helper; o app instala o helper root-owned via `pkexec` e mostra o comando equivalente para copiar.
- `packaging/` traz `.desktop` e ícone para quem quiser instalar a GUI no menu.
- Desenvolvimento: `cargo run --release`; o helper é instalado uma vez.
- Sem Flatpak (o sandbox não escreve no `/etc` do host sem escape). AppImage só no futuro, com helper root-owned.

## Testes e aceitação

- Parser round-trip com fixtures: bloco no fim/topo, sem bloco, múltiplos/malformados, grupos vazios, disabled, comentários de linha, CRLF, IPv6, tabs e sem newline final.
- Importação: move grupos, preserva sistema/comentários, diff/backup.
- Helper: hash errado, conteúdo inválido, marcadores duplicados, modo preservado e atomicidade em teste unitário; dono/modo e escrita real em integração manual como root.
- UI: smoke manual em Wayland/GNOME: CRUD, toggle, busca, import, apply, cancelar pkexec, restore e dirty close.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo build --release`.

## Futuro

Perfis de ambiente; export/import de fragmentos; tema/atalhos; acessibilidade quando o Iced suportar; AppImage se portabilidade for necessária.

## Decisões registradas

`GLOSSARY.md` e ADRs `0001`–`0005`.
