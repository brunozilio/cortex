---
title: Feature flags
description: Todas as feature flags que o {{NAME}} conhece, o que cada uma muda e como ligá-las ou desligá-las com /features, /config ou {{COMMAND}} config.
---

O {{NAME}} traz um conjunto de funcionalidades com nome que você pode ligar ou desligar uma a uma: proteções de segurança, formas de enxugar a saída de comandos e de páginas web, revisores que conferem o trabalho do agente, ajudas de memória e mais. Cada uma é uma **feature flag** com um nome fixo, como `budget` ou `experimental-sandbox`.

> [!IMPORTANT]
> Toda flag fica **ligada** até você desligá-la, inclusive as que começam com `experimental-`. Uma flag que você nunca mexeu está ligada, inclusive a [`experimental-code-mode`](#experimental-code-mode), que faz da REPL a única ferramenta do agente.

## Ligar ou desligar uma flag

### No chat

Digite `/features` para listar todas as flags com o estado e uma descrição de uma linha:

```text
Features (/features <name> on|off):
  on   experimental-sandbox
       Confine Bash and Monitor commands to the project's folders (macOS; Linux with Landlock)
  on   experimental-output-cleanup
       Bash runs commands without colours and drops escape codes, redrawn progress and repeated lines
  ...
```

Troque uma pelo nome exato seguido de `on` ou `off`:

```text
/features experimental-follow-through off
/features budget on
```

A mudança é salva no seu arquivo de configurações de usuário, `~/{{DIR}}/settings.json`, e o chat responde `Saved in your settings; new runs follow it.` Uma execução já em andamento mantém as flags com que começou; o próximo prompt usa o novo estado.

O `/features` recusa um nome que não conhece (`<name> is not a feature; /features lists them`) e qualquer outra forma de argumento (`Use /features to list them, or /features <name> on|off.`).

Uma flag que não funciona no seu sistema continua na lista, marcada `(not on this system)`. Hoje isso vale só para `experimental-sandbox` em plataformas sem o sandbox.

O `/config` (também `/settings`) abre o seletor de configurações; a entrada **Features** mostra a mesma lista.

### Pela linha de comando

O `{{COMMAND}} config` edita os arquivos de configurações sem abrir o chat. As flags ficam sob a chave `features`, então use o caminho com pontos `features.<nome>`:

```bash
{{COMMAND}} config set features.experimental-sandbox false
{{COMMAND}} config get features.experimental-sandbox
{{COMMAND}} config unset features.experimental-sandbox
```

Sem `--project`, o comando escreve no seu arquivo de usuário, `~/{{DIR}}/settings.json`. Com `--project`, escreve no `{{DIR}}/settings.local.json` deste projeto:

```bash
{{COMMAND}} config set --project features.budget false
```

`true` e `false` são lidos como booleanos JSON. Veja [Configurações](./settings) para o resto do comando `config`.

### Em um arquivo de configurações

Escreva as flags como um objeto de booleanos sob `features`:

```json
{
  "features": {
    "experimental-follow-through": false,
    "experimental-sandbox": true
  }
}
```

Um valor que não seja `true` ou `false` é um erro: `features.<name> must be true or false`.

## Quem pode mudar uma flag

O {{NAME}} lê os arquivos de configurações da sua pasta pessoal até a pasta do projeto (veja [Configurações](./settings)). Para as flags, os arquivos se dividem em dois grupos:

| Arquivo | De quem é | O que pode fazer com uma flag |
|---|---|---|
| `~/{{DIR}}/settings.json` | Seu | Ligar ou desligar qualquer flag |
| `{{DIR}}/settings.local.json` (no projeto ou numa pasta acima) | Seu | Ligar ou desligar qualquer flag |
| `~/{{DIR}}/managed-settings.json` | Seu | Ligar ou desligar qualquer flag; lido por último |
| `{{DIR}}/settings.json` versionado em um projeto (ou numa pasta acima) | Do projeto | Só deixar as coisas mais restritas |

Um arquivo versionado do projeto só pode deixar as coisas mais restritas:

- pode **desligar** uma flag comum, mas não religar uma flag que você desligou;
- pode **ligar** uma flag de **segurança**, mas não desligá-la.

As flags de segurança são `experimental-sandbox`, `redact-secrets` e `experimental-dependency-guard`. Assim um repositório pode exigir o sandbox de todo mundo que trabalha nele, enquanto um `"redact-secrets": false` perdido em um repositório clonado não tem efeito.

Os arquivos são lidos em ordem, e um arquivo seu lido depois prevalece. O `/features` escreve em `~/{{DIR}}/settings.json`, que é lido primeiro; então um `settings.local.json` ou `managed-settings.json` que define a mesma flag ainda decide.

> [!TIP]
> Para desligar uma flag só em um projeto, use `{{COMMAND}} config set --project features.<nome> false`. Isso escreve no `settings.local.json` do projeto, que é seu e não vai para o repositório.

## Flags estáveis e experimentais

Uma flag cujo nome começa com `experimental-` é experimental: o comportamento ainda pode mudar ou ser removido. As outras são estáveis. Os dois tipos começam ligados.

As funcionalidades experimentais já vêm ligadas para que sejam usadas. Cada uma caminha para um de dois fins: depois de rodar no uso diário sem nenhum problema relatado, vira parte permanente do {{NAME}} (a flag some, ou vira uma configuração comum), ou é removida. Até lá, você pode desligar qualquer uma com `/features <nome> off`.
## Visão geral das flags

| Flag | Área | Segurança | O que faz |
|---|---|---|---|
| `experimental-sandbox` | Segurança | Sim | Confina os comandos de Bash e Monitor às pastas do projeto (macOS; Linux com Landlock) |
| `redact-secrets` | Segurança | Sim | Esconde tokens, chaves e valores do `.env` na saída das ferramentas |
| `destructive-snapshot` | Segurança | | Guarda arquivos antes de `rm -r`, `git clean`, `git reset --hard` e afins para o `/rewind` poder trazê-los de volta |
| `experimental-dependency-guard` | Segurança | Sim | Pergunta antes de instalar um pacote inexistente, recém-publicado ou parecido com outro |
| `suggested-rules` | Permissões | | O `/rules` sugere regras de permissão a partir das respostas que você repete |
| `experimental-output-cleanup` | Saída de comandos | | Remove cores, códigos de escape, progresso redesenhado e linhas repetidas |
| `experimental-output-filters` | Saída de comandos | | Esconde testes que passaram, passos de build e progresso de ferramentas comuns |
| `experimental-output-savings` | Saída de comandos | | O `/gain` mostra quanto a saída dos comandos encolheu |
| `experimental-web-main-content` | Web | | O WebFetch põe o conteúdo principal da página primeiro |
| `experimental-web-cleanup` | Web | | O WebFetch descarta partes ocultas, botões e sequências de links de menu |
| `experimental-web-links` | Web | | O WebFetch numera os links e lista os endereços no fim |
| `experimental-web-small-pages-direct` | Web | | O WebFetch entrega páginas curtas direto ao agente |
| `experimental-web-formats` | Web | | O WebFetch pede Markdown e compacta JSON e feeds |
| `experimental-web-empty-page-error` | Web | | O WebFetch trata uma página quase sem texto como erro |
| `budget` | Custo | | O `/budget` para as chamadas de modelo de uma sessão num teto de gasto |
| `cache-warning` | Custo | | A linha de status faz a contagem regressiva dos dois últimos minutos do cache de prompt |
| `branches` | Git e sessões | | O `/branch <n>` começa uma conversa nova a partir de antes de um prompt anterior |
| `git-offer` | Git e sessões | | Oferece iniciar o git numa pasta que não é repositório |
| `experimental-session-claims` | Git e sessões | | Avisa o agente quando outra sessão editou o mesmo arquivo |
| `experimental-pr-follow` | Git e sessões | | Habilita a skill `follow-pr` |
| `auto-review` | Revisão | | Um revisor somente leitura confere as mudanças antes de a execução terminar |
| `project-criteria` | Revisão | | Julga as execuções pelo `{{DIR}}/verification.md` |
| `experimental-follow-through` | Revisão | | Manda o agente de volta quando ele termina prometendo mais trabalho |
| `experimental-affected-tests` | Revisão | | Roda em segundo plano os testes que cobrem um arquivo editado |
| `experimental-escalation` | Revisão | | Manda um agente empacado para o Advisor ou para o `escalationModel` |
| `experimental-code-mode` | Ferramentas | | A REPL vira a única ferramenta do agente, que programa todas as outras em JavaScript |
| `experimental-load-on-call` | Ferramentas | | Uma ferramenta guardada para busca que o agente chama pelo nome é carregada e executada |
| `experimental-learning` | Aprendizado e memória | | Oferece uma lição para guardar depois de uma execução em que você corrigiu o agente |
| `team-memory` | Aprendizado e memória | | Lembra as memórias versionadas em `{{DIR}}/memory` |
| `experimental-memory-anchors` | Aprendizado e memória | | Memórias citam seus arquivos e aparecem quando esses arquivos são tocados |
| `experimental-structured-learnings` | Aprendizado e memória | | Decisões são guardadas como decidido, descartado e cuidado |
| `experimental-learning-check` | Aprendizado e memória | | O `/check` compara as mudanças do branch com as memórias sobre elas |
| `experimental-memory-seed` | Aprendizado e memória | | O `/memory seed` oferece o que você pediu em várias sessões |
| `experimental-memory-scorecard` | Aprendizado e memória | | Uma em cada 20 requisições roda sem memória, para o `/memory score` |
| `experimental-code-map` | Inteligência de código | | Aponta cada prompt para o código correspondente a partir de um índice tree-sitter |
| `experimental-blast-radius` | Inteligência de código | | Diz ao agente quais outros arquivos usam o que uma edição mudou |
| `memory-search` | Funcionalidades internas | | A ferramenta `memory_search` que o agente usa para buscar na memória |
| `schedules` | Funcionalidades internas | | `/schedules`, `/loop` e os prompts agendados |
| `plugins` | Funcionalidades internas | | Instalar, atualizar, listar e remover plugins |
| `trace` | Funcionalidades internas | | Os dados das páginas do `/trace` e do `/console` |
| `channels` | Funcionalidades internas | | Servidores MCP que ficam abertos e mandam mensagens ao agente |
| `declared-providers` | Funcionalidades internas | | Carrega os provedores de modelo que arquivos de configuração declaram |

As seções abaixo descrevem cada flag em detalhe.

## Segurança

### `experimental-sandbox`

> [!EXPERIMENTAL]
> Flag de segurança. Padrão: ligada. Disponível só onde o sandbox é suportado (macOS; Linux 5.13 ou mais novo com Landlock); nos outros sistemas o `/features` a mostra como `(not on this system)`.

Confina os comandos que o agente roda com **Bash** e **Monitor** às pastas do projeto, usando as configurações de `sandbox`. Os comandos que o agente inicia para rodar os testes de um arquivo (veja [`experimental-affected-tests`](#experimental-affected-tests)) são confinados do mesmo jeito. Com a flag desligada, os comandos rodam sem sandbox.

Veja [Sandbox e segurança](./sandbox-and-safety) para o que o sandbox permite, como configurá-lo e como o agente pede para sair dele. O `{{COMMAND}} doctor` informa se o sandbox está ligado e disponível.

### `redact-secrets`

> [!NOTE]
> Flag de segurança. Padrão: ligada.

Esconde segredos no que as ferramentas devolvem antes que o modelo e o log da sessão os vejam. Troca por `[redacted]`:

- tokens que começam com `sk-`, `sk_`, `ghp_`, `gho_`, `ghs_`, `ghu_`, `github_pat_`, `glpat-` (20 caracteres ou mais), ou `xoxb-`, `xoxp-`, `xoxa-`, `xoxs-` (10 ou mais);
- ids de chave de acesso da AWS (`AKIA…` ou `ASIA…`, 20 caracteres) e JSON Web Tokens (`eyJ…` com três partes);
- o valor depois de uma chave cujo nome termina em `password`, `passwd`, `secret`, `token`, `api_key`, `apikey` ou `api-key` (como em `API_KEY=…` ou `token: …`), e a palavra depois de `Bearer`;
- todo valor de 8 caracteres ou mais, sem espaços, encontrado nos arquivos `.env` e `.env.*` do projeto (mas não em `*.example` ou `*.sample`).

A ocultação vale para os resultados das ferramentas, para as linhas de progresso ao vivo dos comandos, para os resumos de ferramentas mostrados no chat e para os arquivos de saída completa guardados pela [`experimental-output-filters`](#experimental-output-filters). Os subagentes herdam a regra.

### `destructive-snapshot`

> [!NOTE]
> Padrão: ligada.

Antes de o Bash rodar um comando que destrói arquivos, o {{NAME}} guarda uma cópia deles para o [`/rewind`](./sessions) poder trazê-los de volta. Os comandos reconhecidos são:

- `rm` com caminhos (todos os arquivos sob eles, recursivamente);
- `git clean` (arquivos não rastreados; também os ignorados com `-x` ou `-X`);
- `git reset --hard`, `git checkout .` ou `git checkout -- …`, `git restore .` ou `git restore -- …`, `git stash -u`/`--include-untracked`/`-a`/`--all` (arquivos não rastreados do repositório).

São guardados no máximo 5.000 arquivos e 200 MB por comando. Quando algo ficou de fora, o resultado da ferramenta diz `(before this command N files were kept for /rewind; M more were too many or too large to keep)`.

### `experimental-dependency-guard`

> [!EXPERIMENTAL]
> Flag de segurança. Padrão: ligada.

Antes de um comando Bash instalar pacotes, o {{NAME}} consulta cada um no seu registro e pergunta a você primeiro quando algum parece errado. Ele reconhece:

| Ecossistema | Comandos |
|---|---|
| npm | `npm`, `pnpm` ou `bun` com `add`, `install` ou `i`; `yarn add` |
| crates.io | `cargo add` |
| PyPI | `pip`, `pip3` ou `pipx install`; `uv pip install`; `uv add`; `poetry add`; `python -m pip install` |

Caminhos locais, URLs e arquivos de requisitos (`-r requirements.txt`) não são consultados. Um pacote levanta suspeita quando:

- o registro responde que ele não existe (`<name> is not published in its registry: the name may be made up`);
- o nome está a uma letra de distância de um pacote popular, ou troca duas letras dele (`<name> looks like the popular <known> with a typo`);
- ele foi publicado pela primeira vez há menos de 30 dias (`<name> was first published N days ago`).

O pedido de permissão então começa com `Check the package first: …`. Numa execução que não pode perguntar (como o [modo headless](./headless)), o comando é recusado com `Not run: … Ask the user before installing it.` Uma consulta que falha por outro motivo não levanta nada.

## Permissões

### `suggested-rules`

> [!NOTE]
> Padrão: ligada.

Habilita o `/rules`, que lê as respostas de permissão que você deu nas sessões recentes deste projeto e sugere regras para as que você repete:

- uma regra **allow** depois de você permitir o mesmo pedido 5 vezes sem nunca negar;
- uma regra **deny** depois de você negar 3 vezes sem nunca permitir.

Regras que você já tem não são sugeridas.

```text
/rules
Your answers suggest these rules (/rules keep <n> saves one):
  1. allow Bash(cargo test:*)  (you allowed it 7 times)
  2. deny Bash(git push:*)  (you denied it 3 times)

/rules keep 1
Kept Bash(cargo test:*) as a allow rule in {{DIR}}/settings.local.json.
```

Desligar a flag também impede o [`/learn keep`](./learning) de salvar uma regra ou uma linha de instrução, porque as duas passam pela mesma funcionalidade. Vale na hora, sem reiniciar: com a flag desligada, o `/rules` e o `/learn keep` respondem `suggested_rules is turned off (/features suggested-rules on)`.

## Saída de comandos

Estas três flags mudam a saída dos comandos que o agente roda com o Bash antes de ela chegar ao modelo.

### `experimental-output-cleanup`

> [!EXPERIMENTAL]
> Padrão: ligada.

Os comandos rodam com `NO_COLOR=1`, `CLICOLOR=0`, `CARGO_TERM_COLOR=never`, `CARGO_TERM_PROGRESS_WHEN=never` e `PY_COLORS=0`. A saída perde os códigos de escape do terminal e os backspaces, mantém só o último redesenho de uma linha reescrita com retorno de carro, junta sequências de linhas em branco numa só e mostra uma vez só a linha repetida 3 vezes ou mais, como `<linha> (×N)`.

### `experimental-output-filters`

> [!EXPERIMENTAL]
> Padrão: ligada.

Esconde o ruído de ferramentas conhecidas e guarda a saída completa num arquivo que o agente pode ler:

| Filtro | Vale para | Esconde |
|---|---|---|
| `cargo` | `cargo …` | `Compiling`, `Checking` e outros passos de build, linhas `test … ok`; agrupa avisos repetidos |
| `git` | `git …` | Linhas de progresso (`Counting objects:`…) e dicas `(use "git …")` |
| `javascript` | `npm`, `pnpm`, `yarn`, `bun`, `jest`, `vitest` | Linhas de teste que passou (começando com `✓`, `√`, `✔` ou `PASS`), linhas `Progress: resolved` e linhas só de sinais `+` |
| `python` | `pytest`, `py.test`, `pip`, `pip3`, `uv pip` | Linhas de teste `PASSED`, linhas de progresso feitas só de pontos e passos do pip (`Collecting`, `Downloading`, `Using cached`, `Requirement already satisfied:`…) |
| `go test` | `go test` | `=== RUN`, `=== PAUSE`, `=== CONT`, `--- PASS:` |

Um filtro só é usado quando encurta a saída em pelo menos 400 caracteres. A saída encurtada termina com um rodapé como:

```text
[37 lines of cargo noise hidden (passing tests, progress, build steps, repeated warnings); the whole output is in /…/bash-1234-1.txt]
```

Falhas, erros e resumos nunca são escondidos.

### `experimental-output-savings`

> [!EXPERIMENTAL]
> Padrão: ligada.

Mede quanto a saída de cada comando encolheu com a limpeza e os filtros, e habilita o `/gain` para mostrar isso. Veja [Orçamento e custo](./budget-and-cost#ver-quanta-saida-foi-economizada).

## Web

Estas flags mudam como o **WebFetch** lê uma página. Veja [Sandbox e segurança](./sandbox-and-safety) para como as páginas buscadas ficam separadas das instruções.

### `experimental-web-main-content`

> [!EXPERIMENTAL]
> Padrão: ligada.

Quando a página tem uma área de conteúdo principal, o WebFetch a escreve primeiro e põe o resto (menus, cabeçalho, rodapé) depois de uma linha `--- around the main content ---`. Desligada, a página é lida na ordem dela.

### `experimental-web-cleanup`

> [!EXPERIMENTAL]
> Padrão: ligada.

Descarta elementos ocultos e botões, rótulos de link repetidos em todos os itens de uma lista e longas sequências de links de menu (mantendo os 8 primeiros de cada sequência). Desligada, tudo isso fica.

### `experimental-web-links`

> [!EXPERIMENTAL]
> Padrão: ligada.

Numera os links do texto como `[1]`, `[2]`… e lista cada endereço uma vez numa seção `Links` no fim, escrevendo os endereços do mesmo site como caminhos. Quando uma página longa é cortada, a lista mantém só os links citados no texto que sobrou. Desligada, cada endereço vai entre colchetes ao lado do texto.

### `experimental-web-small-pages-direct`

> [!EXPERIMENTAL]
> Padrão: ligada.

Uma página de 6.000 caracteres ou menos vai inteira para o agente, cercada como conteúdo não confiável. Páginas mais longas, e todas as páginas com a flag desligada, vão para um modelo leitor separado (o `smallModel`, se configurado, senão o modelo da sessão), que responde o que o agente procurava.

### `experimental-web-formats`

> [!EXPERIMENTAL]
> Padrão: ligada.

O WebFetch pede `text/markdown` primeiro (depois texto puro, HTML e JSON), compacta JSON em uma linha por registro e lê feeds RSS e Atom como uma lista de entradas. Desligada, não envia cabeçalho `Accept` e trata JSON como texto puro.

### `experimental-web-empty-page-error`

> [!EXPERIMENTAL]
> Padrão: ligada.

Quando uma página HTML não rende texto legível, ou rende menos de 400 caracteres de texto em mais de 10.000 caracteres de marcação, o WebFetch devolve um erro dizendo que a página provavelmente é montada por JavaScript ou está atrás de um aviso de consentimento, um login ou uma checagem de robôs, em vez de uma página quase vazia.

## Custo

### `budget`

> [!NOTE]
> Padrão: ligada.

Permite que o `/budget <dólares>` limite o gasto de uma sessão: quando a sessão gasta o orçamento, a próxima chamada de modelo é recusada. Com a flag desligada, as execuções ignoram o teto mesmo que você defina um. Veja [Orçamento e custo](./budget-and-cost#limitar-o-gasto-de-uma-sessao).

### `cache-warning`

> [!NOTE]
> Padrão: ligada.

Acrescenta à linha de status uma contagem regressiva do cache de prompt enquanto o agente está parado. Veja [Orçamento e custo](./budget-and-cost#manter-o-cache-de-prompt-quente).

## Git e sessões

### `branches`

> [!NOTE]
> Padrão: ligada.

Habilita o `/branch <n>`, que começa uma conversa nova a partir de antes do n-ésimo prompt desta. Desligada, o comando responde `Branches are turned off in your settings (/features branches on).` Veja [Branches e git](./branches-and-git#ramificar-a-partir-de-um-prompt-anterior).

### `git-offer`

> [!NOTE]
> Padrão: ligada.

Na primeira mensagem de uma conversa nova numa pasta que não é repositório git, o {{NAME}} pergunta se deve criar um. Responder **No, and don't offer again** desliga esta flag. Veja [Branches e git](./branches-and-git#inicializar-o-git-em-uma-pasta-nova).

### `experimental-session-claims`

> [!EXPERIMENTAL]
> Padrão: ligada.

Quando o agente edita ou escreve um arquivo que outra sessão ativa na mesma pasta mudou há menos de 10 minutos, o resultado da ferramenta o avisa para reler o arquivo e falar com você se os dois precisarem dele. Veja [Branches e git](./branches-and-git#varias-sessoes-na-mesma-pasta).

### `experimental-pr-follow`

> [!EXPERIMENTAL]
> Padrão: ligada.

Deixa disponível a skill embutida `follow-pr`. Desligada, a skill sai da lista que o agente vê. Veja [Branches e git](./branches-and-git#acompanhar-um-pull-request-ate-o-merge).

## Revisão

### `auto-review`

> [!NOTE]
> Padrão: ligada.

Quando uma execução mudou arquivos, um revisor independente e somente leitura confere as mudanças, e a documentação que as descreve, antes de a execução terminar. Achados graves mandam o agente de volta uma vez. Veja [Verificação e revisão](./verification-and-review#revisao-de-codigo-automatica).

### `project-criteria`

> [!NOTE]
> Padrão: ligada.

Quando você não define critérios próprios, toda execução é julgada pelo `{{DIR}}/verification.md` versionado no projeto. Veja [Verificação e revisão](./verification-and-review#criterios-do-projeto).

### `experimental-follow-through`

> [!EXPERIMENTAL]
> Padrão: ligada.

Quando a última resposta do agente termina prometendo mais trabalho ("agora vou rodar os testes"), um modelo pequeno confere se o trabalho ficou por fazer e, se ficou, manda o agente de volta para fazê-lo, no máximo duas vezes por execução. Veja [Verificação e revisão](./verification-and-review#follow-through).

### `experimental-affected-tests`

> [!EXPERIMENTAL]
> Padrão: ligada.

Depois que o agente edita ou escreve um arquivo, os testes que o cobrem começam em segundo plano e o resultado chega ao agente quando terminam. Veja [Verificação e revisão](./verification-and-review#testes-afetados).

### `experimental-escalation`

> [!EXPERIMENTAL]
> Padrão: ligada.

Quando o agente falha a mesma chamada repetidas vezes, ele é orientado a consultar o Advisor (quando o `advisorModel` está definido) ou os próximos turnos rodam no `escalationModel`. Veja [Verificação e revisão](./verification-and-review#escalonamento-quando-o-agente-empaca).

## Ferramentas

### `experimental-code-mode`

> [!EXPERIMENTAL]
> Padrão: ligada. Desligue com `/features experimental-code-mode off` para devolver as ferramentas ao agente.

Faz da **REPL** a única ferramenta do agente, além das chaves de plan mode e de worktree; todas as outras ferramentas, inclusive a de fazer perguntas a você, aparecem na descrição da REPL como funções tipadas e são chamadas de dentro dos scripts. As ferramentas guardadas para busca (quando há muitas ferramentas MCP) são encontradas num script com `searchTools("palavras")`, e `describeTool(nome)` dá a declaração de uma ferramenta. Em vez de chamar uma ferramenta por turno, o agente escreve um script em JavaScript em que cada uma das outras ferramentas é uma função (`await Grep({ pattern: "TODO" })`), faz laços e filtros no código e lê só o que o script devolve. Buscar em muitos arquivos ou fazer a mesma edição em muitos lugares passa a custar um turno, e os arquivos que ele só precisava filtrar nunca entram na conversa.

Cada chamada de um script é uma chamada de ferramenta comum: segue o seu modo de permissão, as regras e os hooks, pergunta a você como a ferramenta perguntaria e aparece no chat dentro do bloco **REPL** do próprio script, como as chamadas de um sub-agente. Uma chamada recusada por uma regra falha dentro do script e é listada depois da saída dele; uma chamada a que você responde Não para o script ali. O que um script declara fica para o próximo na mesma sessão até o {{NAME}} fechar. Um script que ainda roda depois de uns 30 segundos (um build, uma suíte de testes) segue em segundo plano, no mesmo contexto, enquanto o agente continua trabalhando; o resultado chega ao agente como o de um agente em segundo plano, no máximo antes de o turno terminar. Desligue a flag para devolver as ferramentas ao agente. Ela substitui a `experimental-code-mode-only`, fundida a ela em 2026-11-17; uma configuração que ainda cita a flag antiga é ignorada.

### `experimental-load-on-call`

> [!EXPERIMENTAL]
> Padrão: ligada.

Com muitas ferramentas MCP, a maioria fica guardada atrás da ToolSearch e o agente precisa carregá-las antes de chamar. Com esta flag, quando o agente chama pelo nome uma ferramenta guardada sem ter carregado, ela é carregada e executada na hora, em vez de voltar um erro pedindo a ToolSearch. A partir daí ela fica oferecida como as carregadas pela busca. Permissões, regras e hooks valem como antes. Desligada, a chamada é recusada até o agente carregar a ferramenta.

## Aprendizado e memória

### `experimental-learning`

> [!EXPERIMENTAL]
> Padrão: ligada.

Depois de uma execução em que você corrigiu o agente pelo menos duas vezes, o {{NAME}} propõe uma lição para guardar (uma memória, uma skill, uma regra de permissão ou uma linha de instrução), que o `/learn keep` salva. Veja [Aprendizado](./learning).

### `team-memory`

> [!NOTE]
> Padrão: ligada.

Lembra as memórias versionadas na pasta `{{DIR}}/memory` do projeto junto com as suas. O `/memory share <id>` escreve uma das suas lá. Veja [Memória](./memory#memoria-da-equipe).

### `experimental-memory-anchors`

> [!EXPERIMENTAL]
> Padrão: ligada.

As memórias podem citar os arquivos de que tratam; quando o agente lê, edita ou escreve um desses arquivos, as memórias sobre ele são mostradas. Veja [Memória](./memory#memorias-ligadas-a-arquivos).

### `experimental-structured-learnings`

> [!EXPERIMENTAL]
> Padrão: ligada.

Decisões são guardadas como o que foi decidido, o que foi tentado e descartado e com o que tomar cuidado. Veja [Aprendizado](./learning#decisoes-estruturadas).

### `experimental-learning-check`

> [!EXPERIMENTAL]
> Padrão: ligada.

Habilita o `/check`, que pede ao agente para comparar as mudanças do branch com as memórias sobre os arquivos que elas tocam. Veja [Aprendizado](./learning#conferir-um-branch-contra-a-memoria).

### `experimental-memory-seed`

> [!EXPERIMENTAL]
> Padrão: ligada.

Habilita o `/memory seed`, que oferece como memórias as frases que você escreveu em duas ou mais sessões anteriores. Veja [Memória](./memory#semear-a-memoria-com-sessoes-anteriores).

### `experimental-memory-scorecard`

> [!EXPERIMENTAL]
> Padrão: ligada.

Uma em cada 20 requisições roda sem memória lembrada, e o `/memory score` compara correções e custo com e sem ela. Veja [Memória](./memory#medir-se-a-memoria-ajuda).

## Inteligência de código

### `experimental-code-map`

> [!EXPERIMENTAL]
> Padrão: ligada.

Indexa as funções, os tipos e as chamadas do repositório com tree-sitter e acrescenta a cada prompt uma lista curta dos arquivos e símbolos que combinam com ele. Veja [Inteligência de código](./code-intelligence#mapa-de-codigo).

### `experimental-blast-radius`

> [!EXPERIMENTAL]
> Padrão: ligada.

Depois de uma edição, pergunta ao servidor de linguagem quais outros arquivos usam os símbolos que a edição mudou e conta ao agente. Veja [Inteligência de código](./code-intelligence#raio-de-impacto).

## Desligar uma funcionalidade interna inteira

Algumas flags desligam uma funcionalidade interna inteira, e não um comportamento dela. São flags comuns: o `/features` as lista, `/features <nome> on|off` as troca na hora, e elas ficam ligadas até você desligar.

| Flag | O que desliga |
|---|---|
| `memory-search` | A ferramenta `memory_search` que o agente usa para buscar na memória; a lembrança a cada prompt continua |
| `schedules` | `/schedules`, `/loop` e o disparo dos prompts agendados |
| `plugins` | Instalar, atualizar, listar e remover plugins |
| `trace` | Os dados das páginas do `/trace` e do `/console` |
| `channels` | Servidores MCP que ficam abertos durante a sessão e mandam mensagens ao agente |
| `declared-providers` | O carregamento dos provedores de modelo declarados por arquivos na sua pasta de configurações e no projeto |
| `suggested-rules` | `/rules` e a gravação de regras e linhas de instrução pelo `/learn keep` |
| `experimental-code-map` | O mapa de código |
| `experimental-output-savings` | `/gain` |

Uma requisição a uma funcionalidade desligada responde com o nome dela e o comando que a religa, por exemplo `trace is turned off (/features trace on)`.

Só esses nomes desligam uma funcionalidade. Um nome que o `/features` não lista não faz nada no objeto `features`, por mais parecido que seja: `"code_map": false`, `"output-savings": false` ou uma flag escrita errado não desligam nada. Cinco funcionalidades internas não podem ser desligadas, porque o {{NAME}} não funciona sem elas: as próprias feature flags (`/features`), a memória do projeto, a lista de sessões, os comandos em segundo plano e o workspace (a oferta de git, as menções de arquivo com `@` e as listas de mudanças). Para parar só a oferta de git, desligue a [`git-offer`](#git-offer).

> [!NOTE]
> Versões anteriores aceitavam aqui o nome da pasta de qualquer funcionalidade interna, com hífens ou sublinhados, inclusive `workspace`, `sessions`, `shell` e `flags`. Esses quatro agora são ignorados; `code-map` e `output-savings` se escrevem `experimental-code-map` e `experimental-output-savings`.
