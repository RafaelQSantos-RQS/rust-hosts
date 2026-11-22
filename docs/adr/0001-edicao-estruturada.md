---
status: accepted
---

# Edição estruturada preservando o arquivo

O app gerencia o `/etc/hosts` como uma lista estruturada de entradas, com CRUD e toggle, em vez de oferecer um editor de texto cru. Linhas que não são entradas — comentários, linhas em branco e conteúdo não gerenciado — são preservadas. O motivo é reduzir o risco de corromper um arquivo crítico do sistema e tornar a troca de ambientes uma ação de um clique; a flexibilidade do editor cru não compensa esse risco no MVP.

A fronteira de propriedade é o bloco marcado definido no ADR-0004; o app não altera o restante do arquivo.
