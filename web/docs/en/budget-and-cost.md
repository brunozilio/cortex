---
title: Budget and cost
description: See what a session has cost, cap its spending with /budget, keep the prompt cache warm and measure how much command output was trimmed.
---

{{NAME}} estimates the price of every model call from the provider's published rates, adds it up per session, and can stop a session once it has spent what you allow.

## See what a session cost

Type `/cost` (or `/usage`):

```text
/cost
This session cost $0.4210.
  claude-opus-5-5   in 12.4k · out 3.1k · cache read 210.8k (94% hit) · cache write 13.0k · $0.3900
  claude-haiku-5-5  in 4.2k · out 600 · cache read 0 · cache write 0 · 2 web searches · $0.0310
```

Each model used in the session gets one line:

| Field | Meaning |
|---|---|
| `in` | Input tokens not served from the cache |
| `out` | Output tokens |
| `cache read` | Input tokens served from the prompt cache, with the share of input that hit the cache |
| `cache write` | Tokens written to the cache |
| `web searches` | Searches the provider ran, when there were any |
| Last column | The estimated cost, or `not priced` |

Token counts above 1,000 are shown as `k`, above a million as `M`.

> [!NOTE]
> Costs are **estimates** made from reference prices, not invoices. For a provider you reach with a subscription, the figure is what the same calls would cost through the API; your plan bills you its own way. When no price is known for any model of the session, `/cost` says `No price is known for these models (a subscription bills by its plan).`

Once the session has cost something, the status bar shows the running total (` · $0.42`) and `/status` adds a `Cost` line. The total is also available to a custom status line as `cost.total_cost_usd` (see [Status line](./statusline)), and [headless mode](./headless) reports `total_cost_usd` in its JSON output.

## Cap a session's spending

> [!NOTE]
> Controlled by the `budget` flag. Default: on.

```text
/budget 5
Budget $5.00: model calls stop once this session has spent it. Spent $0.42 of $5.00.
```

| Command | Effect |
|---|---|
| `/budget` | Show what was spent and the budget, if any |
| `/budget <dollars>` | Set the budget; a leading `$` is accepted (`/budget $2.50`) |
| `/budget off` or `/budget none` | Remove the budget |

While a budget is set, the status line shows `$<spent>/$<budget>`. Before each model call, {{NAME}} adds the cost of the run so far to what the session had spent; when that reaches the budget, the call is not made and the run stops with:

```text
Stopped: this session spent $5.03 of its $5.00 budget. Raise it with /budget <dollars>, or turn it off with /budget off.
```

The check happens before a call, so the call that crosses the budget still completes; the next one is refused. Sub-agents' calls count toward the budget; calls whose price is unknown count as zero. The budget belongs to the conversation: pick a model first, or `/budget` answers `Pick a model first: a budget belongs to a conversation.` It is also listed as **Budget** in `/config`.

> [!WARNING]
> With the `budget` flag off, `/budget` still records a limit and the status line still shows it, but runs ignore it.

## Keep the prompt cache warm

> [!NOTE]
> Controlled by the `cache-warning` flag. Default: on.

Providers keep the start of a conversation in a prompt cache for about five minutes after the last call; reading from the cache is much cheaper than sending the same tokens again. While the agent is idle and the last call used 10,000 cached tokens or more, the status line counts down the cache's last two minutes:

```text
cache 1:45
```

and, once the five minutes have passed, warns that the next prompt will rewrite it:

```text
cache cold: next turn rewrites 48k tokens
```

Nothing is shown while the agent is working, or when less than 10,000 tokens were cached.

## See how much output was saved

> [!EXPERIMENTAL]
> Controlled by the `experimental-output-savings` flag. Default: on.

The [output flags](./experimental-features#command-output) clean and filter what commands print before the model reads it. `/gain` shows how much shorter that made the output since {{NAME}} started:

```text
/gain
Command output this session: 182k → 41k characters, 77% shorter (about 35.4k tokens kept out of the conversation)
  cargo          12 runs    150k → 28.1k   81%
  git             9 runs     20k → 9.2k    54%
  npm             3 runs   12.2k → 3.7k    69%
```

Programs are sorted by characters saved. The token figure assumes four characters per token. With the flag off, `/gain` answers `/gain is off. Turn it on with /features experimental-output-savings on.`

## Share the load between accounts

When you have signed in to the same provider with several accounts, `/accounts balance` decides how they share the work:

| Way | Behaviour |
|---|---|
| `off` | Only the account in use |
| `failover` | The account in use, then the others when it is busy or signed out |
| `round-robin` | Each run starts on the next account, and the others take over when one is busy |

```text
/accounts balance failover
Accounts balance: failover (the account in use, then the others when it is busy or signed out).
```

`/accounts balance` alone lists the three ways. The setting applies to the provider of the current model. See [Accounts](./accounts).

## Related

- [Models and providers](./models-and-providers): fallback models and model roles, such as a cheaper `smallModel`.
- [Feature flags](./experimental-features): `budget`, `cache-warning` and the output flags.
