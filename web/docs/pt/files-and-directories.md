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
| A pasta temporária | Saídas longas de ferramentas e o prompt em edição. | Só o {{NAME}}. |

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
| `prompts/`, `styles/` e os arquivos embutidos em `agents/`, `skills/`, `hooks/`, `tools/` | Cópias dos prompts, estilos, agentes, skills, hooks e ferramentas embutidos. | abaixo |
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
| `users/<usuário>/settings.json` | Suas [preferências guardadas do chat](./settings): modelo, modo, esforço, estilo, histórico de prompts, última conversa por pasta. No máximo 256 KB; um arquivo que não é JSON válido é movido para `settings.json.broken`. |
| `users/<usuário>/sessions.json` | A lista das suas conversas. |
| `providers/<usuário>/providers.json` | Seus provedores conectados e as contas deles. |
| `memory/<usuário>/<projeto>.json` | Suas memórias de cada projeto. |
| `code-map/<projeto>.json` | O índice do mapa de código de cada projeto (recurso `experimental-code-map`). |
| `console/` | Uma cópia instalada da página do console de trace, se existir. |

Na **pasta de dados locais**:

| Caminho | O que é |
|---|---|
| `native-sessions/` | Conversas: um `<sessão>.jsonl` para cada uma, além de `checkpoints-<sessão>.json` (para o `/rewind`) e `worktree-<sessão>.json`. |
| `secrets.json` | Chaves de API e tokens, cifrados. Legível só por você. |
| `secrets.key` | A chave que abre o `secrets.json`. Legível só por você. |
| `logs/{{COMMAND}}.log` | O log (nível definido por `{{ENV}}_LOG`). Passando de 5 MB, é renomeado para `{{COMMAND}}.log.1` e um novo começa. |
| `lsp/` | Servidores de linguagem que o {{NAME}} instalou no primeiro uso. |

`<usuário>` é `local` para você na sua própria máquina, e `<projeto>` é o caminho do projeto com todo caractere que não seja letra ou dígito trocado por `-`.

> [!CAUTION]
> `/factory-reset` apaga a pasta de dados, a pasta de dados locais e `~/{{DIR}}` (sessões, memória, logins e configurações) depois de perguntar a você, e então encerra. As pastas `{{DIR}}/` dos projetos não são tocadas.

## Arquivos temporários

Na pasta temporária do sistema (`TMPDIR` no macOS e no Linux):

| Caminho | O que é |
|---|---|
| `{{COMMAND}}-tool-results/<sessão>/` | Saídas de ferramentas longas demais para a conversa (mais de 30.000 caracteres) e saídas completas de comandos que foram filtradas, para que o agente as leia em partes. |
| `{{COMMAND}}-prompt-<pid>.md` | O prompt enquanto você o edita no seu editor (Ctrl+G); removido depois. |

## Arquivos que o {{NAME}} só lê

| Arquivo | Por quê |
|---|---|
| `.env`, `.env.*` na raiz do projeto | Com `redact-secrets` ligado, os valores deles são escondidos da saída das ferramentas. Veja [Sandbox e segurança](./sandbox-and-safety). |
| `.git` | Encontrar a raiz do repositório (instruções, worktrees, memória de agentes, mapa de código). |
