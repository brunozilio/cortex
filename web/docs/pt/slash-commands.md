---
title: Comandos de barra
description: Referência completa de todos os comandos de barra do chat, seus argumentos e apelidos, e de como skills e comandos personalizados rodam como /nome.
---

Os comandos de barra controlam o próprio chat: a conversa, o modelo, o projeto e o app. Digite `/` no prompt para vê-los com os apelidos e as suas skills; `↑`/`↓` escolhem e `Tab` completa.

```text
/help              lista todos os comandos e as teclas principais
/help compact      explica um comando (ou skill)
/model             escolhe um modelo
/compact mantenha as decisões sobre a API
```

## Como os comandos são lidos

- Um comando é `/` seguido de uma palavra com cara de comando (letras, dígitos, `-`, `_`, `:`, `?`) e argumentos opcionais. Os nomes não diferenciam maiúsculas.
- Uma barra seguida de um caminho, como `/Users/me/notes.txt o que é isto?`, não é comando: vai como prompt.
- Um comando sem argumentos recusa argumentos: `/clear agora` responde `/clear can't take that.` e deixa o texto no prompt.
- Um nome desconhecido ganha a sugestão mais próxima: `/modle is not a command or skill. Did you mean /model?`
- Os comandos embutidos vêm primeiro, depois os comandos de recursos abaixo, depois [skills e comandos personalizados](#skills-e-comandos-personalizados).

O `/help` agrupa os comandos em **Conversation**, **Model and mode**, **Project**, **App** e **Features**, e termina com as teclas.

## Referência rápida

| Comando | Apelidos | O que faz |
|---|---|---|
| `/help [comando]` | `/?` | Mostra os comandos e teclas, ou um comando |
| `/clear` | `/new`, `/reset` | Começa uma conversa nova |
| `/resume [palavras]` | `/continue` | Volta a uma conversa anterior desta pasta |
| `/fleet` | `/sessions` | Mostra todas as sessões, em andamento ou anteriores, e alterna entre elas |
| `/fork` | | Continua numa cópia desta conversa |
| `/compact [o que manter]` | | Resume a conversa para liberar contexto |
| `/rewind` | `/undo` | Volta para antes de um prompt anterior: arquivos e conversa |
| `/retry` | | Envia o último prompt de novo, de onde ele começou |
| `/btw <pergunta>` | | Faz uma pergunta paralela sem interromper |
| `/export [caminho]` | | Grava a conversa em Markdown |
| `/copy` | | Copia a última resposta do agente |
| `/model [id \| papel [modelo] \| roles \| fallbacks …]` | | Escolhe o modelo, por papel, ou a cadeia de fallback |
| `/mode [modo]` | `/plan` (= `/mode plan`) | Escolhe ask, edits, plan ou bypass |
| `/effort [nível \| auto]` | | Define o esforço de raciocínio |
| `/output-style [estilo]` | | Escolhe como o agente fala |
| `/diff` | | Mostra as alterações não commitadas |
| `/diffs` | | Navega pelas alterações em tela cheia |
| `/add-dir [caminho]` | | Deixa o agente ler uma pasta como o projeto |
| `/permissions` | `/allowed-tools` | Mostra as regras allow, ask e deny |
| `/hooks` | | Mostra os hooks que uma execução aqui dispara |
| `/todos` | `/tasks` | Mostra a lista de tarefas do agente |
| `/context` | | Mostra o que ocupa a janela de contexto |
| `/skills [new <o quê>]` | `/commands` | Lista skills e comandos, ou escreve um |
| `/agents [new <o quê>]` | | Lista agentes, ou escreve um |
| `/mcp [approve\|deny\|login\|logout <nome>]` | | Lista ou gerencia servidores MCP |
| `/reload` | | Relê plugins, skills, agentes e servidores MCP |
| `/cost` | `/usage` | Mostra o custo desta sessão |
| `/status` | | Mostra modelo, modo, pastas, sessão e versão |
| `/config` | `/settings` | Muda as configurações do chat |
| `/statusline [comando \| off]` | | Desenha a linha de status com o seu comando |
| `/vim` | | Liga ou desliga as teclas do vim |
| `/login` | | Conecta um provedor |
| `/logout` | | Esquece as credenciais de um provedor |
| `/accounts [remove \| balance [forma]]` | | Troca, esquece ou distribui contas |
| `/doctor` | | Verifica se esta instalação funciona aqui |
| `/terminal-setup` | | Como fazer `Shift+Enter` quebrar linha |
| `/exit` | `/quit` | Sai |
| `/branch [n]` | | Lista os prompts de onde ramificar, ou começa uma conversa nova de antes do prompt n |
| `/budget [dólares \| off]` | | Limita o gasto desta sessão |
| `/verification [critérios \| off]` | | Julga execuções contra critérios |
| `/memory [subcomando]` | | Lista, busca e gerencia a memória do projeto |
| `/learn keep\|skip` | | Decide sobre a lição oferecida depois de uma execução |
| `/check` | | Confere as alterações da branch contra as memórias |
| `/rules [keep <n>]` | | Regras de permissão sugeridas pelas suas respostas |
| `/loop [intervalo] <tarefa>` | | Roda uma tarefa agora e de novo depois |
| `/schedules [delete <id>]` | | Lista ou apaga prompts agendados |
| `/bashes [kill <id>]` | | Lista ou para comandos em segundo plano |
| `/gain` | | Quanto mais curta a saída de comandos chegou ao modelo |
| `/features [nome on\|off]` | | Lista ou liga/desliga os recursos que podem ser desligados |
| `/trace` | `/console` | Abre o trace ao vivo no navegador |
| `/remote [pair\|off]` | | Controle remoto pelo celular: mostra, convida um celular ou desliga |
| `/trust [yes\|no]` | | Mostra se esta pasta é confiável, ou muda isso |
| `/docs [tópico]` | | Abre esta documentação no navegador, ou busca nela |
| `/factory-reset` | | Apaga tudo o que o {{NAME}} guarda nesta máquina |
| `/<skill> [argumentos]` | | Roda uma skill ou comando personalizado |

## Conversa

### /help

`/help` lista todos os comandos por grupo, os comandos de recursos, `/<skill>` e as teclas principais. `/help <nome>` (com ou sem `/`) mostra um comando ou skill e o que ele faz. Apelido: `/?`.

### /clear

Limpa a conversa e começa uma nova com o mesmo modelo: somem a transcrição, as mensagens na fila, os checkpoints, o título e a sugestão. As pastas adicionadas com `/add-dir` continuam. A conversa antiga segue disponível no `/resume`. Apelidos: `/new`, `/reset`. Não aceita argumentos.

### /resume

`/resume` abre um seletor das conversas anteriores desta pasta, da mais recente para a mais antiga, cada uma com o título (ou `Untitled`) e há quanto tempo foi usada (`2h ago`). `/resume <palavras>` mantém só as conversas cujo título contém todas as palavras, sem diferenciar maiúsculas. A conversa atual não aparece. Escolher uma a continua e mostra um resumo `Where you left off:`. Indisponível enquanto o agente trabalha. Apelido: `/continue`. Veja [Sessões](./sessions#retomar-uma-conversa).

### /fleet

Abre a lista de sessões: primeiro esta conversa, depois as sessões que você deixou trabalhando em segundo plano e, por fim, as conversas anteriores desta pasta. `↑`/`↓` movem, `Enter` troca para a escolhida (uma conversa anterior é retomada; a que você deixa continua trabalhando), `n` começa uma sessão nova, `Esc` fecha. `←` no prompt vazio abre a mesma lista. Apelido: `/sessions`. Não aceita argumentos. Veja [Atalhos de teclado](./keyboard-shortcuts#lista-de-sessoes).

### /fork

Faz a próxima mensagem continuar numa cópia desta conversa, deixando esta como está. Precisa de uma conversa com pelo menos uma mensagem ("Nothing to fork yet: send a message first."). Não aceita argumentos.

### /compact

`/compact` resume a conversa para liberar contexto. `/compact <o que manter>` diz ao resumo no que focar. Indisponível enquanto o agente trabalha. Veja [Sessões](./sessions#compactar-a-conversa).

### /rewind

Abre um seletor **Rewind to before…** com os prompts desta conversa, do mais recente para o mais antigo. Escolher um restaura os arquivos que o agente mudou desde então, corta a conversa para antes daquele prompt e devolve o prompt à entrada para você editar. Alterações feitas com Bash não são desfeitas. Indisponível enquanto o agente trabalha. `Esc` `Esc` com o prompt vazio abre o mesmo seletor. Apelido: `/undo`. Veja [Sessões](./sessions#voltar-atras-rewind).

### /retry

Envia o último prompt de novo, de onde ele começou: volta para antes daquele prompt e então o envia. Se nenhum checkpoint for conhecido, só reenvia o último prompt. Indisponível enquanto o agente trabalha ("Esc stops it, then /retry").

### /btw

`/btw <pergunta>` faz uma pergunta paralela sem interromper o agente e sem entrar na conversa. A resposta aparece no painel; `Esc`, `Enter` ou `Espaço` fecham. Sem pergunta, explica como usar.

### /export

`/export` grava a conversa em Markdown num arquivo na pasta do projeto com o nome do título (`fix-the-login-page.md`), ou `conversation.md` se não houver título. `/export <caminho>` grava nesse caminho, relativo à pasta do projeto. O arquivo tem o título como cabeçalho, seus prompts como citações, as respostas, uma linha por chamada de ferramenta (marcada `(failed)` quando falhou), sub-agentes, planos e diffs; avisos, raciocínio e erros ficam de fora.

### /copy

Copia a última resposta do agente para a área de transferência com `pbcopy`, `wl-copy` ou `xclip`, o primeiro que encontrar.

## Modelo e modo

### /model

| Forma | Efeito |
|---|---|
| `/model` | Abre o seletor de modelos (rótulo e id de cada modelo dos provedores conectados). |
| `/model <id ou nome>` | Troca para esse modelo. |
| `/model roles` | Lista os papéis e o modelo de cada um. |
| `/model <papel>` | Escolhe o modelo de um papel, ou **Session model** para seguir o modelo da sessão. |
| `/model <papel> <modelo>` | Define o modelo de um papel. |
| `/model <papel> default` | Limpa o papel (também `none` ou `off`). |
| `/model fallbacks` | Mostra a cadeia de fallback. |
| `/model fallbacks <modelo> [modelo…]` | Define a cadeia tentada quando um modelo está ocupado, desconectado ou inacessível. |
| `/model fallbacks default` | Limpa a cadeia (também `none` ou `off`). |

Os papéis são `plan` (modo plan e o agente Plan), `explore` (o agente Explore que busca no código), `small` (tarefas de fundo: títulos, escolha de esforço, leitura da web), `review` (o juiz que confere o trabalho pronto) e `advisor` (o conselheiro que o agente consulta). Modelo, papéis e fallbacks ficam guardados para os próximos chats. Veja [Modelos e provedores](./models-and-providers).

### /mode

`/mode` abre um seletor; `/mode <modo>` troca na hora.

| Modo | Também aceito | Significado |
|---|---|---|
| `ask` | `default` | Pergunta antes de edições e comandos |
| `edits` | `acceptEdits`, `accept-edits` | Edita arquivos livremente, pergunta antes de comandos |
| `plan` | | Só lê e planeja |
| `bypass` | `bypassPermissions`, `bypass-permissions`, `yolo` | Nunca pergunta (uma checagem de segurança ainda avalia ações arriscadas) |

`/plan` é o atalho de `/mode plan`. `Shift+Tab` alterna os modos. O modo fica guardado para os próximos chats, a não ser que você tenha passado `--mode` na linha de comando. Veja [Permissões](./permissions).

### /effort

`/effort` abre um seletor; `/effort <nível>` define.

| Nível | Significado |
|---|---|
| `low` | Respostas mais rápidas, menos raciocínio |
| `medium` | Equilibrado |
| `high` | Mais raciocínio para trabalhos difíceis |
| `xhigh` | Raciocínio estendido |
| `max` | Todo o raciocínio que o modelo permite |
| `auto` | Cada pedido escolhe o modelo e o esforço de que precisa; `/effort <nível>` volta a um nível fixo |

O esforço fica guardado para os próximos chats.

### /output-style

`/output-style` abre um seletor com os estilos embutidos; `/output-style <estilo>` define um: `default`, `concise`, `proactive`, `explanatory` ou `learning`. Vale a partir da próxima mensagem e fica guardado. Veja [Estilos de saída](./output-styles).

## Projeto

### /diff

Mostra na conversa as alterações não commitadas do projeto, com arquivos novos não rastreados listados como `● New, not yet in git: …`. Diz `No uncommitted changes.` quando não há nenhuma. Precisa de git.

### /diffs

Abre o navegador de diffs em tela cheia nas alterações não commitadas; `Tab` alterna entre uncommitted, unstaged, staged e branch. As teclas estão em [Chat](./chat#revisar-alteracoes).

### /add-dir

`/add-dir <caminho>` deixa o agente ler uma pasta como o projeto, pelo resto do chat (sobrevive ao `/clear`). O caminho pode ser relativo ao projeto ou começar com `~/`; precisa ser uma pasta existente. `/add-dir` sozinho lista as pastas adicionadas. O equivalente na linha de comando é `--add-dir`.

### /permissions

Mostra as regras de permissão em vigor aqui (`deny`, `ask`, `allow`; deny vence ask, ask vence allow) e as pastas adicionadas. Apelido: `/allowed-tools`. Veja [Permissões](./permissions).

### /hooks

Mostra os hooks que uma execução aqui dispara: evento, matcher (`*` para todos) e o que roda. Veja [Hooks](./hooks).

### /todos

Imprime a lista de tarefas do agente (`☒` feita, `◼` em andamento, `☐` pendente). `Ctrl+T` mostra ou esconde a mesma lista no painel. Apelido: `/tasks`.

### /context

Mostra uma barra de quanto a janela de contexto está cheia, os tokens usados do total, quantos estão livres antes de compactar e a parte de cada item: prompt de sistema, ferramentas, ferramentas MCP, agentes, memória, skills e mensagens.

### /skills

`/skills` lista as skills e comandos personalizados encontrados aqui, como `/nome  descrição`. `/skills new <o que ela faz>` pede ao agente para escrever uma skill nova em `{{DIR}}/skills/<nome>/SKILL.md`. Apelido: `/commands`. Veja [Skills](./skills).

### /agents

`/agents` lista os sub-agentes disponíveis aqui. `/agents new <o que ele faz>` pede ao agente para escrever um novo em `{{DIR}}/agents/<nome>.md`. Veja [Agentes](./agents).

### /mcp

| Forma | Efeito |
|---|---|
| `/mcp` | Lista os servidores MCP que uma sessão aqui pode usar e o estado de cada um. |
| `/mcp approve <nome>` | Aprova um servidor que espera aprovação. |
| `/mcp deny <nome>` | Recusa. |
| `/mcp login <nome>` | Entra num servidor pelo navegador. |
| `/mcp logout <nome>` | Sai. |

As mudanças valem a partir da próxima mensagem. Veja [MCP](./mcp).

### /reload

Relê `~/{{DIR}}` e o projeto e informa o que encontrou: plugins, quantas skills e agentes, e servidores MCP prontos, esperando aprovação ou desligados.

## App

### /cost

Mostra quanto esta sessão custou e, por modelo, os tokens de entrada e saída, leituras de cache (com a taxa de acerto) e escritas, buscas na web e o preço. Modelos sem preço conhecido (uma assinatura cobra pelo plano) mostram `not priced`. Apelido: `/usage`. Veja [Orçamento e custo](./budget-and-cost).

### /status

Mostra a versão, o modelo (rótulo e id), modo, esforço, pasta, id da sessão, pastas adicionadas e o custo até agora.

### /config

Abre o seletor **Settings**: Model, Permission mode, Effort, Output style, Vim keys, Status line, Budget, Features e Verification, cada um com o valor atual. Escolher um abre o comando correspondente. Apelido: `/settings`. Para os arquivos de configuração em si, veja [Configurações](./settings).

### /statusline

`/statusline <comando>` desenha a linha de status com a primeira linha que o seu comando imprime; o comando recebe o estado da sessão em JSON pela entrada padrão e roda a cada poucos segundos. `/statusline off` volta à linha de sempre. `/statusline` sozinho diz qual está em uso. Veja [Linha de status](./statusline).

### /vim

Liga ou desliga as teclas do vim no prompt; fica guardado. Veja [Atalhos de teclado](./keyboard-shortcuts#modo-vim).

### /login

Abre o seletor **Connect a provider**. Provedores com chave de API pedem a chave num campo mascarado; assinaturas entram pelo navegador ou com um código. Veja [Contas](./accounts).

### /logout

Abre um seletor dos provedores conectados e esquece as credenciais do escolhido.

### /accounts

| Forma | Efeito |
|---|---|
| `/accounts` | Escolhe qual conta de um provedor é usada (a atual aparece como `in use`); vale a partir da próxima mensagem. |
| `/accounts remove` | Escolhe uma conta para esquecer. |
| `/accounts balance` | Lista as formas de as contas dividirem o trabalho. |
| `/accounts balance <forma>` | Define para o provedor do modelo atual: `off` (só a conta em uso), `failover` (as outras quando ela está ocupada ou desconectada) ou `round-robin` (cada execução começa na próxima conta). |

Veja [Contas](./accounts).

### /doctor

Verifica se esta instalação consegue trabalhar nesta pasta, como `{{COMMAND}} doctor`.

### /terminal-setup

Explica como fazer `Shift+Enter` quebrar linha: `Alt+Enter` ou `Ctrl+J` funcionam em qualquer terminal; para `Shift+Enter`, use um terminal que informe a tecla (Ghostty, kitty, WezTerm, iTerm2 com "Report modifiers using CSI u") ou mapeie-a para enviar `\n`.

### /exit

Sai do chat. Apelido: `/quit`.

## Recursos

Estes comandos vêm dos módulos de recursos do chat. Alguns dependem de um recurso estar ligado; veja [Recursos experimentais](./experimental-features).

### /branch

`/branch <n>` começa uma conversa nova a partir de antes do prompt número `n` desta (numerados do mais antigo para o mais novo), mantendo esta: a conversa é cortada ali, o prompt volta para a entrada e a original continua no `/resume`. Indisponível enquanto o agente trabalha. Precisa do recurso `branches` (`/features branches on`).

> [!NOTE]
> `/branch` sem argumento lista os prompts numerados de onde você pode ramificar. Veja [Branches e git](./branches-and-git).

### /budget

| Forma | Efeito |
|---|---|
| `/budget` | Mostra quanto a sessão gastou e o limite. |
| `/budget <dólares>` | Define o limite, por exemplo `/budget 5` ou `/budget $5`: as chamadas ao modelo param quando a sessão gastar isso. |
| `/budget off` | Remove o limite (também `none`). |

A linha de status mostra `$gasto/$limite` enquanto houver orçamento. Veja [Orçamento e custo](./budget-and-cost).

### /verification

| Forma | Efeito |
|---|---|
| `/verification` | Mostra os critérios em vigor e de onde vêm (seus, do plano ou do projeto). |
| `/verification <critérios>` | Julga cada execução contra eles antes de terminar; um agente juiz roda as checagens. |
| `/verification off` | Limpa os seus critérios (os de um plano continuam valendo). |

A linha de status mostra `verify (yours)`, `verify (plan)` ou `verify (project)`. Veja [Verificação e revisão](./verification-and-review).

### /memory

| Forma | Efeito |
|---|---|
| `/memory` | Lista as memórias do projeto com seus ids. |
| `/memory <palavras>` | Busca nelas, com uma pontuação por resultado. |
| `/memory optimize` | Pede ao modelo que organize a memória (também `optimise`); precisa de um modelo. |
| `/memory undo` | Devolve a memória ao estado de antes da última mudança. |
| `/memory forgotten` | Lista as memórias esquecidas (restauráveis por 30 dias). |
| `/memory restore <id>` | Restaura uma memória esquecida. |
| `/memory share <id>` | Grava uma memória num arquivo para commitar, para os agentes do time também lembrarem dela. |
| `/memory edit` | Abre o {{INSTRUCTIONS}} do projeto (ou `{{DIR}}/{{INSTRUCTIONS}}`) no seu editor. |
| `/memory seed` | Oferece coisas que você pediu em duas ou mais sessões passadas. Precisa de `experimental-memory-seed`. |
| `/memory seed keep <n> [<n>…]` | Guarda os itens oferecidos com esses números. |
| `/memory score` | Mostra o placar da memória. |

Um prompt começando com `#` salva uma nota na memória. Veja [Memória](./memory).

### /learn

Depois de uma execução em que você corrigiu o agente, ele pode oferecer uma lição. `/learn keep` guarda (como memória, regra de permissão ou linha no {{INSTRUCTIONS}}); `/learn skip` descarta. Veja [Aprendizado](./learning).

### /check

Pede ao agente que confira as alterações desta branch contra as memórias sobre os arquivos que elas tocam. Precisa de git e do recurso `experimental-learning-check`.

### /rules

`/rules` lista regras de permissão sugeridas pelas suas respostas passadas, numeradas, com quantas vezes você permitiu ou negou cada uma. `/rules keep <n>` salva uma em `{{DIR}}/settings.local.json`. Veja [Permissões](./permissions).

### /loop

`/loop [intervalo] <tarefa>` roda a tarefa agora e de novo depois. Com um intervalo (`30s`, `5m`, `2h`, `1d`) o agente a agenda para se repetir nesse intervalo; sem intervalo, o agente volta a ela quando achar melhor, a menos que a tarefa esteja concluída de vez. Veja [Agendamentos](./schedules).

### /schedules

`/schedules` lista os prompts agendados (id, quando, a espera e o prompt). `/schedules delete <id>` remove um. Veja [Agendamentos](./schedules).

### /bashes

`/bashes` lista os comandos deixados rodando em segundo plano (id, rodando ou terminado, o comando). `/bashes kill <id>` para um deles.

### /gain

Mostra quanto mais curta a saída de comandos chegou ao modelo nesta sessão. Precisa do recurso `experimental-output-savings`.

### /features

`/features` lista os recursos que você pode desligar, cada um `on` ou `off` com o que faz (e `(not on this system)` quando não está disponível). `/features <nome> on|off` liga ou desliga um; fica salvo nas suas configurações e as próximas execuções seguem. Também dá para chegar pelo `/config`. Veja [Recursos experimentais](./experimental-features).

### /trace

Abre no navegador o trace ao vivo de todas as sessões: execuções, spans, custo, esperas e checagens. Apelido: `/console`. Veja [Console de trace](./trace-console).

### /remote

`/remote` mostra o relay, o id desta máquina, os celulares pareados e se este chat está conectado ao relay. `/remote pair` imprime um QR code e um link que pareiam um celular em até dez minutos, e conecta este chat ao relay. `/remote off` desliga até o próximo `/remote pair` ou a próxima abertura. Com um relay definido e um celular pareado, um chat novo se conecta sozinho; só um chat da máquina ocupa o relay por vez, e os outros avisam. Veja [`{{COMMAND}} remote`](./cli-reference#remote).

### /trust

`/trust` mostra se a pasta do projeto é confiável e lista o que ela traz que depende de confiança: hooks, scripts, servidores MCP, provedores declarados, concessões no `settings.local.json`. `/trust yes` confia na pasta e `/trust no` deixa de confiar. A mudança vale a partir da próxima execução. Veja [Confiança na pasta](./sandbox-and-safety#confianca-na-pasta).

### /docs

`/docs` abre esta documentação no navegador, servida pelo {{NAME}} nesta máquina. `/docs <tópico>` abre já buscando esse tópico, por exemplo `/docs hooks`. O chat mostra o endereço que abriu.

### /factory-reset

Pergunta antes de apagar tudo o que o {{NAME}} guarda nesta máquina (sessões, memória, logins, configurações, skills e agentes nas pastas de dados e em `~/{{DIR}}`), depois sai e informa o que foi apagado. O programa continua instalado e as pastas dos projetos não são tocadas. A resposta segura, **No, keep everything**, é o padrão.

> [!CAUTION]
> Um factory reset não pode ser desfeito.

## Skills e comandos personalizados

Qualquer `/nome` que não seja um dos comandos acima roda a skill ou o comando personalizado com esse nome. O resto da linha vira os argumentos: eles substituem `$ARGUMENTS` no texto da skill e também são acrescentados como `Arguments: …`.

```text
/review-pr 123
/acme:triage o bug do login
```

| Origem | Vira |
|---|---|
| `{{DIR}}/skills/<nome>/SKILL.md` (ou `SKILL.js`) no projeto, nas pastas acima dele ou em `~/{{DIR}}` | `/<nome>` (ou o `name` do frontmatter) |
| `{{DIR}}/commands/<nome>.md` (ou `.js`) | `/<nome>`; um arquivo numa subpasta, `commands/git/push.md`, vira `/git:push` |
| Skills e comandos de um plugin | `/<plugin>:<nome>` |
| Prompts de um servidor MCP | `/mcp__<servidor>__<prompt>` (veja [MCP](./mcp)) |

Eles aparecem nas sugestões do `/` e no `/skills`. `/help <nome>` mostra a descrição de uma skill. Veja [Skills](./skills) e [Plugins](./plugins).
