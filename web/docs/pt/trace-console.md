---
title: Console de trace
description: Abra no navegador uma página ao vivo com as execuções, spans, custo, esperas e checagens de cada sessão, conduza uma sessão por um terminal dentro da página e leia esta documentação com /docs.
---

O `/trace` abre no navegador uma página que mostra o que o agente fez em cada sessão dos últimos 30 dias: cada prompt que você enviou, as chamadas de modelo e as ferramentas por trás dele, quanto custou, quanto tempo esperou por você e o que as proteções pegaram. Ela se atualiza ao vivo enquanto você trabalha.

## Abrir a página

```text
/trace
Live trace opened in the browser: http://127.0.0.1:53817/?token=…
```

O `/console` faz o mesmo. Na primeira vez, o {{NAME}} inicia um pequeno servidor web, abre a página no navegador padrão e imprime o endereço no chat. O servidor fica de pé até você sair do chat; os próximos `/trace` ou `/docs` o reaproveitam.

A página tem três partes, de cima para baixo:

1. **Overview**: como o agente trabalhou em todas as sessões, ou na sessão que você escolher.
2. **Traces**: as execuções da sessão escolhida, uma por prompt.
3. **Terminal**: a sessão, ao vivo, na mesma CLI, dentro da página.

Escolha uma sessão no menu de sessões do cabeçalho (**All sessions** por padrão; digite nele para buscar). Traces e Terminal aparecem depois que uma sessão é escolhida.

## Overview

O overview cobre os últimos 30 dias, de todas as sessões ou da que você escolheu.

| Cartão | Mostra |
|---|---|
| **Spend** | O custo estimado total, com a variação em relação à semana anterior e um minigráfico de 14 dias |
| **Runs** | O número de execuções, com a mesma tendência |
| **p95 run time** | O percentil 95 da duração das execuções e a fração do tempo de execução gasta esperando por você |
| **Tool error rate** | A fração de todas as chamadas de ferramenta que falharam, marcada `high` quando é alta |
| Gráfico de uso | **Cost**, **Tokens** ou **Runs** por dia (escolha um) |
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

### Prompts enviados à página

O servidor da página também aceita prompts para uma sessão (`POST /api/prompt` com o corpo JSON `{"session": "<id>", "text": "<prompt>"}`), e o chat que abriu a página os entrega:

- para a sessão aberta no chat: enviado como uma mensagem que você digitou (ou acrescentado à execução quando o agente está trabalhando);
- para uma sessão que já está rodando um prompt enviado pela página: acrescentado a essa execução;
- para qualquer outra sessão: rodado em segundo plano, retomando essa sessão na pasta dela com o modelo, o modo de permissão e o esforço do chat.

Um prompt que falha é informado no chat como `The prompt sent from the trace page to <session> failed: …`.

## Ler a documentação com /docs

```text
/docs
/docs hooks
```

O `/docs` abre esta documentação no mesmo servidor da página, com os nomes reais do {{NAME}} preenchidos. O `/docs <assunto>` a abre já com uma busca pelo assunto.

## Acesso e segurança

- O servidor só escuta em `127.0.0.1`, numa porta livre escolhida ao iniciar.
- O endereço impresso no chat carrega um token secreto. Abri-lo define um cookie `HttpOnly` e `SameSite=Strict` e tira o token do endereço; os dados da página exigem esse cookie.
- Requisições que vêm de outro site são recusadas.
- Um link de uma execução anterior do chat não funciona mais (`This console link is not valid any more.`); digite `/trace` de novo para ter um novo.

## Quando a página não carrega

A página é um app web construído com Bun. O {{NAME}} a serve a partir de, em ordem:

1. a pasta indicada na variável de ambiente `{{ENV}}_CONSOLE_DIR`, quando ela tem um `index.html`;
2. a cópia embutida no binário, quando o Bun estava presente na compilação;
3. uma pasta `console` na pasta de dados do {{NAME}}, ou o `web/dist` do repositório.

Quando nenhuma é encontrada, a página diz **The console app is not built yet**: instale o [Bun](https://bun.sh) e compile de novo, ou construa o app uma vez:

```bash
cd web && bun install && bun run build
```

Se o servidor nem conseguir iniciar, o chat diz `The {{NAME}} page could not start: …`.

## Relacionados

- [Orçamento e custo](./budget-and-cost): o `/cost` no chat.
- [Verificação e revisão](./verification-and-review): de onde vêm as checagens do overview.
- [Sessões](./sessions): retomar e arquivar sessões.
