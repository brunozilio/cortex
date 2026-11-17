# 0011 Code mode: the agent scripts its tools in a persistent JavaScript context

Status: Accepted, 2026-11-16

## Context

Every tool call is a model round trip: the whole conversation is read again, the model decides the next call, and the call's full output lands in the context and stays there. A task such as "find every caller of this function and change the argument" costs one turn per file and fills the context with files the model only needed to filter.

Other agents answer this by letting the model write code that calls tools as functions. Claude Code ships it as an experimental "Code Mode": a `REPL` tool backed by a sealed, persistent JavaScript context in which the tools are async functions, with output limited to what the script returns or logs. Anthropic's API has the same idea server-side ("programmatic tool calling"), but it runs in Anthropic's container and only for Anthropic models; this app speaks to several providers and runs tools on the user's machine.

The app already embeds QuickJS (`crates/script`) for script tools, hooks and prompts, and already routes a script's `ctx.tool(...)` through the agent's ordinary tool call.

## Decision

Add a `REPL` tool, behind the `experimental-code-mode` feature flag, that runs locally for every provider:

- One QuickJS context per session and agent, kept on its own thread (`script::session::Session`), at most eight kept at once, the least recently used let go. Scripts are evaluated as global async code in strict mode, so top-level `await` works and declarations persist between calls. No module loader and no host objects: the only way out of the context is a tool call.
- Every tool in the request, except `REPL` itself and the tools that change the turn (plan mode, worktrees), is a non-writable global async function; any tool is also reachable as `tools["name"]`. A call is an ordinary `Agent::call`, so hooks, permission rules, asks, redaction and the transcript events apply to each one, and inner calls do not enter the conversation.
- A call resolves to the tool's text and rejects with a `ToolError` when the tool fails or is refused. Claude Code resolves its built-in tools to `{error}` instead; throwing gives one rule for every tool and makes a refused call impossible to mistake for a result.
- Calls a script starts together run concurrently when the same rule that parallelises a model's reply allows it (read-only, inside the project, not denied); anything else is served one at a time.
- What returns is the last expression's value (strings as they are, the rest as JSON, promises in the keys of a returned object awaited) and the `self.log` lines, each cut to 20,000 characters keeping head and tail. `console.*` goes to a ring the script reads with `console.tail`.
- Limits: 30 s of the script's own JavaScript (time waiting on tools excluded), a whole-script timeout of at most 10 minutes, 1,000 tool calls, 512 MB of memory; the user's interrupt stops the script.

Two modes:

- `experimental-code-mode` (on unless turned off, like every flag) adds `REPL` beside the other tools.
- `experimental-code-mode-only` is the first opt-in flag: off until the owner turns it on, and a project cannot turn it on. With it, the model is offered `REPL` plus the turn-level tools (`AskUserQuestion`, plan mode, worktrees); every other tool is described inside `REPL`'s description as a TypeScript-style `declare function`, generated from its JSON schema with its documentation, sorted by name so the same tools always give the same text and the prompt cache holds. The request still carries every tool, so a script's calls pass the same "offered" check as before. Deferred tools are not declared; a script finds them with `searchTools(words)`, which loads them as ToolSearch does and returns their declarations.

Helpers live in `content/repl/helpers.js`, evaluated once per context: `` $`cmd` `` runs a command through Bash with interpolated values shell-quoted. There is no `file()`/`write()` helper: the Read tool numbers lines and answers "unchanged since you last read" on a second read, so raw file text from a script would need its own permission-checked read. That, and scripts that keep running across turns, are left for later.

## Consequences

- Long, mechanical work costs one model turn, and only what the script chooses to return enters the context.
- The permission model is unchanged: a script can do nothing its tools could not, and asks the user exactly as the tools would.
- Like every feature flag, it is on unless turned off; the extra tool description costs prompt tokens on every request.
- A script's tool calls are shown as separate tool rows, not nested under the `REPL` call.
- A `const` cannot be redeclared in a later call; the model is told to pick another name.
- A context lives only in the process: it is lost on restart, and a resumed session starts with an empty one.
- Calls that finished scripts leave unawaited are not answered; their effects (a write already served) are not undone.
