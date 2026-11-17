---
title: Modo headless
description: Rode o {{NAME}} sem o chat com --print, leia a resposta como texto ou JSON e use-o em scripts e CI.
---

O modo print executa um pedido sem abrir o chat: o {{NAME}} trabalha no prompt, imprime a resposta e sai. É a forma de usar o {{NAME}} em scripts, hooks do Git e CI.

## Obter uma resposta

```bash
{{COMMAND}} -p "explain src/main.rs"
```

`-p` (ou `--print`) pega o prompt da linha de comando; todas as palavras depois das opções são unidas com espaços. A resposta sai na saída padrão conforme é escrita. As ferramentas que o agente inicia aparecem na saída de erro, esmaecidas, como `● <Ferramenta>(<resumo>)`, para não se misturarem à resposta quando você a redireciona:

```bash
{{COMMAND}} -p "list the TODOs in this repository" > todos.md
```

Coloque `--` antes de um prompt que começa com hífen:

```bash
{{COMMAND}} -p -- "-v does what?"
```

## Escolher o que o agente pode fazer

Não há ninguém para responder a um pedido de permissão, então no modo print todo pedido de permissão é negado, e uma nota avisa na saída de erro:

```text
denied: the agent asked for permission (use --mode or --allowedTools)
```

Dê à execução o que ela precisa logo de início:

| Opção | Efeito |
|---|---|
| `--mode ask` | Toda ferramenta que precisa de permissão é negada |
| `--mode edits` | Edições de arquivos são permitidas; os outros pedidos são negados. É o modo usado quando `--mode` não é passado. |
| `--mode plan` | O agente planeja sem mudar nada (veja abaixo) |
| `--mode bypass` | Nada pede permissão |
| `--allowedTools "<regras>"` | Estas regras são permitidas sem perguntar |
| `--disallowedTools "<regras>"` | Estas regras são recusadas |

```bash
{{COMMAND}} -p --allowedTools "Bash(cargo test:*) Edit" "make the failing test pass"
```

As regras são separadas por espaços ou vírgulas (fora dos parênteses), e as duas opções podem ser repetidas. A sintaxe das regras está em [Permissões](./permissions).

### Hooks, scripts e servidores do projeto precisam de `--trust`

O modo de impressão nunca pergunta se você confia na pasta. Se a pasta ainda não é confiável, a execução não carrega nada do projeto que rode código ou conceda permissões (hooks, ferramentas de script, servidores MCP, provedores declarados, concessões do `settings.local.json`), e uma linha na saída de erro diz quantos itens foram ignorados. Passe `--trust` para carregá-los naquela chamada; nada é guardado. Quando a pasta não pode ser confiável (a sua pasta pessoal, por exemplo), uma linha `--trust ignored: <motivo>` avisa na saída de erro e a execução segue sem confiança:

```bash
{{COMMAND}} -p --trust "rode as verificações de release"
```

> [!WARNING]
> Um script ou job de CI que depende dos hooks do projeto precisa passar `--trust`. Use a opção só onde você controla o conteúdo do repositório. Veja [Confiança na pasta](./sandbox-and-safety#confianca-na-pasta).

> [!WARNING]
> `--mode bypass` deixa o agente rodar qualquer comando sem perguntar. Use só num ambiente descartável, como um container de CI ou um checkout de rascunho. Veja [Sandbox e segurança](./sandbox-and-safety).

### Modo plan no print

Com `--mode plan`, quando o agente termina o plano, o plano é impresso e não é executado:

```text
plan not carried out: run again with --mode edits to implement it
```

## Formatos de saída

`--output-format` escolhe o que vai para a saída padrão. Exige `--print`.

| Formato | Saída padrão |
|---|---|
| `text` (padrão) | A resposta conforme é transmitida, e depois uma quebra de linha |
| `json` | Um objeto JSON com o resultado, quando a execução termina |
| `stream-json` | Cada evento como uma linha JSON, as notas como linhas JSON, e depois o resultado |

### `json`

```bash
{{COMMAND}} -p --output-format json "what does this project do?"
```

```json
{
  "type": "result",
  "subtype": "success",
  "is_error": false,
  "result": "This project is ...",
  "error": null,
  "session_id": "0f9c2d3e-...",
  "total_cost_usd": 0.0123,
  "duration_ms": 8421
}
```

| Campo | Tipo | Significado |
|---|---|---|
| `type` | string | Sempre `result` |
| `subtype` | string | `success`, ou `error` quando a execução falhou ou uma verificação falhou |
| `is_error` | boolean | `true` quando `subtype` é `error` |
| `result` | string | A resposta inteira do agente: todo o texto de todos os prompts da execução, mais o plano no modo plan |
| `error` | string ou null | Por que a execução falhou, `cancelled`, ou `null` |
| `session_id` | string ou null | O id da sessão, para continuar com `--resume` |
| `total_cost_usd` | number | O custo da execução, `0` quando o provedor não informa |
| `duration_ms` | number | Quanto tempo o trabalho do modelo levou, em milissegundos |

No formato `json`, as notas (permissões negadas, planos não executados, vereditos de verificação) não são impressas.

### `stream-json`

```bash
{{COMMAND}} -p --output-format stream-json "fix the typo in README.md"
```

Cada linha é um objeto JSON. Os eventos vêm como `{"type":"event","event":...}`, as notas como `{"type":"note","text":...}`, e a última linha é o mesmo objeto de resultado do formato `json`:

```json
{"type":"event","event":{"SessionStarted":"0f9c2d3e-..."}}
{"type":"event","event":{"TextDelta":"Hel"}}
{"type":"event","event":{"TextDelta":"lo"}}
{"type":"event","event":{"Completed":{"duration_ms":7,"cost_usd":0.25}}}
{"type":"result","subtype":"success","is_error":false,"result":"Hello","error":null,"session_id":"0f9c2d3e-...","total_cost_usd":0.25,"duration_ms":7}
```

O valor de `event` tem o tipo do evento como única chave, com os dados do evento como valor; um evento sem dados é só o nome, como `"Cancelled"`. Os tipos mais comuns:

| Evento | Dados |
|---|---|
| `SessionStarted` | O id da sessão |
| `TextDelta` | Um trecho do texto da resposta |
| `ThinkingDelta` | Um trecho do raciocínio do modelo |
| `ToolStarted` | `id`, `name`, `summary`, `output`, `failed`, e `input` quando conhecido |
| `ToolInputDelta` | `name`, `piece`: a entrada da ferramenta conforme é transmitida |
| `ToolProgress` | `id`, `line` |
| `ToolLog` | `id`, `text` |
| `ToolResult` | `id`, `output`, `failed` |
| `ToolFinished` | `id` |
| `AgentSpawned` | `index`, `kind`, `summary`, `prompt`: um subagente começou |
| `AgentEvent` | `index`, `event`: um evento desse subagente, no mesmo formato |
| `AgentUpdate` | `index`, `status`, `summary` |
| `Usage` | Os tokens que uma chamada ao modelo usou |
| `ContextMeasured` | Quanto a janela de contexto está cheia |
| `Compacted` | A conversa foi compactada |
| `Asked` | O agente pediu permissão ou fez uma pergunta (negado no modo print) |
| `FellBack` | O modelo com que seguiu depois de uma chamada que falhou |
| `AccountSwitched` | `from`, `to`: outra conta assumiu |
| `Picked` | `model`, `effort`: o que o esforço automático escolheu |
| `Verified` | Um veredito de verificação |
| `Refused` | `tool`, `reason`: uma chamada de ferramenta foi recusada |
| `Edited` | Um arquivo foi alterado |
| `Completed` | `duration_ms`, `cost_usd`: a execução terminou |
| `Failed` | Por que a execução falhou |
| `Cancelled` | A execução foi cancelada |

Outros tipos de evento podem aparecer conforme o {{NAME}} evolui; ignore os que você não usa.

### Formato text e erros

No formato `text`, a resposta vai para a saída padrão e todo o resto para a saída de erro: linhas de ferramentas, notas e, quando a execução falha, `{{COMMAND}}: <erro>`.

Em `json` e `stream-json`, a saída padrão sempre termina com um objeto de resultado, mesmo quando o comando não conseguiu começar (por exemplo `no model is available`): `is_error` é `true` e `error` diz o motivo. O erro também é impresso na saída de erro.

## Vários prompts pela entrada padrão

`--input-format stream-json` lê os prompts da entrada padrão em vez da linha de comando, um objeto JSON por linha, e responde um de cada vez na mesma sessão:

```bash
printf '%s\n' \
  '{"type":"user","message":{"content":"read src/lib.rs"}}' \
  '{"type":"user","message":{"content":[{"type":"text","text":"now add a test for parse"}]}}' \
  | {{COMMAND}} -p --input-format stream-json --output-format stream-json
```

- Só as linhas com `"type":"user"` são prompts; as outras são ignoradas, assim como as linhas em branco.
- `message.content` é uma string ou uma lista de blocos; o `text` de cada bloco é unido com quebras de linha.
- Cada linha é respondida assim que chega, então outro programa pode escrever um prompt, ler a resposta e escrever o próximo.
- Uma linha que não é JSON, cujo conteúdo não é texto, ou que não pode ser lida (por exemplo UTF-8 inválido) encerra o comando com `input line <n>: ...`; as linhas anteriores já foram respondidas.

## Limitar a execução

| Opção | Efeito |
|---|---|
| `--max-turns <n>` | O máximo de turnos do modelo que a execução pode usar. Sem ela, o limite é 200. |
| `--verification "<critérios>"` | Antes de a execução terminar, o trabalho é julgado por estes critérios |
| `--fallback-model <modelo>` | O modelo com que seguir quando o escolhido está sobrecarregado |
| `--append-system-prompt "<texto>"` | Texto adicionado ao fim do prompt de sistema |
| `-m`, `--model <modelo>` | O modelo; senão, o último que você escolheu |

Quando o agente esgota os turnos, a execução falha com `The agent stopped after too many steps without finishing.`

Com `--verification`, cada veredito vira uma nota (`✓ verification passed (score 0.90)` ou `✗ verification failed, attempt 1/3 ...`). Se algum veredito falhar, o `subtype` do resultado é `error` e o comando sai com 1. Veja [Verificação e revisão](./verification-and-review).

```bash
{{COMMAND}} -p --mode edits \
  --verification "cargo test passes and no public API changed" \
  "fix the overflow in parse_size"
```

## Continuar uma conversa

| Opção | Efeito |
|---|---|
| `-c`, `--continue` | Continua a última conversa desta pasta |
| `-r <id>`, `--resume <id>` | Continua a conversa com esse id. Com `--print` o id é obrigatório: `--resume needs a session id with --print` |
| `--fork-session` | Com `-c` ou `-r`: continua numa cópia, deixando a original como está |

O `session_id` de um resultado `json` é o id a passar para `--resume`:

```bash
id=$({{COMMAND}} -p --output-format json "summarize the open issues in NOTES.md" | jq -r .session_id)
{{COMMAND}} -p --resume "$id" "now draft a reply to the first one"
```

No modo print, `-r` sem id encerra o comando com esse erro (código 1): o seletor que ele abre no chat não existe aqui. `-c` sem conversa anterior na pasta começa uma nova.

## Outras opções

`-C <caminho>` (`--cwd`) roda em outra pasta de projeto, e `--add-dir <caminho>` deixa o agente ler outra pasta como se fosse o projeto. Veja a [referência da CLI](./cli-reference) para todas as opções.

## Códigos de saída

| Código | Quando |
|---|---|
| `0` | A execução terminou e todas as verificações passaram |
| `1` | A execução falhou ou foi cancelada, uma verificação falhou, ou o comando não conseguiu começar (por exemplo `no model is available`, `--print needs a prompt`, um `--input-format` desconhecido) |
| `2` | A linha de comando está errada: opção desconhecida, valor faltando, opção que exige `--print` usada sem ele, `--output-format` ou `--mode` desconhecido. Também quando `{{COMMAND}}` é iniciado sem `-p` e a entrada ou a saída padrão não é um terminal: `no terminal: use -p for print mode` |

## Scripts e CI

### Falhar um job de CI quando o agente falha

```bash
#!/usr/bin/env bash
set -euo pipefail
{{COMMAND}} -p --mode edits --max-turns 40 \
  --allowedTools "Bash(cargo test:*)" \
  --verification "cargo test passes" \
  "fix the failing tests"
```

### Ler a resposta e o custo com jq

```bash
out=$({{COMMAND}} -p --output-format json "is there dead code in src/?")
echo "$out" | jq -r .result
echo "custo: $(echo "$out" | jq .total_cost_usd) USD"
```

### Acompanhar as chamadas de ferramentas ao vivo

```bash
{{COMMAND}} -p --output-format stream-json "update the changelog" \
  | jq -c 'select(.type == "event" and .event.ToolStarted) | .event.ToolStarted | {name, summary}'
```

### Revisar as mudanças staged num hook do Git

```bash
#!/bin/sh
{{COMMAND}} -p --mode ask "Review this diff for bugs. Answer OK if there are none.

$(git diff --cached)" | grep -q '^OK' || exit 1
```

> [!NOTE]
> No formato de entrada `text`, o prompt vem só da linha de comando; a entrada padrão não é lida. Para passar o conteúdo de um arquivo, coloque-o no prompt (`"$(cat arquivo)"`) ou peça ao agente para ler o arquivo.

> [!TIP]
> Antes de rodar no CI, conecte um provedor nessa máquina com `{{COMMAND}} login` e confira com `{{COMMAND}} doctor`. Veja [Modelos e provedores](./models-and-providers) e [Solução de problemas](./troubleshooting).
