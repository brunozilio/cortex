---
title: Hooks
description: Rode seus próprios comandos, scripts, requisições web ou verificações por modelo em pontos fixos de uma execução para bloquear, permitir, reescrever ou acrescentar contexto ao que o agente faz.
---

Hooks permitem encaixar sua própria lógica no ciclo do agente do {{NAME}}. Um hook roda num ponto fixo (antes de uma chamada de ferramenta, quando um prompt é enviado, quando o agente quer parar, antes de uma chamada ao modelo…), recebe uma descrição em JSON do que está acontecendo e pode:

- **bloquear** a ação, com um motivo que o agente vê;
- **permitir** ou **perguntar** sobre uma chamada de ferramenta, passando por cima do pedido de permissão normal;
- **reescrever** a entrada da ferramenta, a saída da ferramenta, o prompt, o modelo, o prompt de sistema ou a resposta;
- **acrescentar contexto** que o agente lê.

Hooks são determinísticos: ao contrário das instruções, o agente não tem como esquecê-los ou ignorá-los.

## Começo rápido

Recuse todo `rm -rf` que o agente tentar rodar. Salve isto como `~/{{DIR}}/hooks/no-rm-rf.js`:

```javascript
export const event = "PreToolUse";
export const matcher = "^Bash$";

export default function (input) {
  if (input.tool_input.command.includes("rm -rf")) {
    return { decision: "block", reason: "rm -rf não é permitido aqui; apague os arquivos um a um." };
  }
}
```

O próximo prompt já usa o hook. Rode `/hooks` no chat para ver todos os hooks que uma execução nesta pasta dispara.

O mesmo hook como comando de shell em `~/{{DIR}}/settings.json`:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "^Bash$",
        "hooks": [
          { "type": "command", "command": "~/bin/no-rm-rf.sh", "timeout": 10 }
        ]
      }
    ]
  }
}
```

```bash
#!/bin/sh
if jq -e '.tool_input.command | contains("rm -rf")' >/dev/null; then
  echo "rm -rf não é permitido aqui; apague os arquivos um a um." >&2
  exit 2
fi
```

O código de saída `2` bloqueia a chamada, e o texto no stderr é o motivo que o agente vê.

## De onde vêm os hooks

Todas estas fontes se somam; nenhuma substitui a outra.

| Fonte | Vale para |
|---|---|
| `~/{{DIR}}/settings.json`, em `"hooks"` | Você, em todos os projetos |
| `{{DIR}}/settings.json` e `{{DIR}}/settings.local.json`, em `"hooks"`, na pasta atual e em cada pasta acima dela até a raiz do projeto | Aquele projeto, depois que você confia na pasta dele (veja [abaixo](#hooks-do-projeto-e-confianca-na-pasta)) |
| `~/{{DIR}}/managed-settings.json`, em `"hooks"` | Você, em todos os projetos |
| `~/{{DIR}}/hooks/*.js` | Você, em todos os projetos |
| `{{DIR}}/hooks/*.js` na pasta atual e em cada pasta acima dela até a raiz do projeto | Aquele projeto, depois que você confia na pasta dele (veja [abaixo](#hooks-do-projeto-e-confianca-na-pasta)) |
| `hooks/hooks.json` num plugin instalado | Onde o plugin estiver instalado (veja [Plugins](./plugins)) |
| `hooks` no frontmatter de um agente próprio | Só enquanto aquele agente roda (veja [Agentes](./agents)) |

Os hooks são lidos de novo no início de cada prompt, então uma mudança vale a partir da sua próxima mensagem.

### Hooks do projeto e confiança na pasta

Um hook roda comandos no seu computador com as suas permissões, por isso o {{NAME}} só roda os hooks de um projeto depois que você confia na pasta dele. Até lá, a chave `hooks` do `{{DIR}}/settings.json` e do `{{DIR}}/settings.local.json` do projeto e os scripts em `{{DIR}}/hooks/` são **ignorados**. O resto desses arquivos que só restringe continua valendo: as regras de negar e de perguntar, por exemplo.

Quando hooks são ignorados, o {{NAME}} avisa:

- o chat pergunta se você confia na pasta antes do seu primeiro prompt e lista os hooks que seriam habilitados;
- `/hooks` e `/permissions` terminam com uma nota listando o que foi ignorado, com o motivo `folder not trusted (/trust)`;
- com `-p`, uma linha no stderr diz quantos itens do projeto foram ignorados; passe `--trust` para rodá-los naquela chamada.

Confie na pasta pela pergunta inicial ou com `/trust yes`. Veja em [Confiança na pasta](./sandbox-and-safety#confianca-na-pasta) o que mais ela cobre.

Estes hooks sempre rodam:

- os seus, em `~/{{DIR}}/settings.json`, `~/{{DIR}}/managed-settings.json` e `~/{{DIR}}/hooks/`;
- os hooks de plugins instalados.

Hooks em uma pasta `{{DIR}}/` acima do projeto (acima da pasta que contém o `.git`) nunca rodam, não importa em que você confiou.

> [!WARNING]
> Depois que a pasta é confiável, um `{{DIR}}/settings.json` versionado ou a pasta `{{DIR}}/hooks/` rodam comandos no seu computador com as suas permissões. Leia esses arquivos antes de confiar em um repositório, e de novo depois de puxar alterações neles.

> [!NOTE]
> Hooks nas pastas de outra ferramenta, como `.claude/settings.json`, não são lidos.

### Conferir os hooks

`/hooks` lista cada hook que uma execução aqui dispara, com o evento, o matcher (`*` para todos) e o que ele roda: o comando, a URL, ou `prompt:`/`agent:` seguido do começo do prompt. Sem hooks, aparece *No hooks. Add them under "hooks" in {{DIR}}/settings.json.* Quando a pasta não é confiável, a lista termina com uma nota listando o que foi ignorado, com o motivo `folder not trusted (/trust)` (veja [Hooks do projeto e confiança na pasta](#hooks-do-projeto-e-confianca-na-pasta)).

## Formato da configuração

Num arquivo de configurações, `"hooks"` liga o nome de um evento a uma lista de **grupos**. Cada grupo tem um `matcher` opcional e uma lista de **handlers**:

```json
{
  "hooks": {
    "<Evento>": [
      {
        "matcher": "<regex>",
        "hooks": [
          { "type": "command", "command": "…" }
        ]
      }
    ]
  }
}
```

Um grupo aceita só `matcher` e `hooks`. Um handler aceita:

| Campo | Usado por | Significado |
|---|---|---|
| `type` | todos | `command`, `script`, `http`, `prompt` ou `agent` |
| `command` | `command`, `script` | O comando de shell, ou o caminho de um arquivo `.js` |
| `url` | `http` | Um endereço `http://` ou `https://`, sem usuário nem senha |
| `headers` | `http` | Cabeçalhos da requisição; os valores podem usar `$VAR` ou `${VAR}` |
| `allowed_env_vars` | `http` | As variáveis de ambiente que `headers` pode ler |
| `prompt` | `prompt`, `agent` | O que o modelo deve verificar |
| `model` | `prompt`, `agent` | O modelo que verifica (por padrão, o da execução) |
| `timeout` | todos | Segundos, de 1 a 600; 600 por padrão |
| `status_message` | todos | Aceito; o chat ainda não o mostra |

Um arquivo de configurações com evento desconhecido, tipo de handler desconhecido, matcher que não é expressão regular válida, `command`/`url`/`prompt` faltando ou timeout fora de 1–600 é recusado com uma mensagem que cita o evento e o número do handler, por exemplo `SessionStart hook #2: a prompt hook cannot run on SessionStart; use a command hook`. A execução não começa até o arquivo ser corrigido. Já o `hooks.json` quebrado de um plugin é deixado de fora, com um aviso no log.

### Matchers

`matcher` é uma expressão regular testada contra o assunto do evento (veja a tabela abaixo). Um matcher vazio, ausente ou `*` casa com tudo. O casamento não é ancorado: `Edit` também casa com `NotebookEdit`, então escreva `^Edit$` para uma ferramenta, ou `^(Edit|Write)$` para várias.

| Evento | O matcher é testado contra |
|---|---|
| `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `PermissionRequest` | O nome da ferramenta: `Bash`, `Edit`, `mcp__jira__search`… |
| `Stop`, `SubagentStop` | O nome do agente: `main` para a conversa, senão o do subagente (`general-purpose`, `Explore`, um nome próprio…) |
| `SubagentStart` | O tipo do subagente |
| `SessionStart`, `RunStart` | `startup` ou `resume` |
| `PreCompact`, `PostCompact` | `manual` ou `auto` |
| `Notification` | O tipo da notificação |
| `ModelRequest`, `ModelResponse`, `ModelError` | `main` ou `sub-agent` |
| `TeammateIdle`, `TaskCreated`, `TaskCompleted` | Um texto vazio: deixe o matcher vazio |
| `UserPromptSubmit`, `SessionEnd`, `RunEnd`, `Brief` | Uma string vazia: deixe o matcher vazio |

Os grupos rodam em ordem; dentro de um grupo, os handlers rodam em ordem. Assim que um bloqueia, os demais são pulados.

## Eventos

| Evento | Quando dispara | Pode bloquear? |
|---|---|---|
| [`Brief`](#brief) | Quando skills, agentes, regras e arquivos de instruções são encontrados: no primeiro prompt de uma sessão e depois que um desses arquivos mudou | Sim: a execução não começa |
| [`SessionStart`](#sessionstart) | Uma vez por sessão, no primeiro prompt dela neste processo | Sim: a execução não começa |
| [`RunStart`](#runstart) | No início de cada execução (cada prompt que você envia) | Sim: a execução não começa |
| [`UserPromptSubmit`](#userpromptsubmit) | Quando o prompt é enviado, antes de o modelo vê-lo | Sim: a execução não começa |
| [`ModelRequest`](#modelrequest) | Antes de cada chamada ao modelo | Sim: a execução falha |
| [`ModelResponse`](#modelresponse) | Quando chega uma resposta, antes de ser mostrada ou usada | Sim: o modelo responde de novo |
| [`ModelError`](#modelerror) | Quando uma chamada ao modelo falha | Sim: a execução falha com o erro |
| [`PreToolUse`](#pretooluse) | Antes de uma ferramenta rodar | Sim: a chamada é recusada |
| [`PermissionRequest`](#permissionrequest) | Quando o agente iria pedir sua permissão | Sim: a chamada é recusada |
| [`PostToolUse`](#posttooluse) | Depois que uma ferramenta tem sucesso | Dá retorno: o resultado é marcado como falho |
| [`PostToolUseFailure`](#posttoolusefailure) | Depois que uma ferramenta falha | Dá retorno |
| [`Notification`](#notification) | Quando o {{NAME}} precisa da sua atenção | Não |
| [`SubagentStart`](#subagentstart) | Quando um subagente começa | Sim: o subagente falha |
| [`SubagentStop`](#subagentstop) | Quando um subagente quer terminar | Sim: ele continua trabalhando |
| [`Stop`](#stop) | Quando o agente principal quer terminar | Sim: ele continua trabalhando |
| [`TeammateIdle`](#teammateidle) | Quando um teammate vai ficar ocioso, depois do `Stop` | Sim: ele continua trabalhando |
| [`TaskCreated`](#taskcreated) | Antes de uma tarefa entrar na lista | Sim: a tarefa não é criada |
| [`TaskCompleted`](#taskcompleted) | Antes de uma tarefa ser marcada como concluída | Sim: ela continua aberta |
| [`PreCompact`](#precompact) | Antes de a conversa ser resumida | Sim: a compactação é cancelada |
| [`PostCompact`](#postcompact) | Depois de a conversa ser resumida | Não |
| [`RunEnd`](#runend) | No fim de cada execução | Não |
| [`SessionEnd`](#sessionend) | Uma vez por sessão, quando o {{NAME}} a deixa | Não |

## Entrada do hook

Um hook recebe um objeto JSON. Todo evento inclui:

| Campo | Valor |
|---|---|
| `hook_event_name` | O evento, como `PreToolUse` |
| `session_id` | O id da conversa |
| `agent_type` | O agente em execução: `main` para a conversa, senão o nome do subagente |
| `cwd` | A pasta do projeto |

Cada evento acrescenta campos próprios, listados abaixo. Um hook `command` lê o objeto pelo stdin; um hook HTTP o recebe como corpo da requisição; um hook `script` o recebe como primeiro argumento.

## Saída do hook

### Hooks de comando: códigos de saída

| Código de saída | Efeito |
|---|---|
| `0` | Sucesso. O stdout é lido como JSON (veja abaixo). Em `SessionStart`, `RunStart` e `UserPromptSubmit`, um stdout que não é JSON entra como contexto para o agente. |
| `2` | Bloqueia. O stderr é o motivo. Os demais hooks do evento são pulados. |
| Qualquer outro | O hook falhou. `Hook failed: <stderr>` é acrescentado ao contexto que o agente vê, e o próximo hook roda. Em `Brief`, `ModelRequest`, `ModelResponse` e `ModelError` a falha só vai para o log. |

Um hook que estoura o tempo ou é cancelado é um erro, que [cada evento trata do seu jeito](#erros-e-timeouts).

### Saída em JSON

Um hook que sai com `0` (ou um hook HTTP que responde `2xx`, ou um script que retorna um valor) pode imprimir um objeto JSON:

```json
{
  "decision": "block",
  "reason": "Por que foi bloqueado",
  "continue": false,
  "stopReason": "Por que foi parado",
  "systemMessage": "Texto acrescentado ao contexto do agente",
  "hookSpecificOutput": {
    "additionalContext": "Mais texto para o contexto do agente",
    "permissionDecision": "allow",
    "permissionDecisionReason": "Por quê",
    "updatedInput": {},
    "updatedToolOutput": "…",
    "decision": { "behavior": "allow", "updatedInput": {} },
    "updatedPrompt": "…",
    "model": "provedor:modelo",
    "effort": "high",
    "system": "…",
    "history": [],
    "text": "…"
  }
}
```

| Campo | Efeito |
|---|---|
| `decision: "block"`, `continue: false` ou `hookSpecificOutput.permissionDecision: "deny"` | Bloqueia. O motivo é `reason`, senão `stopReason`, senão `permissionDecisionReason`. |
| `systemMessage`, `hookSpecificOutput.additionalContext` | Acrescentados ao contexto que o agente vê (onde, depende do evento). |
| `hookSpecificOutput.permissionDecision` | `PreToolUse`: `allow`, `deny` ou `ask`. |
| `hookSpecificOutput.decision.behavior` | `PermissionRequest`: `allow`, `deny` ou `ask`. |
| `hookSpecificOutput.updatedInput` | `PreToolUse`: substitui a entrada da ferramenta (precisa ser um objeto). |
| `hookSpecificOutput.decision.updatedInput` | `PermissionRequest`: propõe uma nova entrada. |
| `hookSpecificOutput.updatedToolOutput` | `PostToolUse`: substitui o resultado da ferramenta. |
| `hookSpecificOutput.updatedPrompt` | `UserPromptSubmit`: substitui o prompt. |
| `hookSpecificOutput.model` | `ModelRequest`, `ModelError`: o modelo a usar. |
| `hookSpecificOutput.effort` | `ModelRequest`: `low`, `medium`, `high`, `xhigh` ou `max`. |
| `hookSpecificOutput.system`, `hookSpecificOutput.history` | `ModelRequest`: substituem o prompt de sistema ou as mensagens. |
| `hookSpecificOutput.text` | `ModelResponse`: substitui o texto da resposta. |

Uma decisão de permissão diferente de `allow`, `deny` ou `ask` é um erro.

### Erros e timeouts

Quando um hook não consegue rodar, estoura o tempo ou é cancelado:

| Evento | O que acontece |
|---|---|
| `PreToolUse` | A chamada é recusada: `Hook blocked <ferramenta>: hook cancelled or timed out` |
| `PostToolUse`, `PostToolUseFailure` | O resultado é marcado como falho, com `Hook failed: …` acrescentado |
| `PermissionRequest` | A chamada falha com o erro |
| `SessionStart`, `RunStart`, `UserPromptSubmit`, `Stop`, `SubagentStop`, `TeammateIdle`, `Brief` | A execução para com o erro |
| `TaskCreated`, `TaskCompleted` | A ferramenta de tarefas falha com o erro, e a lista fica como estava |
| `SubagentStart` | O subagente falha |
| `PreCompact`, `PostCompact` | A compactação falha |
| `ModelRequest`, `ModelResponse`, `ModelError` | Vai para o log; a chamada segue como se não houvesse hook |
| `Notification`, `RunEnd`, `SessionEnd` | Vai para o log e é ignorado |

## Tipos de handler

### `command`

Roda um comando de shell (`/bin/sh -c` no macOS e no Linux). O comando:

- roda na pasta do projeto;
- recebe o JSON de entrada pelo stdin;
- recebe `{{ENV}}_PROJECT_DIR` com a pasta do projeto;
- fica confinado como a ferramenta `Bash` do agente quando o sandbox está ligado (veja [Sandbox e segurança](./sandbox-and-safety));
- tem stdout e stderr lidos até 64 KiB cada.

```json
{ "type": "command", "command": "\"${{{ENV}}_PROJECT_DIR}\"/scripts/check.sh", "timeout": 30 }
```

### `script`

Roda um arquivo JavaScript no runtime de scripts embutido do {{NAME}}, sem abrir um processo. `command` deve ser o caminho de um arquivo `.js`; use um caminho absoluto. O export padrão do arquivo é chamado com a entrada e um objeto de contexto, e o valor retornado é a saída do hook:

```javascript
export default function (input, ctx) {
  if (input.tool_input.command.startsWith("git push")) {
    return { decision: "block", reason: "Faça o push no seu próprio terminal." };
  }
}
```

| Valor retornado | Tratado como |
|---|---|
| Nada (`undefined` ou `null`) | Saída `0`, sem saída |
| Uma string | Saída `0`, essa string como stdout |
| Um objeto | Saída `0`, o objeto como saída JSON |
| Um erro lançado | Um hook que falhou (como uma saída diferente de zero), com o erro como stderr |

O contexto oferece `ctx.cwd`, `ctx.session` e `ctx.agent`, `ctx.log`, além de funções de arquivo, processo e rede (`ctx.readFile`, `ctx.writeFile`, `ctx.exec`, `ctx.fetch`). Um script lista as funções que chama em `export const capabilities = [...]`; uma chamada não listada falha. Cada chamada listada é julgada pelas suas permissões como a ferramenta correspondente (`ctx.exec` como `Bash`), perguntando a você quando elas mandam, e a configuração `scripts` continua sendo o limite de fora; veja "Capacidades" em [Sandbox e segurança](./sandbox-and-safety). Quando o evento roda junto com o modelo (eventos de ferramenta, de prompt, de parada e de compactação), o script também pode chamar `ctx.llm(…)` para perguntar ao modelo e `ctx.tool(nome, entrada)` para usar uma ferramenta somente leitura dentro do projeto.

As definições de tipo da entrada e da saída de cada evento estão no `content/types.d.ts` do {{NAME}} (`Hook<"PreToolUse">` e assim por diante).

### Pastas de scripts de hook

Em vez de listar um script nas configurações, coloque-o numa pasta de hooks (`~/{{DIR}}/hooks/` ou `{{DIR}}/hooks/`). O arquivo indica o evento e, se quiser, um matcher e um timeout, por meio de exports:

```javascript
export const event = "PostToolUse";
export const matcher = "^(Edit|Write)$";
export const timeout = 30;
export const capabilities = ["exec"];

export default (input, ctx) => {
  const result = ctx.exec(`npx prettier --check ${JSON.stringify(input.tool_input.file_path)}`);
  if (result.code !== 0) {
    return { hookSpecificOutput: { additionalContext: "Rode o prettier nesse arquivo." } };
  }
};
```

| Export | Significado |
|---|---|
| `event` | Um nome de evento, ou uma lista deles. Obrigatório. |
| `matcher` | Como um matcher das configurações. Vazio por padrão. |
| `timeout` | Segundos, de 1 a 600. 60 por padrão. |
| `capabilities` | As funções do host que o hook chama: `readFile`, `writeFile`, `exec`, `fetch`. Vazia por padrão. |
| `default` | A função do hook. Um arquivo sem export padrão, ou com uma função vazia, é ignorado. |

O {{NAME}} vem com um modelo vazio para cada evento. Um arquivo em `~/{{DIR}}/hooks/` com o mesmo nome de um modelo (por exemplo `PreToolUse.js`) toma o lugar dele.

> [!NOTE]
> Um script na pasta `{{DIR}}/hooks/` de um projeto não pode ler nem gravar arquivos, iniciar processos ou acessar a rede (`ctx.readFile`, `ctx.exec`, `ctx.fetch`…) enquanto um dos seus próprios arquivos de configuração não liberar. Os scripts em `~/{{DIR}}/hooks/` podem. Veja [`scripts`](./settings#scripts).

> [!WARNING]
> Mudança incompatível: um script de hook que chama `ctx.readFile`, `ctx.writeFile`, `ctx.exec` ou `ctx.fetch` agora precisa listá-la em `export const capabilities`, e a chamada é julgada pelas suas permissões. Um hook que roda onde ninguém pode ser consultado (uma execução `-p`) precisa de uma regra allow para uma chamada que perguntaria, por exemplo `Bash(cargo clippy:*)`. Veja o exemplo de migração em "Capacidades", em [Sandbox e segurança](./sandbox-and-safety).

### `http`

Faz um POST do JSON de entrada para uma URL. O corpo de uma resposta `2xx` é lido como o stdout de um comando (até 64 KiB, UTF-8). Qualquer outro status é um hook que falhou. Um hook HTTP bloqueia respondendo um JSON como `{"decision": "block", "reason": "…"}`.

```json
{
  "type": "http",
  "url": "https://hooks.example.com/agent",
  "headers": { "Authorization": "Bearer ${HOOK_TOKEN}" },
  "allowed_env_vars": ["HOOK_TOKEN"],
  "timeout": 15
}
```

Um cabeçalho só consegue ler uma variável de ambiente, como `$NOME` ou `${NOME}`, se ela estiver em `allowed_env_vars`; qualquer outra vira vazio.

### `prompt` e `agent`

Perguntam a um modelo se uma condição é verdadeira. O modelo recebe o seu `prompt` e a entrada do hook (onde o prompt diz `$ARGUMENTS`, ou depois dele) e precisa responder um objeto JSON `{"ok": true|false, "reason": "…"}`. `ok: false` bloqueia com esse motivo; uma resposta que não é esse objeto é um erro.

- `prompt` pergunta ao modelo uma vez.
- `agent` roda antes um pequeno agente somente leitura: ele pode usar `Read`, `Glob` e `Grep` (os que a execução oferece), em modo plan, por até 50 turnos.

```json
{
  "Stop": [
    {
      "hooks": [
        {
          "type": "agent",
          "prompt": "Confira se cada arquivo que o agente mudou tem um teste correspondente. $ARGUMENTS",
          "timeout": 120
        }
      ]
    }
  ]
}
```

Hooks `prompt` e `agent` funcionam nos eventos que rodam junto com o modelo: `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `PermissionRequest`, `UserPromptSubmit`, `Stop` e `SubagentStop`. São recusados em `Brief`, `SessionStart`, `SessionEnd`, `RunStart`, `RunEnd`, `SubagentStart`, `PreCompact`, `PostCompact`, `ModelRequest`, `ModelResponse` e `ModelError`.

## Referência de eventos

### Brief

Dispara depois que o {{NAME}} encontrou as skills, os agentes, as regras e os arquivos de instruções, antes de o prompt de sistema ser montado. Eles são encontrados no primeiro prompt de uma sessão e guardados para os prompts seguintes até que um dos arquivos de onde vêm (configurações, arquivos de instruções, as pastas de skills, agentes, regras, hooks, plugins ou ferramentas) mude; o prompt seguinte os encontra de novo e o hook roda de novo. Use-o para acrescentar, mudar ou remover esses itens, por exemplo para injetar regras da equipe vindas de um servidor.

A entrada acrescenta:

| Campo | Valor |
|---|---|
| `skills` | `[{ name, description, path, body, disableModelInvocation }]` |
| `agents` | `[{ name, description, instructions, config }]` |
| `rules` | `[{ path, base, text, patterns, error }]` |
| `instructions` | `[{ path, scope, text }]`, com `scope` sendo `User`, `Project` ou `Local` |

Saída: retorne qualquer um de `skills`, `agents`, `rules` e `instructions` no nível de cima do JSON (não dentro de `hookSpecificOutput`). Uma lista retornada substitui aquela lista; uma que você não retorna fica como estava.

- `skills`: cada item precisa de `name`. Um item com nome conhecido mantém aquela skill e muda os campos que você passar (`description`, `body`, `disableModelInvocation`); um nome novo precisa de `description` e `body`. Skills que ficam fora da lista são removidas.
- `agents`, `rules`, `instructions`: a lista substitui a atual. Um nome de agente pode aparecer uma vez só.

Os hooks `Brief` rodam um depois do outro, cada um vendo o resultado do anterior. Um bloqueio encerra a sessão: `a Brief hook stopped the session: <motivo>`.

```javascript
export const event = "Brief";
export default (input) => ({
  rules: [...input.rules, { path: "/equipe/rust.md", text: "Sem unwrap.", patterns: ["**/*.rs"] }],
  agents: input.agents.filter((agent) => agent.name !== "Plan"),
});
```

### SessionStart

Dispara uma vez por sessão: no primeiro prompt que o {{NAME}} executa para ela neste processo, seja a conversa nova ou retomada. Os prompts seguintes da mesma sessão não o disparam de novo; [`RunStart`](#runstart) dispara a cada prompt.

| Campo de entrada | Valor |
|---|---|
| `source` | `resume` quando a execução continua uma conversa anterior, senão `startup` |
| `model` | O id do modelo |

O contexto do hook (JSON `additionalContext`/`systemMessage`, ou stdout simples) é acrescentado a essa primeira mensagem e fica na conversa. Um bloqueio encerra a execução com o motivo.

> **Mudou:** `SessionStart` e `SessionEnd` disparavam a cada prompt. Um hook que precisa rodar a cada prompt agora escuta `RunStart` ou `RunEnd`: troque `export const event = "SessionStart"` por `"RunStart"`, ou mova uma entrada das configurações de `"SessionStart"` para `"RunStart"`.

### RunStart

Dispara no início de cada execução (cada prompt que você envia), depois de `SessionStart` quando este dispara, antes de `UserPromptSubmit`. A mesma entrada de `SessionStart`.

O contexto do hook (JSON `additionalContext`/`systemMessage`, ou stdout simples) é acrescentado à sua mensagem. Um bloqueio encerra a execução com o motivo.

### UserPromptSubmit

Dispara quando um prompt é enviado, depois de qualquer `/skill` ou comando nele ter sido expandido, antes de o modelo vê-lo.

| Campo de entrada | Valor |
|---|---|
| `prompt` | O texto do prompt |

- `hookSpecificOutput.updatedPrompt` substitui o prompt.
- O contexto (JSON ou stdout simples) é acrescentado à mensagem.
- Um bloqueio encerra a execução com o motivo.

```bash
#!/bin/sh
echo "Branch atual: $(git branch --show-current)"
```

### ModelRequest

Dispara antes de cada chamada ao modelo, para o agente principal e para os subagentes.

| Campo de entrada | Valor |
|---|---|
| `model` | O modelo que vai ser chamado |
| `kind` | `main` ou `sub-agent` |
| `mode` | `ask`, `edits`, `plan` ou `bypass` |
| `messages` | Quantas mensagens são enviadas |
| `effort` | O nível de esforço |
| `maxTokens` | O limite de tokens da resposta |
| `tools` | Os nomes das ferramentas oferecidas |
| `system` | O prompt de sistema |
| `history` | As mensagens, como objetos `{ role, blocks }` |

Saída (`hookSpecificOutput`): `model` manda a chamada para outro modelo (`provedor:modelo` para trocar de provedor), `effort` muda o esforço, `system` e `history` substituem o prompt de sistema e as mensagens, `additionalContext` é acrescentado ao prompt de sistema daquela chamada. Um bloqueio encerra a execução: `A ModelRequest hook blocked the call: <motivo>`.

```javascript
export const event = "ModelRequest";
export const matcher = "sub-agent";
export default () => ({ hookSpecificOutput: { model: "provedor-barato:modelo-pequeno" } });
```

### ModelResponse

Dispara quando chega uma resposta, antes de ela ser mostrada ou usada. Enquanto houver um hook `ModelResponse` configurado, o texto da resposta aparece quando o hook termina, em vez de chegar aos poucos.

A entrada acrescenta os campos `model`, `kind`, `mode` e `messages` do `ModelRequest`, e:

| Campo de entrada | Valor |
|---|---|
| `text` | O texto da resposta |
| `toolCalls` | `[{ name, input }]` |
| `stopReason` | `end_turn`, `tool_use`, `max_tokens`, `refusal` ou `pause_turn` |
| `usage` | Tokens `{ input, output }` |
| `rejections` | Quantas respostas foram rejeitadas seguidas |

- `hookSpecificOutput.text` substitui o texto da resposta (por exemplo para esconder um segredo).
- Um bloqueio descarta a resposta e pede ao modelo que responda de novo, informando o motivo. O modelo é chamado de novo no máximo duas vezes seguidas; uma rejeição a mais faz a execução falhar.

```javascript
export const event = "ModelResponse";
export default (input) => {
  const text = input.text.replace(/sk-[A-Za-z0-9]{8,}/g, "[oculto]");
  if (text !== input.text) return { hookSpecificOutput: { text } };
};
```

### ModelError

Dispara quando uma chamada ao modelo falha, antes de a lista `fallbackModel` ser usada.

A entrada acrescenta `model`, `kind`, `mode`, `messages` e:

| Campo de entrada | Valor |
|---|---|
| `error` | A mensagem de erro |
| `errorKind` | `overloaded`, `unauthorized`, `network`, `provider` ou `too_long` |
| `attempt` | Quantos modelos falharam desde a última resposta |
| `tried` | Os modelos que falharam desde a última resposta |
| `fallbacks` | A lista `fallbackModel` das configurações |

Responda `hookSpecificOutput.model` para encaminhar a chamada a esse modelo, bloqueie para encerrar a execução com o erro, ou não responda nada para deixar o `fallbackModel` decidir. Um modelo que já está em `tried` não é tentado de novo antes de uma chamada dar certo.

```javascript
export const event = "ModelError";
export const matcher = "main";
const chain = ["provedor-reserva:outro-modelo", "terceiro-provedor:modelo"];
export default (input) => {
  if (input.errorKind === "too_long") return;
  const next = chain.find((model) => !input.tried.includes(model));
  if (next) return { hookSpecificOutput: { model: next } };
};
```

Veja [Modelos e provedores](./models-and-providers) sobre `fallbackModel`.

### PreToolUse

Dispara antes de uma ferramenta rodar, para as ferramentas oferecidas ao agente.

| Campo de entrada | Valor |
|---|---|
| `tool_name` | A ferramenta, como `Bash` ou `mcp__github__create_issue` |
| `tool_use_id` | O id da chamada |
| `tool_input` | Os argumentos da ferramenta |

- Um bloqueio recusa a chamada; o agente vê `Hook blocked <ferramenta>: <motivo>`.
- `permissionDecision: "allow"` roda a chamada sem perguntar, a menos que uma regra de permissão mande perguntar. `"ask"` sempre pergunta a você. `"deny"` recusa. Uma regra `deny` nas configurações sempre vence.
- `updatedInput` substitui os argumentos. Se a nova entrada tocar um caminho com regras que o agente ainda não viu, a chamada volta para que ele as leia antes.
- O contexto é acrescentado ao resultado da ferramenta como `Hook context: …`.

```json
{
  "hookSpecificOutput": {
    "permissionDecision": "allow",
    "permissionDecisionReason": "Comando git somente leitura"
  }
}
```

### PermissionRequest

Dispara quando uma chamada de ferramenta iria mostrar um pedido de permissão (depois de checadas as regras e o modo), antes de o pedido aparecer.

Entrada: `tool_name`, `tool_input`, `tool_use_id`.

Responda com `hookSpecificOutput.decision`:

- `{"behavior": "allow"}` aprova a chamada sem perguntar, a menos que uma regra de permissão mande perguntar;
- `{"behavior": "deny"}` recusa (`permissionDecisionReason` é o motivo);
- `{"behavior": "ask"}` mostra o pedido normalmente.

Se `decision.updatedInput` for diferente da entrada, a chamada não roda: o agente é instruído a refazê-la com esses argumentos, para que as regras de caminho sejam checadas sobre eles.

### PostToolUse

Dispara depois que uma ferramenta retorna com sucesso.

Entrada: `tool_name`, `tool_use_id`, `tool_input` e `tool_response` (o texto do resultado).

- `updatedToolOutput` substitui o resultado que o agente vê (uma string, ou qualquer valor JSON, que é passado como texto).
- Um bloqueio marca o resultado como falho e acrescenta `Hook feedback: <motivo>`, para que o agente se corrija.
- O contexto é acrescentado ao resultado.

```javascript
export const event = "PostToolUse";
export const matcher = "^(Edit|Write)$";
export const capabilities = ["exec"];
export default (input, ctx) => {
  const out = ctx.exec("cargo clippy --quiet -- -D warnings");
  if (out.code !== 0) return { decision: "block", reason: out.stderr.slice(0, 2000) };
};
```

### PostToolUseFailure

Como `PostToolUse`, para uma chamada de ferramenta que falhou (inclusive uma que um hook `PreToolUse` bloqueou). Mesma entrada e mesma saída.

### Notification

Dispara quando o {{NAME}} precisa de você. A saída é ignorada, o que o torna um bom lugar para notificações na área de trabalho ou sons.

| Campo de entrada | Valor |
|---|---|
| `notification_type` | `permission_prompt`, `elicitation_dialog` (o agente tem perguntas) ou `plan_approval` (um plano está pronto) |
| `message` | Um texto curto, como *{{NAME}} needs your permission to use Bash* |

```json
{
  "hooks": {
    "Notification": [
      { "hooks": [{ "type": "command", "command": "osascript -e 'display notification \"O {{NAME}} precisa de você\"'" }] }
    ]
  }
}
```

### TeammateIdle

Dispara num teammate (veja [Equipes de agentes](./agents#equipes-de-agentes)) quando ele vai ficar ocioso e avisar o líder, depois que os hooks `Stop` o deixam terminar. Mesma entrada do [`Stop`](#stop), mais `teammate_name` e `team_name`. Um bloqueio mantém o teammate trabalhando, com o motivo como próxima mensagem, como o `Stop`.

### TaskCreated

Dispara antes de `TaskCreate` acrescentar uma tarefa. `task` traz o `subject` e a `description` propostos; `teammate_name` é quem a cria (`team-lead` para o líder). Um bloqueio recusa a tarefa, e o agente lê o motivo.

### TaskCompleted

Dispara antes de `TaskUpdate` marcar uma tarefa como `completed`. `task` traz a tarefa inteira, com responsável e bloqueios; `teammate_name` é quem a conclui. Um bloqueio deixa a tarefa aberta, e o agente lê o motivo, por exemplo *rode os testes antes*.

### SubagentStart

Dispara quando um subagente começa (não quando um agente em segundo plano é retomado).

| Campo de entrada | Valor |
|---|---|
| `agent_type` | O tipo do subagente |
| `prompt` | A tarefa que ele recebeu |

O contexto é acrescentado à conversa do subagente. Um bloqueio faz o subagente falhar com o motivo.

### SubagentStop

Dispara quando um subagente quer terminar. Mesma entrada e mesmo comportamento de [`Stop`](#stop). Um hook `Stop` declarado no frontmatter de um agente próprio roda aqui.

### Stop

Dispara quando o agente principal está para encerrar o turno.

| Campo de entrada | Valor |
|---|---|
| `last_assistant_message` | A última mensagem do agente |
| `stop_hook_active` | `true` quando o agente já está continuando porque um hook de parada bloqueou |

Um bloqueio mantém o agente trabalhando: ele recebe `Completion blocked by hook: <motivo>` mais qualquer contexto. Confira `stop_hook_active` para não deixar o agente andando em círculos.

```bash
#!/bin/sh
input=$(cat)
if [ "$(printf '%s' "$input" | jq -r .stop_hook_active)" = "true" ]; then exit 0; fi
if ! cargo test --quiet >/dev/null 2>&1; then
  echo "Os testes falham; corrija antes de terminar." >&2
  exit 2
fi
```

### PreCompact

Dispara antes de a conversa ser resumida, por `/compact` ou automaticamente.

| Campo de entrada | Valor |
|---|---|
| `trigger` | `manual` ou `auto` |
| `custom_instructions` | O que `/compact <texto>` pediu para manter (vazio nos outros casos) |
| `context_tokens` | O tamanho estimado da conversa |

Um bloqueio cancela a compactação: `Compaction blocked by hook: <motivo>`.

### PostCompact

Dispara depois de a conversa ser resumida. Mesma entrada de `PreCompact`. O contexto é acrescentado à conversa depois do resumo.

### RunEnd

Dispara no fim de cada execução, qualquer que seja o resultado, antes de a conversa ser salva. A entrada tem só os campos comuns. A saída é ignorada.

### SessionEnd

Dispara uma vez por sessão que disparou `SessionStart`, quando o {{NAME}} deixa a sessão: quando você sai, ou quando outras oito sessões foram usadas depois dela no mesmo processo. Roda com os hooks da última execução da sessão. A entrada tem só os campos comuns. A saída é ignorada. Um processo que é morto não o dispara.

## Variáveis de ambiente

| Variável | Definida para | Valor |
|---|---|---|
| `{{ENV}}_PROJECT_DIR` | Hooks `command` | A pasta do projeto |
| `{{ENV}}_PLUGIN_ROOT` | `hooks.json` de plugins | Escrita como `${{{ENV}}_PLUGIN_ROOT}`, trocada pela pasta do plugin antes de o hook rodar |

Os `headers` de um hook HTTP só conseguem ler as variáveis listadas no seu `allowed_env_vars`.

## Exemplos

### Formatar cada arquivo editado

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "^(Edit|Write)$",
        "hooks": [
          { "type": "command", "command": "jq -r .tool_input.file_path | xargs npx prettier --write >/dev/null" }
        ]
      }
    ]
  }
}
```

### Permitir comandos git somente leitura sem perguntar

```javascript
export const event = "PreToolUse";
export const matcher = "^Bash$";
export default (input) => {
  if (/^git (status|diff|log|show)\b/.test(input.tool_input.command)) {
    return { hookSpecificOutput: { permissionDecision: "allow" } };
  }
};
```

### Proteger arquivos

```javascript
export const event = "PreToolUse";
export const matcher = "^(Edit|Write|NotebookEdit)$";
export default (input) => {
  const path = input.tool_input.file_path ?? "";
  if (path.endsWith(".env") || path.includes("/migrations/")) {
    return { decision: "block", reason: `${path} é protegido.` };
  }
};
```

### Perguntar a um modelo se um comando é seguro

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "^Bash$",
        "hooks": [
          {
            "type": "prompt",
            "prompt": "Este comando de shell só lê arquivos, sem apagar nem enviar nada? $ARGUMENTS",
            "timeout": 30
          }
        ]
      }
    ]
  }
}
```

## Veja também

- [Permissões](./permissions) para regras allow, ask e deny.
- [Configurações](./settings) para os arquivos onde ficam os hooks.
- [Plugins](./plugins) para compartilhar hooks.
- [Agentes](./agents) para hooks limitados a um agente.
