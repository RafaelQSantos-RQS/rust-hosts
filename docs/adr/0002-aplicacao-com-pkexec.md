---
status: accepted
---

# Aplicação direta com elevação via pkexec

O app aplica as mudanças diretamente no `/etc/hosts`, mediante preview do diff, elevação via `pkexec` e backup automático, em vez de apenas gerar o texto para cópia manual ou rodar o app inteiro como root. A elevação fica restrita a um helper root-owned instalado em `/usr/local/libexec/rust-hosts-helper`; o binário da GUI em `~/.local/bin` nunca roda como root. O helper valida o conteúdo e o hash do arquivo atual, preserva dono/permissões e faz escrita atômica. O motivo é manter o app rodando sem privilégios e ainda entregar a ação principal sem fricção.
