---
title: Orçamento e custo
description: Veja quanto uma sessão custou, limite o gasto com /budget, mantenha o cache de prompt quente e meça quanto a saída dos comandos foi enxugada.
---

O {{NAME}} estima o preço de cada chamada de modelo a partir das tabelas publicadas pelo provedor, soma por sessão e pode parar uma sessão quando ela gasta o que você permitiu.

## Ver quanto uma sessão custou

Digite `/cost` (ou `/usage`):

```text
/cost
This session cost $0.4210.
  claude-opus-5-5   in 12.4k · out 3.1k · cache read 210.8k (94% hit) · cache write 13.0k · $0.3900
  claude-haiku-5-5  in 4.2k · out 600 · cache read 0 · cache write 0 · 2 web searches · $0.0310
```

Cada modelo usado na sessão ganha uma linha:

| Campo | Significado |
|---|---|
| `in` | Tokens de entrada que não vieram do cache |
| `out` | Tokens de saída |
| `cache read` | Tokens de entrada servidos pelo cache de prompt, com a fração da entrada que acertou o cache |
| `cache write` | Tokens gravados no cache |
| `web searches` | Buscas feitas pelo provedor, quando houve |
| Última coluna | O custo estimado, ou `not priced` |

Contagens acima de 1.000 aparecem com `k`, acima de um milhão com `M`.

> [!NOTE]
> Os custos são **estimativas** feitas a partir de preços de referência, não faturas. Para um provedor que você usa por assinatura, o valor é o que as mesmas chamadas custariam pela API; o seu plano cobra do jeito dele. Quando não há preço conhecido para nenhum modelo da sessão, o `/cost` diz `No price is known for these models (a subscription bills by its plan).`

Assim que a sessão custa alguma coisa, a barra de status mostra o total acumulado (` · $0.42`) e o `/status` ganha uma linha `Cost`. O total também fica disponível para uma linha de status personalizada como `cost.total_cost_usd` (veja [Linha de status](./statusline)), e o [modo headless](./headless) informa `total_cost_usd` na saída JSON.

## Limitar o gasto de uma sessão

> [!NOTE]
> Controlado pela flag `budget`. Padrão: ligada.

```text
/budget 5
Budget $5.00: model calls stop once this session has spent it. Spent $0.42 of $5.00.
```

| Comando | Efeito |
|---|---|
| `/budget` | Mostra o que foi gasto e o orçamento, se houver |
| `/budget <dólares>` | Define o orçamento; aceita um `$` na frente (`/budget $2.50`) |
| `/budget off` ou `/budget none` | Remove o orçamento |

Com um orçamento definido, a linha de status mostra `$<gasto>/$<orçamento>`. Antes de cada chamada de modelo, o {{NAME}} soma o custo da execução até ali ao que a sessão já tinha gastado; quando isso chega ao orçamento, a chamada não é feita e a execução para com:

```text
Stopped: this session spent $5.03 of its $5.00 budget. Raise it with /budget <dollars>, or turn it off with /budget off.
```

A conferência acontece antes de cada chamada, então a chamada que ultrapassa o orçamento ainda termina; a seguinte é recusada. As chamadas de subagentes contam para o orçamento; chamadas sem preço conhecido contam como zero. O orçamento pertence à conversa: escolha um modelo antes, ou o `/budget` responde `Pick a model first: a budget belongs to a conversation.` Ele também aparece como **Budget** no `/config`.

> [!WARNING]
> Com a flag `budget` desligada, o `/budget` ainda registra um limite e a linha de status ainda o mostra, mas as execuções o ignoram.

## Manter o cache de prompt quente

> [!NOTE]
> Controlado pela flag `cache-warning`. Padrão: ligada.

Os provedores mantêm o começo de uma conversa num cache de prompt por uns cinco minutos depois da última chamada; ler do cache sai bem mais barato do que mandar os mesmos tokens de novo. Enquanto o agente está parado e a última chamada usou 10.000 tokens em cache ou mais, a linha de status faz a contagem regressiva dos dois últimos minutos do cache:

```text
cache 1:45
```

e, passados os cinco minutos, avisa que o próximo prompt vai reescrevê-lo:

```text
cache cold: next turn rewrites 48k tokens
```

Nada aparece enquanto o agente trabalha, nem quando menos de 10.000 tokens estavam em cache.

## Ver quanta saída foi economizada

> [!EXPERIMENTAL]
> Controlado pela flag `experimental-output-savings`. Padrão: ligada.

As [flags de saída](./experimental-features#saida-de-comandos) limpam e filtram o que os comandos imprimem antes de o modelo ler. O `/gain` mostra quanto isso encurtou a saída desde que o {{NAME}} iniciou:

```text
/gain
Command output this session: 182k → 41k characters, 77% shorter (about 35.4k tokens kept out of the conversation)
  cargo          12 runs    150k → 28.1k   81%
  git             9 runs     20k → 9.2k    54%
  npm             3 runs   12.2k → 3.7k    69%
```

Os programas aparecem ordenados pelos caracteres economizados. A conta de tokens considera quatro caracteres por token. Com a flag desligada, o `/gain` responde `/gain is off. Turn it on with /features experimental-output-savings on.`

## Dividir a carga entre contas

Quando você entrou no mesmo provedor com várias contas, o `/accounts balance` decide como elas dividem o trabalho:

| Modo | Comportamento |
|---|---|
| `off` | Só a conta em uso |
| `failover` | A conta em uso e, quando ela está ocupada ou desconectada, as outras |
| `round-robin` | Cada execução começa na próxima conta, e as outras assumem quando uma está ocupada |

```text
/accounts balance failover
Accounts balance: failover (the account in use, then the others when it is busy or signed out).
```

O `/accounts balance` sozinho lista os três modos. A configuração vale para o provedor do modelo atual. Veja [Contas](./accounts).

## Relacionados

- [Modelos e provedores](./models-and-providers): modelos de fallback e papéis de modelo, como um `smallModel` mais barato.
- [Feature flags](./experimental-features): `budget`, `cache-warning` e as flags de saída.
