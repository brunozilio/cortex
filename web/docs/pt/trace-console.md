---
title: Console de trace
description: Abra no navegador uma página ao vivo com as execuções, spans, custo, esperas e checagens de cada sessão, conduza uma sessão por um terminal dentro da página e leia esta documentação com /docs.
---

O `/trace` abre no navegador uma página que mostra o que o agente fez em cada sessão dos últimos 30 dias: cada prompt que você enviou, as chamadas de modelo e as ferramentas por trás dele, quanto custou, quanto tempo esperou por você e o que as proteções pegaram. Ela se atualiza ao vivo enquanto você trabalha.

## Abrir a página

```text
/trace
Live trace opened in the browser: http://127.0.0.1:53817/#enter=…
```

O `/console` faz o mesmo. Na primeira vez, o {{NAME}} inicia um pequeno servidor web, abre a página no navegador padrão e imprime o endereço no chat. O servidor fica de pé até você sair do chat; os próximos `/trace` ou `/docs` o reaproveitam.

A página tem três partes, de cima para baixo:

1. **Overview**: como o agente trabalhou em todas as sessões, ou na sessão que você escolher.
2. **Traces**: as execuções da sessão escolhida, uma por prompt.
3. **Terminal**: a sessão, ao vivo, na mesma CLI, dentro da página.

Escolha uma sessão no menu de sessões do cabeçalho (**All sessions** por padrão; digite nele para buscar). Traces e Terminal aparecem depois que uma sessão é escolhida. A chave **EN** / **PT** à direita do cabeçalho muda o idioma da página; o navegador guarda a escolha.

## Overview

O overview cobre os últimos 30 dias, de todas as sessões ou da que você escolheu.

| Cartão | Mostra |
|---|---|
| **Spend** | O custo estimado total, com a variação em relação à semana anterior e um minigráfico de 14 dias |
| **Runs** | O número de execuções, com a mesma tendência |
| **p95 run time** | O percentil 95 da duração das execuções e a fração do tempo de execução gasta esperando por você |
| **Tool error rate** | A fração de todas as chamadas de ferramenta que falharam, marcada `high` quando é alta |
| **Usage by model** | **Cost**, **Tokens** ou **Runs** por dia (escolha um), empilhados por modelo |
| **Where the time went** | A fração do tempo de execução em chamadas de modelo, ferramentas, espera por você, subagentes, hooks e testes, e avaliações |
| **Prompt cache** | A fração dos tokens de entrada servidos pelo cache |
| **Checks** | O que as proteções pegaram: aprovações e reprovações do juiz de verificação, achados da revisão automática, loops interrompidos, chamadas recusadas, vezes que perguntou a você |
| **Models** | Latência e custo por modelo |
| **Tools** | Chamadas, falhas e quantas vezes cada ferramenta perguntou a você, com um aviso quando uma ferramenta chamada 5 vezes ou mais falhou em mais de 10% das chamadas |

## Traces

O **Traces** lista cada prompt que você enviou na sessão, do mais novo para o mais antigo, com o que o agente fez para responder:

| Coluna | Significado |
|---|---|
| Prompt | O que você enviou |
| Model | O modelo usado na execução |
| Duration | Quanto a execução durou |
| Tokens | Tokens de entrada e saída |
| Cost | Custo estimado |
| Waste | A fração do tempo da execução gasta em chamadas de ferramenta que falharam |
| Signals | Selos do que aconteceu |

Os sinais são **Tool error**, **Eval failed** (o juiz de verificação reprovou o trabalho), **Review** (a revisão automática achou algo), **Repeated** (a proteção contra chamadas repetidas parou o agente), **Refused**, **Subagent**, **Waste**, **Fell back** (a execução trocou de modelo) e **Compacted**. Filtre a tabela por status, modelo, sessão e sinal.

Abra uma execução para ver a **cascata** (waterfall): uma barra por span, colorida pelo tipo:

| Tipo | É |
|---|---|
| Run | A execução inteira |
| Agent | Um subagente |
| LLM | Uma chamada de modelo |
| Tool | Uma chamada de ferramenta |
| Task | Um hook ou uma execução de testes |
| Evaluation | Um juiz de verificação ou uma revisão |
| Waiting on you | Tempo em que a execução esperou a sua resposta |
| Event | Algo que aconteceu num instante, como um fallback ou uma compactação |

Escolha um span para inspecioná-lo nas abas **Input / Output**, **Messages**, **System** e **Tools** (para chamadas de modelo), **Metrics**, **Metadata** e **Evaluations**. A aba Metrics aponta a chamada mais cara da execução.

As execuções da sessão atual trazem mais texto que as das outras sessões; prompts e saídas longos são cortados.

## Conduzir uma sessão pela página

### Terminal

Com uma sessão escolhida, o painel **Terminal** roda `{{COMMAND}} --resume <sessão>` na pasta da sessão e o mostra na página, para você conversar com essa sessão pelo navegador exatamente como no seu terminal. Se ele terminar, **Start again** o inicia de novo.

> [!WARNING]
> O terminal da página é um processo real do {{NAME}} no seu computador, com as suas permissões. Quem conseguir abrir a página consegue usá-lo; guarde o link só para você.

A sessão aberta no chat que abriu a página não é iniciada de novo: o painel mostra a própria tela daquele chat, e o que você digita na página chega a ele como se fosse digitado no seu terminal.

Uma sessão que o chat não listou é retomada na pasta do projeto do chat que abriu a página. No máximo 4 terminais ficam abertos ao mesmo tempo a partir de um servidor da página; um quinto é recusado até que um deles feche.

O terminal é a única forma de conduzir uma sessão pela página: o servidor da página não aceita prompts.

## Ler a documentação com /docs

```text
/docs
/docs hooks
```

O `/docs` abre esta documentação no mesmo servidor da página, com os nomes reais do {{NAME}} preenchidos. O `/docs <assunto>` a abre já com uma busca pelo assunto.

## Exportar para o OpenTelemetry

O {{NAME}} também pode mandar cada run terminado a um coletor OpenTelemetry (Jaeger, Grafana Tempo, Honeycomb, um `otelcol` seu), como OTLP sobre HTTP com corpo JSON. Fica desligado até você indicar um coletor:

```bash
{{ENV}}_OTLP_ENDPOINT=http://localhost:4318 {{COMMAND}}
{{ENV}}_OTLP_ENDPOINT=https://api.honeycomb.io {{ENV}}_OTLP_HEADERS="x-honeycomb-team=<chave>" {{COMMAND}}
```

Os spans vão para `<endpoint>/v1/traces` (um endpoint que já termina em `/v1/traces` é usado como está). O que é mandado:

| Na página de trace | No OpenTelemetry |
|---|---|
| Um run (um prompt e tudo o que ele causou) | Um trace, cujo span raiz é `run`. Atributos: `session.id`, `{{COMMAND}}.run.index`, `{{COMMAND}}.run.status`, `gen_ai.request.model`, `{{COMMAND}}.tokens`, `{{COMMAND}}.cost_usd`, `{{COMMAND}}.waiting_ms`, e o modelo, o provedor, o modo de permissão, o esforço, a pasta e a branch do run como `{{COMMAND}}.*` |
| Uma chamada ao modelo | Um span `chat <modelo>` (tipo client) com `gen_ai.request.model`, `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`, `gen_ai.usage.cache_read_input_tokens`, `gen_ai.usage.cache_creation_input_tokens`, `gen_ai.response.finish_reasons` e `{{COMMAND}}.cost_usd` |
| Uma chamada de ferramenta | Um span `execute_tool <nome>` com `gen_ai.tool.name`; uma chamada que falhou tem status de erro |
| Subagentes, esperas, verificações e eventos | Spans com os nomes da página, sob o span que os iniciou |

Todo span leva `{{COMMAND}}.span.kind` e `{{COMMAND}}.duration_ms`, e os horários reais de início e fim. Prompts, respostas, entradas e saídas de ferramentas nunca são mandados: só nomes, contagens, custos e tempos.

Exportar nunca atrasa um run. Um run terminado entra numa fila e uma thread em segundo plano lê o trace dele, espera até dois segundos por outros runs que terminem para irem no mesmo pedido, e os envia. Um run ainda respondendo é mandado quando termina; cada run é mandado uma vez, e o primeiro run visto de uma sessão retomada é o mais recente, não o histórico inteiro. Quando o coletor está fora do ar ou responde com erro, os runs daquele pedido são descartados e o problema é escrito uma vez no [arquivo de log](./troubleshooting) (falhas seguintes só no nível `debug`); o chat não diz nada.

## Acesso e segurança

- O servidor só escuta em `127.0.0.1`, numa porta livre escolhida ao iniciar.
- O endereço impresso no chat carrega, depois do `#`, um código de entrada de uso único, que o navegador nunca envia a um servidor. A página tira o código do endereço, troca-o uma vez pelo token do console e guarda esse token no próprio armazenamento, que pertence só àquela porta; toda requisição pelos dados da página o leva. Nenhum cookie é definido, porque o navegador mandaria um cookie de `127.0.0.1` também para qualquer outro servidor local. Um código vale uma vez e por dois minutos: rode `/trace` ou `/console` de novo para um link novo.
- O servidor só responde a requisições endereçadas a `127.0.0.1:<porta>` ou `localhost:<porta>`; qualquer outro nome de host recebe `403`.
- Requisições que vêm de outro site são recusadas.
- Um link de uma execução anterior do chat não funciona mais (`This console link is not valid any more.`); digite `/trace` de novo para ter um novo.

## Quando a página não carrega

A página é um app web construído com Bun. O {{NAME}} a serve a partir de, em ordem:

1. a pasta indicada na variável de ambiente `{{ENV}}_CONSOLE_DIR`, quando ela tem um `index.html`;
2. a cópia embutida no binário, quando a página já tinha sido construída antes de o binário ser compilado;
3. uma pasta `console` na pasta de dados do {{NAME}}.

Quando nenhuma é encontrada, a página diz **The console app is not built yet**: o binário foi compilado sem a página. No repositório, com o [Bun](https://bun.sh) instalado, construa a página e depois o binário de novo:

```bash
cargo run -p xtask -- web
cargo build --release
```

Se o servidor nem conseguir iniciar, o chat diz `The {{NAME}} page could not start: …`.

## Relacionados

- [Orçamento e custo](./budget-and-cost): o `/cost` no chat.
- [Verificação e revisão](./verification-and-review): de onde vêm as checagens do overview.
- [Sessões](./sessions): retomar sessões.
