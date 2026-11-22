---
status: accepted
---

# Formato do bloco gerenciado

O bloco é autocontido: grupos são cabeçalhos de comentário e entradas desabilitadas são linhas de entrada comentadas; o app não mantém sidecar para o conteúdo gerenciado. O motivo é ter uma única fonte de verdade no `/etc/hosts`, legível e coberta pelos backups. O custo é que o parser depende dessas convenções; todo comentário que não é uma entrada desabilitada é um cabeçalho de grupo, grupos vazios persistem e um grupo só pode ser excluído quando estiver vazio.
