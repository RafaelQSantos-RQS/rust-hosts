---
status: accepted
---

# Bloco marcado como fronteira de propriedade

O app só altera o conteúdo entre os marcadores `# BEGIN rust-hosts` e `# END rust-hosts`; todo o resto do `/etc/hosts` é somente leitura. Entradas existentes fora do bloco não são gerenciadas até serem importadas. O motivo é reduzir o risco de sobrescrever configurações do sistema e tornar o escopo do app explícito; o custo é que o primeiro uso exige importar as entradas que o usuário quer trazer para o bloco ou criar os grupos à mão dentro do bloco novo.

## Considered Options

- **Arquivo inteiro**: mais simples, mas o app poderia alterar qualquer linha do sistema.
- **Só grupos gerenciados**: reduz o risco sem marcadores, mas a fronteira fica implícita e difícil de auditar.
- **Sidecar**: mantém o hosts file limpo, mas cria duas fontes de verdade.
