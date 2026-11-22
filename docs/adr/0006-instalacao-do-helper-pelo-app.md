---
status: accepted
---

# Instalação do helper pelo próprio app

O app instala o helper root-owned com `pkexec /usr/bin/install -Dm755 -o root -g root <binário ao lado da GUI> /usr/local/libexec/rust-hosts-helper`, em vez de depender de um script externo. A oferta aparece só quando o helper falta ou responde a um `--protocol` diferente do esperado; nesse estado o Apply fica desabilitado e o app também mostra o comando equivalente para copiar e instalar à mão.

O motivo é que o helper é a única parte que exige privilégio e não pode vir de dentro do AppImage, que é montado a partir de um arquivo gravável pelo usuário; levar o passo para dentro do app elimina o "descubra sozinho qual linha de `sudo` rodar". O custo é uma segunda operação privilegiada além da aplicação do bloco, autorizada explicitamente no diálogo do polkit, e a confiança no binário que acompanha a GUI: quem puder escrever nessa pasta escolhe o que o root executa — o mesmo risco que um script de instalação teria, agora visível no app.

## Considered Options

- **Script externo (`install.sh`)**: auditável e simples, mas fica fora do app e precisa ser descoberto e executado à mão.
- **Helper embutido no AppImage**: o AppImage é montado via FUSE a partir de um arquivo do usuário e não serve como origem confiável para root.
- **Pacote `.rpm`/`.deb`**: é a ferramenta certa para arquivo de sistema, mas adia a distribuição e não cobre quem roda com `cargo run`.
