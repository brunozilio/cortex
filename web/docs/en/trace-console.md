---
title: Trace console
description: Open a live page in your browser that shows every session's runs, spans, cost, waits and checks, drive a session from a terminal in the page, and read these docs with /docs.
---

`/trace` opens a page in your browser that shows what the agent did in every session of the last 30 days: each prompt you sent, the model calls and tools behind it, what it cost, how long you were waited on, and what the guards caught. It updates live while you work.

## Open the page

```text
/trace
Live trace opened in the browser: http://127.0.0.1:53817/#enter=…
```

`/console` does the same. {{NAME}} starts a small web server the first time, opens the page in your default browser and prints the address in the chat. The server stays up until you quit the chat; later `/trace` or `/docs` commands reuse it.

The page has three parts, top to bottom:

1. **Overview**: how the agent worked across all sessions, or in the session you pick.
2. **Traces**: the runs of the session you pick, one per prompt.
3. **Terminal**: the session, live in the same CLI, inside the page.

Pick a session in the header's session menu (**All sessions** by default; type in it to search). Traces and Terminal appear once a session is picked. The **EN** / **PT** switch at the right of the header changes the page's language; the browser remembers the choice.

## Overview

The overview covers the last 30 days, for every session or for the one you picked.

| Card | Shows |
|---|---|
| **Spend** | Total estimated cost, with the change from the week before and a 14-day sparkline |
| **Runs** | Number of runs, with the same trend |
| **p95 run time** | The 95th percentile of run duration, and the share of run time spent waiting on you |
| **Tool error rate** | The share of all tool calls that failed, marked `high` when it is |
| **Usage by model** | Daily **Cost**, **Tokens** or **Runs** (pick one), stacked by model |
| **Where the time went** | Share of run time in model calls, tools, waiting on you, subagents, hooks and tests, and evaluations |
| **Prompt cache** | The share of input tokens served from the cache |
| **Checks** | What the guards caught: verification judge passes and failures, auto-review findings, loops stopped, calls refused, times it asked you |
| **Models** | Latency and cost per model |
| **Tools** | Calls, failures and how often each tool asked you, with a warning when a tool called 5 times or more failed over 10% of its calls |

## Traces

**Traces** lists every prompt you sent in the session, newest first, with what the agent did to answer it:

| Column | Meaning |
|---|---|
| Prompt | What you sent |
| Model | The model the run used |
| Duration | How long the run took |
| Tokens | Input and output tokens |
| Cost | Estimated cost |
| Waste | The share of the run's time spent in tool calls that failed |
| Signals | Badges for what happened |

The signals are **Tool error**, **Eval failed** (the verification judge failed the work), **Review** (the automatic review found something), **Repeated** (the repeated-call guard stopped the agent), **Refused**, **Subagent**, **Waste**, **Fell back** (the run switched to another model) and **Compacted**. Filter the table by status, model, session and signal.

Open a run to see its **waterfall**: one bar per span, coloured by kind:

| Kind | Is |
|---|---|
| Run | The whole run |
| Agent | A sub-agent |
| LLM | A model call |
| Tool | A tool call |
| Task | A hook or a test run |
| Evaluation | A verification judge or review |
| Waiting on you | Time the run waited for your answer |
| Event | Something that happened at one moment, such as a fallback or a compaction |

Pick a span to inspect it in tabs: **Input / Output**, **Messages**, **System** and **Tools** (for model calls), **Metrics**, **Metadata** and **Evaluations**. The Metrics tab points out the most expensive call of the run.

The current session's runs carry more text than other sessions'; long prompts and outputs are trimmed.

## Drive a session from the page

### Terminal

With a session picked, the **Terminal** panel runs `{{COMMAND}} --resume <session>` in the session's folder and shows it in the page, so you can chat with that session from the browser exactly as in your own terminal. If it ends, **Start again** starts it again.

> [!WARNING]
> The terminal in the page is a real {{NAME}} process on your computer, with your permissions. Anyone who can open the page can use it; keep the link to yourself.

The session open in the chat that opened the page is not started again: the panel shows that chat's own screen, and what you type in the page reaches it as if typed in your terminal.

A session the chat did not list is resumed in the project folder of the chat that opened the page. At most 4 terminals can be open from one page server at a time; a fifth is refused until one closes.

The terminal is the only way to drive a session from the page: the page's server does not accept prompts.

## Read the docs with /docs

```text
/docs
/docs hooks
```

`/docs` opens this documentation in the same page server, with {{NAME}}'s real names filled in. `/docs <topic>` opens it with a search for the topic.

## Export to OpenTelemetry

{{NAME}} can also send each finished run to an OpenTelemetry collector (Jaeger, Grafana Tempo, Honeycomb, an `otelcol` of your own), as OTLP over HTTP with a JSON body. It is off until you name a collector:

```bash
{{ENV}}_OTLP_ENDPOINT=http://localhost:4318 {{COMMAND}}
{{ENV}}_OTLP_ENDPOINT=https://api.honeycomb.io {{ENV}}_OTLP_HEADERS="x-honeycomb-team=<key>" {{COMMAND}}
```

Spans are sent to `<endpoint>/v1/traces` (an endpoint that already ends in `/v1/traces` is used as is). What is sent:

| In the trace page | In OpenTelemetry |
|---|---|
| A run (one prompt and all it caused) | A trace, whose root span is `run`. Its attributes: `session.id`, `{{COMMAND}}.run.index`, `{{COMMAND}}.run.status`, `gen_ai.request.model`, `{{COMMAND}}.tokens`, `{{COMMAND}}.cost_usd`, `{{COMMAND}}.waiting_ms`, and the run's model, provider, permission mode, effort, folder and branch as `{{COMMAND}}.*` |
| A model call | A `chat <model>` span (kind client) with `gen_ai.request.model`, `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`, `gen_ai.usage.cache_read_input_tokens`, `gen_ai.usage.cache_creation_input_tokens`, `gen_ai.response.finish_reasons` and `{{COMMAND}}.cost_usd` |
| A tool call | An `execute_tool <name>` span with `gen_ai.tool.name`; a failed call has status error |
| Sub-agents, waits, checks and events | Spans named as on the page, under the span that started them |

Every span carries `{{COMMAND}}.span.kind` and `{{COMMAND}}.duration_ms`, and its real start and end times. Prompts, answers, tool inputs and outputs are never sent: only names, counts, costs and times.

Exporting never slows a run down. A finished run is queued and a background thread reads its trace, waits up to two seconds for other runs to finish so they go in the same request, and posts them. A run still answering is sent once it ends; each run is sent once, and the first run seen of a resumed session is its latest one, not its whole history. When the collector is down or answers with an error, the runs of that request are dropped and the problem is written once to the [log file](./troubleshooting) (later failures only at `debug` level); the chat says nothing.

## Access and security

- The server listens only on `127.0.0.1`, on a free port chosen at start.
- The address printed in the chat carries a one-time entry code after the `#`, which the browser never sends to a server. The page takes the code out of the address, trades it once for the console's token and keeps that token in its own storage, which belongs to that port alone; every request for the page's data carries it. No cookie is set, because the browser would send a cookie for `127.0.0.1` to every other local server too. A code works once and for two minutes: run `/trace` or `/console` again for a fresh link.
- The server only answers requests addressed to `127.0.0.1:<port>` or `localhost:<port>`; any other host name gets `403`.
- Requests that come from another website are refused.
- A link from an earlier run of the chat no longer works (`This console link is not valid any more.`); type `/trace` again for a new one.

## When the page does not load

The page is a web app built with Bun. {{NAME}} serves it from, in order:

1. the folder named in the `{{ENV}}_CONSOLE_DIR` environment variable, when it holds an `index.html`;
2. the copy built into the binary, when the page had been built before the binary was compiled;
3. a `console` folder in {{NAME}}'s data folder.

When none is found, the page says **The console app is not built yet**: the binary was compiled without the page. From the repository, with [Bun](https://bun.sh) installed, build the page and then the binary again:

```bash
cargo run -p xtask -- web
cargo build --release
```

If the server cannot start at all, the chat says `The {{NAME}} page could not start: …`.

## Related

- [Budget and cost](./budget-and-cost): `/cost` in the chat.
- [Verification and review](./verification-and-review): where the checks in the overview come from.
- [Sessions](./sessions): resuming sessions.
