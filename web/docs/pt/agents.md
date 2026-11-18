---
title: Agentes
description: Defina subagentes com instruções, ferramentas, modelo e permissões próprios, e deixe o {{NAME}} delegar tarefas focadas a eles.
---

Um **subagente** é um agente separado que o {{NAME}} inicia para uma tarefa. Ele trabalha numa conversa própria, com instruções e ferramentas próprias, e devolve um único relatório. Delegar mantém a conversa principal focada: o subagente lê vinte arquivos e só a conclusão volta.

O {{NAME}} tem subagentes embutidos, e você pode definir os seus como arquivos Markdown.

## Começo rápido

Crie `{{DIR}}/agents/reviewer.md` no seu projeto:

```markdown
---
name: reviewer
description: Revisa um diff em busca de bugs e testes faltando. Use depois de uma mudança, antes do commit.
tools: Read, Grep, Glob, Bash
model: inherit
---

Você é um revisor de código rigoroso. Leia o diff com `git diff` e depois os
arquivos alterados. Aponte cada bug com arquivo e linha, e cada mudança sem
teste. Não edite nada.
```

A partir do próximo prompt, o agente pode delegar a ele ("pede para o reviewer conferir isso") e o escolhe sozinho quando a descrição combina com a tarefa. `/agents` lista todos os agentes disponíveis aqui.

> [!TIP]
> `/agents new <o que ele deve fazer>` pede ao agente que escreva o arquivo para você em `{{DIR}}/agents/<nome>.md`.

## Como a delegação funciona

O agente principal delega com a ferramenta `Agent`:

| Entrada | Significado |
|---|---|
| `description` | Um resumo de 3 a 5 palavras, mostrado no chat. Obrigatório. |
| `prompt` | A tarefa. O subagente só sabe o que está escrito aqui. Obrigatório. |
| `subagent_type` | O agente a iniciar; `general-purpose` quando omitido |
| `run_in_background` | `true` roda enquanto o agente principal segue; o relatório é recolhido com `TaskOutput` |
| `isolation` | `worktree` roda numa worktree git própria |
| `max_turns` | No máximo esse número de turnos do modelo, de 1 a 200 |
| `model` | Um modelo só para esta chamada; ignorado se o provedor atual não o oferecer |

A descrição da ferramenta lista todos os agentes disponíveis com suas descrições, então uma `description` clara é o que faz o agente escolher o seu.

Regras da delegação:

- Um subagente não pode iniciar outros subagentes.
- O relatório volta para o agente principal, que repassa o que importa; ele não é mostrado a você como está.
- Um subagente recebe os mesmos arquivos de instruções que o agente principal, a menos que seja um agente embutido somente leitura ou defina `omitClaudeMd: true`. Ele não recebe o estilo de saída da conversa.
- Os hooks também disparam para ele: `SubagentStart` quando começa, `SubagentStop` quando quer terminar (veja [Hooks](./hooks)).

## Agentes embutidos

| Agente | Para que serve | Ferramentas |
|---|---|---|
| `general-purpose` | Pesquisa e tarefas de vários passos (o padrão) | Todas as do agente principal |
| `Explore` | Busca rápida somente leitura: arquivos por padrão, símbolos, "onde X é definido" | Sem `Agent`, `Edit`, `Write` nem `StructuredOutput`; começa sem arquivos de instruções |
| `Plan` | Desenha um plano de implementação: passos, arquivos críticos, trade-offs | Igual ao `Explore` |
| `fork` | Uma cópia do agente principal, com toda a conversa e o modelo dele, para uma tarefa paralela | Todas as do agente principal |

O `Explore` usa o modelo pequeno quando não indica um (veja [Modelos e provedores](./models-and-providers)). Um arquivo seu com o mesmo nome substitui um agente embutido.

## Onde ficam os agentes

| Local | Escopo |
|---|---|
| `{{DIR}}/agents/*.md` na pasta de trabalho ou em qualquer pasta acima | O projeto |
| `~/{{DIR}}/agents/*.md` | Você, em todos os projetos |
| `<plugin>/agents/*.md` | Um plugin instalado, com o nome `<plugin>:<nome>` |

Quando dois agentes têm o mesmo nome, vale o primeiro encontrado: a pasta de trabalho, cada pasta acima dela, os seus, os plugins e, por fim, os embutidos. Os agentes são procurados de novo no início de cada prompt.

> [!NOTE]
> Agentes nas pastas de outra ferramenta, como `.claude/agents/`, não são lidos.

## Formato do arquivo

Um agente é um arquivo Markdown com frontmatter YAML. O corpo são as instruções dele, acrescentadas ao prompt de sistema sob `# Your instructions (<nome>)`.

```markdown
---
name: migrations
description: Escreve e confere migrações de banco de dados. Use para qualquer mudança de schema.
tools: [Read, Edit, Write, Bash, Grep, Glob]
disallowedTools: [WebFetch]
model: inherit
effort: high
maxTurns: 60
permissionMode: acceptEdits
skills: [db-conventions]
memory: project
isolation: worktree
---

Você escreve migrações reversíveis...
```

| Campo | Valor | Significado |
|---|---|---|
| `name` | Texto | O nome do agente. Por padrão, o nome do arquivo sem `.md`. |
| `description` | Texto | Quando usá-lo. O agente principal escolhe por aqui. |
| `tools` | Lista ou texto separado por vírgulas | As únicas ferramentas que ele recebe. Um nome terminado em `*` casa por prefixo: `mcp__docs__*`. Todas, quando omitido. |
| `disallowedTools` | Lista ou texto separado por vírgulas | Ferramentas que ele nunca recebe, com a mesma sintaxe |
| `mcpServers` | Lista ou texto separado por vírgulas | Os únicos servidores MCP cujas ferramentas ele recebe |
| `model` | Um id de modelo, um nome curto, `provedor:modelo` ou `inherit` | O modelo dele. `inherit` (ou nada) usa o do agente principal. |
| `effort` | `low`, `medium`, `high`, `xhigh` ou `max` | O nível de esforço |
| `maxTurns` | 1 a 200 | No máximo esse número de turnos do modelo |
| `permissionMode` | `default`, `acceptEdits`, `plan`, `bypassPermissions` ou `dontAsk` | O máximo que ele pode fazer sem perguntar (veja abaixo) |
| `skills` | Lista ou texto separado por vírgulas | Skills carregadas no prompt de sistema dele quando começa |
| `memory` | `user`, `project` ou `local` | Dá a ele uma pasta de memória própria (veja abaixo) |
| `isolation` | `worktree` | Roda numa worktree git própria |
| `background` | `true` ou `false` | Roda em segundo plano, a menos que a chamada diga o contrário |
| `omitClaudeMd` | `true` ou `false` | `true` faz ele começar sem os arquivos de instruções |
| `hooks` | Como `"hooks"` nas configurações | Hooks que só valem enquanto ele roda |
| `initialPrompt` | Texto | Usado quando ele é o agente principal (veja abaixo) |
| `autoCompactWindow` | 100000 a 1000000 | Compacta a conversa dele quando ela chega a esse número de tokens |
| `observer` | Nomes de agentes, até 4 | Agentes que observam os turnos deste (veja [Observadores](#observadores)) |
| `observerMessage` | Texto | Acrescentado ao fim de cada resumo que os observadores dele recebem |
| `observeSubagents` | `true` (padrão) ou `false` | Se os sub-agentes dele são observados pelos mesmos observadores |

Um seletor de ferramenta só pode ter letras, dígitos, `_`, `-` e `:`, mais um `*` no fim; `Bash(git:*)` não é um seletor. Um arquivo com campo inválido continua aparecendo na lista, mas iniciá-lo falha com `Agent <nome>: <problema>`, por exemplo `Agent migrations: maxTurns must be within 1..200`.

Um agente do projeto só mantém `hooks`, `mcpServers`, `permissionMode` e `initialPrompt` depois que você confia na pasta do projeto; até lá esses campos são descartados e o agente é oferecido sem eles (veja [Confiança na pasta](./sandbox-and-safety#confianca-na-pasta)).

### Modelo

| `model` | Resultado |
|---|---|
| Omitido ou `inherit` | O modelo do agente principal |
| Um nome curto que o provedor conhece, como `haiku` | O modelo correspondente desse provedor |
| Um modelo que o provedor atual oferece | Esse modelo |
| `provedor:modelo` de um provedor conectado | O modelo desse provedor |

Qualquer outra coisa faz a chamada falhar com *the agent model must be served by the current provider or name a connected one as provider:model*. Um hook `ModelRequest` com matcher `sub-agent` também pode direcionar as chamadas de todos os subagentes (veja [Hooks](./hooks#modelrequest)).

### Modo de permissão

`permissionMode` é um teto: ele pode diminuir o que o subagente pode fazer, nunca aumentar acima do modo da conversa principal. `bypassPermissions` numa conversa em `ask` continua perguntando.

| Valor | Modo |
|---|---|
| `default` | `ask` |
| `acceptEdits` | `edits` |
| `plan` | `plan` (somente leitura) |
| `bypassPermissions` | `bypass`, se a conversa permitir |
| `dontAsk` | `ask`, mas nunca mostra um pedido: o que seria perguntado é recusado |

Os nomes do próprio app (`ask`, `edits`, `plan`, `bypass`) e as outras grafias listadas em [Modos de permissão](./permissions#modos-de-permissao) também são lidos, em maiúsculas ou minúsculas.

### Memória

`memory` dá ao agente uma pasta privada que sobrevive entre execuções, e as ferramentas `Read`, `Write` e `Edit` para usá-la:

| Valor | Pasta |
|---|---|
| `user` | `~/{{DIR}}/agent-memory/…` |
| `project` | `<repositório>/{{DIR}}/agent-memory/…` |
| `local` | `<repositório>/{{DIR}}/agent-memory-local/…` (deixe fora do git) |

Cada agente tem a própria subpasta. As primeiras 200 linhas (no máximo 25 KB) do `MEMORY.md` dele entram no prompt de sistema, e ele é instruído a manter esse índice curto e guardar os detalhes em outros arquivos. Essa memória fica sempre ligada para um agente que declara um escopo `memory`; deixe `memory` de fora para um agente não guardar memória (veja [Memória](./memory)).

### Isolamento em worktree

`isolation: worktree` (ou `isolation` na chamada do `Agent`) roda o agente numa worktree git nova, para que as edições dele não toquem sua árvore de trabalho até você aproveitá-las. Quando ele muda algo, a worktree é mantida e o caminho dela entra no relatório (*Changes retained in worktree: …*). Fora de um repositório git com pelo menos um commit, o agente roda sem isolamento.

### Hooks num agente

```markdown
---
name: careful-editor
description: Faz edições arriscadas com verificações.
hooks:
  PostToolUse:
    - matcher: "^(Edit|Write)$"
      hooks:
        - type: command
          command: cargo check --quiet
  Stop:
    - hooks:
        - type: command
          command: ./scripts/verify.sh
---
```

Esses hooks só rodam enquanto este agente trabalha. Um hook `Stop` aqui roda como `SubagentStop` quando o agente é um subagente.

### Agentes de plugins

Agentes de um plugin se chamam `<plugin>:<nome>`. Por segurança, os campos `hooks`, `mcpServers`, `permissionMode` e `initialPrompt` deles são ignorados.

### Observadores

Um observador é um agente que acompanha o trabalho de outro e só se manifesta quando vale a pena. Nomeie-o no campo `observer` do agente observado (ou no do agente principal, veja abaixo):

```markdown
---
name: migrations
description: Escreve migrações de banco
observer: reviewer
observerMessage: Aponte qualquer migração que apague uma coluna.
---
```

- Enquanto o agente observado trabalha, cada observador recebe um resumo do que aconteceu desde o último: o pedido, as chamadas de ferramenta e seus resultados, o que o agente disse e, no fim, como o turno terminou. Um resumo sai depois de cada passo que rodou ferramentas, em segundo plano, então o agente não espera; um observador ainda lendo o anterior recebe os próximos itens depois, juntos. O resumo vem dentro de uma tag com um nonce novo e é apresentado ao observador como dado, não como instrução. Cada item é cortado em 2.000 caracteres (o pedido em 16.000), e um resumo guarda no máximo cerca de 64.000 caracteres, descartando primeiro os itens mais antigos.
- O observador roda com as instruções e o modelo da própria definição, com uma única ferramenta, `ObserverReport`. Ele mantém a conversa entre os turnos da mesma sessão, para acompanhar o trabalho.
- Um relatório chega ao agente observado como conselho do observador, nunca como palavras suas ou consentimento. Ele chega ao agente entre os passos, enquanto ele trabalha. Um relatório ainda não lido quando o turno termina faz o agente principal continuar uma vez no mesmo turno para pesá-lo, em vez de abrir um turno sozinho; relatórios de observadores de um sub-agente chegam ao agente principal entre os passos dele.
- Quando o turno termina, ele espera os resumos ainda em leitura e envia os últimos itens com como o turno terminou. Um turno que você interrompe deixa de ser observado.
- Sub-agentes herdam os observadores do pai, até dois níveis abaixo, a menos que `observeSubagents: false`. Um sub-agente que nomeia os próprios observadores usa esses. Um observador nunca é observado.
- Um nome de observador que não corresponde a nenhum agente é ignorado e registrado no log; além de quatro nomes, os demais ficam de fora.

## Rodar a conversa como um agente

A configuração `agent` faz a própria conversa principal rodar como um dos seus agentes:

```json
{ "agent": "migrations" }
```

As instruções do agente entram no prompt de sistema sob `# Agent instructions (<nome>)`, e `tools`, `disallowedTools`, `mcpServers`, `model`, `effort`, `permissionMode`, `skills`, `memory`, `isolation` e `hooks` dele valem para a conversa. O `initialPrompt` vai antes da sua primeira mensagem numa conversa nova; um `initialPrompt` que começa com `/` roda essa skill ou comando.

## Equipes de agentes

Um sub-agente trabalha uma vez e relata. Um **teammate** continua trabalhando: roda no próprio processo, recebe mensagens enquanto vive e espera ocioso entre tarefas. O agente com quem você conversa é o **líder** da equipe (`team-lead`); ele inicia os teammates e os coordena.

- **Iniciar um.** O líder chama `Agent` com um `name` (letras, dígitos, `_`, `-`) e a tarefa em `prompt`, opcionalmente um `subagent_type` e um `model`. Um nome já usado na equipe ganha `-2`, `-3`…; `main`, `team-lead` e `user` são reservados. O teammate roda `{{COMMAND}} teammate` na mesma pasta, com o modelo do líder (a menos que outro seja pedido), o modo de permissão do líder e as instruções da definição de agente. Com `teammateMode` em `tmux` ou `iterm2` (ou `auto` dentro de um deles), ele abre num painel novo ao lado do líder e imprime ali; senão roda em segundo plano e imprime no log. Um painel do iTerm2 é aberto por AppleScript e lê as configurações de `spawn/<nome>.json` na pasta da equipe, então o caminho do app e a sua pasta pessoal só podem ter letras, dígitos e `/._-`.
- **A equipe.** Cada sessão tem uma equipe, criada com o primeiro teammate, em `~/{{DIR}}/teams/team-<sessão>/`: `config.json` lista os membros, `inboxes/<nome>.json` guarda as mensagens de cada um e `logs/<nome>.log` o que cada teammate imprimiu. Um teammate que termina é marcado como inativo.
- **Mensagens.** `SendMessage` recebe o nome de um teammate (ou `team-lead`, também escrito `main`, a partir de um teammate), uma `message` e um `summary` curto. As mensagens chegam ao líder entre os passos dele, ou como um turno próprio quando o chat está ocioso, como `<teammate-message teammate_id="…" summary="…">`. Um teammate lê as mensagens quando está ocioso; a tarefa do líder chega como a primeira mensagem.
- **Ociosidade e encerramento.** Depois de cada resposta, o teammate avisa o líder que está ocioso (`idle_notification`). O líder encerra um teammate com `SendMessage` e `message: {"type": "shutdown_request", "reason": "…"}`; o teammate responde `shutdown_approved` e termina. `shutdown_response` (com `request_id` e `approve`) e `plan_approval_response` são as outras mensagens estruturadas.
- **Pedidos de permissão.** Um teammate não pode perguntar a você diretamente: quando uma chamada dele precisa da sua permissão, ele manda o pedido ao líder (`permission_request`) e espera. O chat do líder pergunta a você, como *Bash (teammate tester)* com o que a chamada vai fazer, um pedido de cada vez; o seu Sim ou Não volta só para aquele teammate (`permission_response`). O modelo do líder nunca vê esses pedidos. *Sim, e não perguntar de novo* é oferecido quando o teammate pode guardar a regra, e ele a guarda para a própria sessão. Um plano que um teammate quer aprovar (`plan_approval_request`) aparece para você do mesmo jeito; aprová-lo define o modo em que o teammate o executa, e uma recusa com nota devolve a nota. Quando a sessão do líder fecha, todo teammate ainda trabalhando recebe um pedido de encerramento, e um que esperava resposta para de esperar.
- **Quem pode o quê.** Só o líder inicia teammates; um teammate, ou um sub-agente, não. Um teammate relata com `SendMessage` para `team-lead`: o que ele imprime ninguém lê além do log.

- **Tarefas compartilhadas.** `TaskCreate`, `TaskUpdate`, `TaskGet` e `TaskList` usam uma lista por sessão do líder, em `~/{{DIR}}/tasks/<sessão>/tasks.json`, compartilhada pelo líder e por todos os teammates e mantida entre os turnos. Os ids são `1`, `2`…; cada tarefa mostra o `owner`, do que depende (`blocked_by`) e o que bloqueia. `TaskUpdate` com `owner` reivindica uma tarefa; uma tarefa de outro não pode ser reivindicada (o líder pode reatribuí-la), uma tarefa não começa nem termina enquanto uma de que depende está aberta, e dar uma tarefa a um teammate manda a ele uma mensagem `task_assignment`. Um teammate que termina devolve as tarefas abertas, e o líder é avisado de quais.

## Listar agentes

`/agents` lista todos os agentes disponíveis aqui (os seus, os do projeto, os dos plugins e os embutidos) com a descrição. `/reload` os relê e mostra quantos foram encontrados.

## Mudar agentes com um hook

Um hook `Brief` recebe todos os agentes encontrados (nome, descrição, instruções e configuração) e pode devolver uma lista nova, por exemplo para acrescentar o revisor da equipe ou esconder um agente. Veja [Hooks](./hooks#brief).

## Veja também

- [Skills](./skills) para pré-carregar conhecimento num agente.
- [Permissões](./permissions) para modos e regras.
- [Branches e git](./branches-and-git) para worktrees.
