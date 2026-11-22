# Rust Hosts

Ferramenta pessoal de Linux para gerenciar um bloco do arquivo de hosts do sistema.

## Language

**Arquivo de hosts** (Hosts file):
O arquivo do sistema que mapeia endereços IP para nomes de host; no Linux, `/etc/hosts`.
_Avoid_: arquivo de DNS, hosts file

**Documento de hosts** (Hosts document):
A visão do arquivo de hosts no app: o bloco gerenciado editável e as linhas não gerenciadas somente leitura.
_Avoid_: modelo, buffer

**Bloco gerenciado** (Managed block):
A região do arquivo de hosts delimitada pelos marcadores do app; é a única região que o app altera.
_Avoid_: seção do app, área editável

**Marcador do bloco** (Block marker):
As linhas `# BEGIN rust-hosts` e `# END rust-hosts` que delimitam o bloco gerenciado.
_Avoid_: tag, delimitador

**Grupo** (Group):
Um conjunto nomeado de entradas dentro do bloco gerenciado, delimitado por um comentário de cabeçalho; pode estar vazio.
_Avoid_: seção, perfil, bloco

**Entrada** (Entry):
Uma linha do arquivo de hosts que associa um endereço IP a um ou mais nomes de host.
_Avoid_: registro, item, linha

**Entrada habilitada** (Enabled entry):
Uma entrada ativa dentro do bloco gerenciado.
_Avoid_: entrada ligada, entrada ativa

**Entrada desabilitada** (Disabled entry):
Uma entrada comentada dentro do bloco gerenciado; não está ativa no arquivo de hosts.
_Avoid_: entrada comentada, entrada inativa

**Linha não gerenciada** (Unmanaged line):
Qualquer linha fora do bloco gerenciado; o app a preserva como está.
_Avoid_: linha ignorada, lixo

**Importar** (Import):
Trazer entradas existentes fora do bloco para dentro dele, removendo-as do local original.
_Avoid_: migrar, copiar

**Aplicar** (Apply):
Gravar o bloco gerenciado no arquivo de hosts do sistema, após confirmação do diff e elevação de privilégio.
_Avoid_: salvar, sincronizar, escrever

**Backup** (Backup):
Uma cópia do arquivo de hosts feita antes de uma aplicação, destinada a restauração.
_Avoid_: cópia de segurança, snapshot

**Restaurar** (Restore):
Substituir o arquivo de hosts atual por um backup, após confirmação do diff e elevação de privilégio.
_Avoid_: reverter, desfazer
