---
title: Arquivos e pastas
description: Todos os arquivos e pastas que o {{NAME}} lê ou grava, na sua pasta pessoal, nos seus projetos, na pasta de dados e na pasta temporária.
---

O {{NAME}} guarda coisas em quatro lugares:

| Lugar | O que guarda | Quem edita |
|---|---|---|
| `~/{{DIR}}/` | Suas configurações, instruções, skills, agentes, plugins e outras extensões, para todos os projetos. | Você (e alguns comandos). |
| `{{DIR}}/` em um projeto | Configurações, instruções, skills, agentes e outras extensões do projeto. Em geral vai para o repositório. | Você e seu time. |
| A pasta de dados | Sessões, memória, escolhas guardadas, credenciais, logs, caches. | Só o {{NAME}}. |
| A pasta temporária | Saídas longas de ferramentas, o prompt em edição e alguns arquivos de trabalho. | Só o {{NAME}}. |

## Sua pasta: `~/{{DIR}}`

| Caminho | O que é | Veja |
|---|---|---|
| `settings.json` | Suas configurações, para todos os projetos. Gravado por `{{COMMAND}} config` e `/features`. | [Configurações](./settings) |
| `settings.local.json` | Mais configurações suas, lidas logo depois de `settings.json`. | [Configurações](./settings) |
| `managed-settings.json` | Configurações lidas depois de todos os outros arquivos. | [Configurações](./settings) |
| `{{INSTRUCTIONS}}` | Suas instruções, adicionadas a toda conversa. | [Arquivos de instruções](./instructions-files) |
| `rules/` | Seus arquivos de regras (`*.md`). | [Arquivos de instruções](./instructions-files) |
| `mcp.json` | Seus servidores MCP (`mcpServers`). Gravado por `{{COMMAND}} mcp add` e `remove`. | [MCP](./mcp) |
| `mcp/` | Seus servidores MCP escritos em JavaScript. | [MCP](./mcp) |
| `skills/` | Suas skills, uma pasta cada com um `SKILL.md`. | [Skills](./skills) |
| `commands/` | Seus comandos personalizados (`*.md` ou `*.js`), executados como `/nome`. | [Skills](./skills) |
| `agents/` | Seus subagentes (`<nome>.md`). | [Agentes](./agents) |
| `agent-memory/` | Memória dos agentes com escopo de memória `user`. | [Agentes](./agents) |
| `hooks/` | Seus hooks em JavaScript (`*.js`). | [Hooks](./hooks) |
| `tools/` | Suas ferramentas em JavaScript (`*.js`). | [Skills](./skills) |
| `providers/` | Provedores que você declara em JavaScript. | [Modelos e provedores](./models-and-providers) |
| `output-styles/` | Seus estilos de resposta personalizados (`<nome>.md`). | [Estilos de resposta](./output-styles) |
| `plugins/` | Plugins instalados, uma pasta cada. `{{COMMAND}} plugin install <git-url>` clona aqui. | [Plugins](./plugins) |
| `prompts/`, `styles/`, `repl/`, `types.d.ts` e os arquivos embutidos em `agents/`, `skills/`, `hooks/`, `tools/` | Cópias dos prompts, estilos, agentes, skills, hooks e ferramentas embutidos, e os tipos que os scripts podem usar. | abaixo |
| `.content.json` | As impressões digitais dessas cópias. | abaixo |

### Cópias do conteúdo embutido

No primeiro uso, o {{NAME}} copia seus agentes, hooks, prompts, skills, estilos e ferramentas embutidos para `~/{{DIR}}` e registra a impressão digital de cada arquivo em `.content.json`. Quando você edita uma cópia, a sua versão passa a ser usada no lugar da embutida, e as atualizações seguintes a deixam em paz (uma cópia não editada é atualizada a cada nova versão). Apague um arquivo para receber o embutido de volta na próxima abertura.

### A pasta de um plugin

Cada plugin em `~/{{DIR}}/plugins/<nome>/` pode ter:

| Caminho | O que é |
|---|---|
| `{{PLUGIN_DIR}}/plugin.json` | O manifesto: o `name` e, opcionalmente, `mcpServers` e `lspServers`. |
| `.mcp.json` | Os servidores MCP do plugin (`mcpServers`), no lugar dos do manifesto. |
| `.lsp.json` | Os servidores de linguagem do plugin, no lugar dos do manifesto. |
| `hooks/hooks.json` | Os hooks do plugin. |
| `skills/`, `commands/`, `agents/` | As skills, comandos e agentes do plugin. |

## Pasta do projeto: `{{DIR}}/`

Configurações, regras, skills, agentes, comandos, hooks e estilos de resposta são procurados no `{{DIR}}/` da pasta atual **e de todas as pastas acima**, para que um monorepo possa compartilhá-los a partir da raiz. Quando duas pastas definem a mesma coisa, vale a mais próxima.

| Caminho | O que é | Vai para o repositório? | Veja |
|---|---|---|---|
| `{{DIR}}/settings.json` | As configurações do projeto, para o time todo. | Sim | [Configurações](./settings) |
| `{{DIR}}/settings.local.json` | Suas configurações privadas neste projeto. "Don't ask again", `/rules keep`, `{{COMMAND}} config --project` e `{{COMMAND}} mcp approve` gravam aqui. | Não | [Configurações](./settings) |
| `{{DIR}}/{{INSTRUCTIONS}}` | As instruções do projeto (alternativa ao `{{INSTRUCTIONS}}` na raiz). | Sim | [Arquivos de instruções](./instructions-files) |
| `{{DIR}}/rules/` | Arquivos de regras (`*.md`). | Sim | [Arquivos de instruções](./instructions-files) |
| `{{DIR}}/mcp.json` | Os servidores MCP do projeto. Vale o mais próximo subindo a árvore de pastas; seus servidores aguardam a sua aprovação. | Sim | [MCP](./mcp) |
| `{{DIR}}/mcp/` | Servidores MCP escritos em JavaScript (só a pasta atual). | Sim | [MCP](./mcp) |
| `{{DIR}}/skills/` | Skills do projeto. | Sim | [Skills](./skills) |
| `{{DIR}}/commands/` | Comandos do projeto. | Sim | [Skills](./skills) |
| `{{DIR}}/agents/` | Subagentes do projeto. | Sim | [Agentes](./agents) |
| `{{DIR}}/hooks/` | Hooks em JavaScript. | Sim | [Hooks](./hooks) |
| `{{DIR}}/tools/` | Ferramentas em JavaScript (só a pasta atual). | Sim | [Skills](./skills) |
| `{{DIR}}/providers/` | Provedores declarados pelo projeto (só a pasta atual); cada um aguarda até você adicionar o id dele a `enabledProviders`. | Sim | [Modelos e provedores](./models-and-providers) |
| `{{DIR}}/output-styles/` | Estilos de resposta personalizados. | Sim | [Estilos de resposta](./output-styles) |
| `{{DIR}}/verification.md` | Critérios contra os quais toda execução é julgada quando você não define nenhum (até 64 KB; vale o mais próximo, nunca o de `~/{{DIR}}`). | Sim | [Verificação e revisão](./verification-and-review) |
| `{{DIR}}/memory/` | Memórias do time, compartilhadas com `/memory share <id>`. | Sim | [Memória](./memory) |
| `{{DIR}}/evals/<nome>/case.json` | Casos de avaliação para `{{COMMAND}} eval`, com uma pasta `fixture/` opcional. | Sim | [Evals](./evals) |
| `{{DIR}}/agent-memory/` | Memória dos agentes com escopo `project` (na raiz do repositório). | Você decide | [Agentes](./agents) |
| `{{DIR}}/agent-memory-local/` | Memória dos agentes com escopo `local` (na raiz do repositório). | Não | [Agentes](./agents) |
| `{{DIR}}/worktrees/<id>/` | Worktrees do git de agentes que trabalham isolados (na raiz do repositório). | Não | [Branches e git](./branches-and-git) |

Arquivos de instruções fora do `{{DIR}}/`, lidos em cada pasta, da raiz do sistema de arquivos até a pasta atual:

| Arquivo | O que é |
|---|---|
| `{{INSTRUCTIONS}}` | As instruções do projeto. |
| `AGENTS.md` | Também é lido, para projetos compartilhados com outros agentes. |
| `{{LOCAL_INSTRUCTIONS}}` | Suas instruções privadas para esta pasta; deixe fora do controle de versão. |

> [!TIP]
> Adicione `{{DIR}}/settings.local.json`, `{{DIR}}/agent-memory-local/`, `{{DIR}}/worktrees/` e `{{LOCAL_INSTRUCTIONS}}` ao seu `.gitignore`.

## Pasta de dados

Os dados do próprio {{NAME}} ficam fora dos seus projetos, em uma pasta que depende do sistema:

| Sistema | Pasta de dados | Pasta de dados locais |
|---|---|---|
| macOS | `~/Library/Application Support/{{NAME}}` | a mesma pasta |
| Linux | `$XDG_CONFIG_HOME/{{COMMAND}}`, ou `~/.config/{{COMMAND}}` | a mesma pasta |
| Windows | `%APPDATA%\{{NAME}}` | `%LOCALAPPDATA%\{{NAME}}` |

Na **pasta de dados**:

| Caminho | O que é |
|---|---|
| `users/<usuário>/settings.json` | Suas [preferências guardadas do chat](./settings): modelo, modo, esforço, estilo, histórico de prompts, última conversa por pasta. No máximo 256 KB; um arquivo que não é JSON válido é movido para `settings.json.corrupt-<hora>`. |
| `users/<usuário>/trusted-folders.json` | As pastas em que você confiou (veja [Confiança na pasta](./sandbox-and-safety#confianca-na-pasta)). |
| `app.db` | Um banco de dados SQLite: a lista das suas conversas e as suas memórias de cada projeto. Legível só por você. |
| `app.db-wal`, `app.db-shm` | Arquivos de trabalho do SQLite para o `app.db`. Existem enquanto o {{NAME}} está aberto; deixe-os ao lado dele. |
| `users/<usuário>/sessions.json.imported` | A lista de conversas que uma versão anterior guardava, renomeada depois de importada para o `app.db`. Não é mais lida. |
| `memory/<usuário>/<projeto>.json.imported` | As memórias que uma versão anterior guardava de um projeto, renomeadas depois de importadas para o `app.db`. Não são mais lidas. |
| `providers/<usuário>/providers.json` | Seus provedores conectados e as contas deles. |
| `code-map/<projeto>.json` | O índice do mapa de código de cada projeto (recurso `experimental-code-map`). |
| `console/` | Uma cópia instalada da página do console de trace, se existir. |

Na **pasta de dados locais**:

| Caminho | O que é |
|---|---|
| `native-sessions/` | Conversas. Para cada uma: `<sessão>.jsonl` (a conversa em si, salva a cada passo, então ela sobrevive a uma falha ou a um terminal fechado), `<sessão>.trace.jsonl` (o que o `/trace` mostra; ao passar de 64 MB é renomeado para `<sessão>.trace.1.jsonl` e um novo começa, então só a parte mais recente e a anterior são mantidas), `checkpoints-<sessão>.json` (para o `/rewind`: os arquivos dos 50 prompts mais recentes, até 64 MB), `snapshot-<sessão>.json` (a data e o fuso horário local, por exemplo `America/Sao_Paulo, UTC-03:00`, o estado do git e os arquivos do topo com que a conversa começou, para o prompt dela não mudar) e `worktree-<sessão>.json`. Um arquivo `<sessão>.lock` existe só enquanto a conversa está respondendo. Apagar os arquivos de trace perde o trace, nunca a conversa. |
| `secrets.json` | Chaves de API e tokens, cifrados. Legível só por você. |
| `secrets.key` | Onde está a chave que abre o `secrets.json`. Por padrão ele contém a própria chave, em 64 dígitos hexadecimais. Só com `{{ENV}}_SECRETS=keychain` ele contém apenas a palavra `keychain`, e a chave fica no chaveiro do sistema (Keychain no macOS, Secret Service no Linux, Gerenciador de Credenciais no Windows). Legível só por você. Não o apague: sem ele, as chaves do `secrets.json` não podem ser abertas. |
| `logs/{{COMMAND}}.log` | O log (nível definido por `{{ENV}}_LOG`). Passando de 5 MB, é renomeado para `{{COMMAND}}.log.1` e um novo começa. |
| `lsp/` | Servidores de linguagem que o {{NAME}} instalou no primeiro uso. |

`<usuário>` é `local` para você na sua própria máquina, e `<projeto>` é o caminho do projeto com todo caractere que não seja letra ou dígito trocado por `-`.

Várias cópias do {{NAME}} abertas ao mesmo tempo compartilham o `app.db` com segurança: cada alteração é uma transação, então nenhuma perde o que a outra gravou. Para copiar ou fazer backup, feche o {{NAME}} antes, ou copie `app.db`, `app.db-wal` e `app.db-shm` juntos.

Se o `app.db` não puder ser aberto (foi gravado por uma versão mais nova, outro programa o está segurando, ou o arquivo não pode ser lido), o {{NAME}} inicia mesmo assim e avisa uma vez: `The session list and memory could not be opened (<motivo>); they will not be kept after this session.` O chat mostra isso como um aviso; com `--print` vai para a saída de erro. Até a causa ser corrigida, a lista de conversas e o que for lembrado duram só até o {{NAME}} fechar. O arquivo fica como está. As conversas em si, em `native-sessions/`, continuam sendo salvas.

### Vindo de uma versão anterior

Versões anteriores guardavam a lista de conversas em `users/<usuário>/sessions.json` e as memórias de cada projeto em `memory/<usuário>/<projeto>.json`. Na primeira vez que uma versão com `app.db` é aberta, ela importa esses arquivos e renomeia cada um acrescentando `.imported`. Nada é apagado. Um arquivo que não é JSON válido é renomeado com `.corrupt-<hora>` e o restante é importado.

> [!WARNING]
> Voltando para uma versão anterior depois da importação: a versão anterior procura `sessions.json` e `<projeto>.json`, encontra só os arquivos `.imported` e mostra a lista de conversas vazia e a memória vazia. Seus dados não foram perdidos. Feche o {{NAME}} e renomeie os arquivos de volta (tire o `.imported` de cada nome): a versão anterior mostra tudo como estava no momento da importação. As conversas listadas e as memórias salvas depois da importação existem só no `app.db`, que a versão anterior não lê. As conversas em si, em `native-sessions/`, não são afetadas em nenhum dos casos.

> [!CAUTION]
> `/factory-reset` apaga a pasta de dados, a pasta de dados locais e `~/{{DIR}}` (sessões, memória, logins e configurações) depois de perguntar a você, e então encerra. As pastas `{{DIR}}/` dos projetos não são tocadas.

## Arquivos temporários

Na pasta temporária do sistema (`TMPDIR` no macOS e no Linux):

| Caminho | O que é |
|---|---|
| `{{COMMAND}}-tool-results/<sessão>/` | Saídas de ferramentas longas demais para a conversa (mais de 30.000 caracteres), saídas completas de comandos Bash que passaram do `bashOutputLimit` e saídas completas de comandos que foram filtradas, para que o agente as leia em partes. A pasta de uma conversa é removida depois de 7 dias sem nada ser gravado nela. |
| `{{COMMAND}}-prompt-<aleatório>.md` | O prompt enquanto você o edita no seu editor (Ctrl+G); só você pode lê-lo, e ele é removido depois. |
| `{{COMMAND}}-claims/<hash>.json` | Qual sessão editou por último cada arquivo, por projeto, para o aviso do `experimental-session-claims`. |
| `{{COMMAND}}-memory/<execução>.json` | As mudanças de memória que uma execução pede, antes de o {{NAME}} aplicá-las. |

## Arquivos que o {{NAME}} só lê

| Arquivo | Por quê |
|---|---|
| `.env`, `.env.*` na raiz do projeto | Com `redact-secrets` ligado, os valores deles são escondidos da saída das ferramentas. Veja [Sandbox e segurança](./sandbox-and-safety). |
| `.git` | Encontrar a raiz do repositório (instruções, worktrees, memória de agentes, mapa de código). |
