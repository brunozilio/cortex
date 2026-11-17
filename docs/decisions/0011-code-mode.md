# 0011 Code mode: the agent scripts its tools in a persistent JavaScript context

Status: Accepted, 2026-11-16

## Context

Every tool call is a model round trip: the whole conversation is read again, the model decides the next call, and the call's full output lands in the context and stays there. A task such as "find every caller of this function and change the argument" costs one turn per file and fills the context with files the model only needed to filter.

Other agents answer this by letting the model write code that calls tools as functions. Claude Code ships it as an experimental "Code Mode": a `REPL` tool backed by a sealed, persistent JavaScript context in which the tools are async functions, with output limited to what the script returns or logs. Anthropic's API has the same idea server-side ("programmatic tool calling"), but it runs in Anthropic's container and only for Anthropic models; this app speaks to several providers and runs tools on the user's machine.

The app already embeds QuickJS (`crates/script`) for script tools, hooks and prompts, and already routes a script's `ctx.tool(...)` through the agent's ordinary tool call.

## Decision

Add a `REPL` tool, behind the `experimental-code-mode` feature flag, that runs locally for every provider:

- One QuickJS context per session and agent, kept on its own thread (`script::session::Session`), at most eight kept at once, the least recently used let go. Scripts are evaluated as global async code in strict mode, so top-level `await` works and declarations persist between calls. No module loader and no host objects: the only way out of the context is a tool call.
- Every tool in the request, except `REPL` itself and the tools that change the turn (plan mode, worktrees), is a non-writable global async function; any tool is also reachable as `tools["name"]`. A call is an ordinary `Agent::call`, so hooks, permission rules, asks, redaction and the transcript events apply to each one, and inner calls do not enter the conversation. Two pieces of turn state describe what the model itself did, so a script's calls are kept out of them: Read's record of lines the model has already seen (a script's read would otherwise make the model's own later read answer "unchanged") and the repeated-call guard (a script loops by design); both are set aside while a script runs and restored after. Calls refused while a script runs are listed after its output, so a refusal the script caught is still seen.
- A call resolves to the tool's text and rejects with a `ToolError` when the tool fails or is refused. Claude Code resolves its built-in tools to `{error}` instead; throwing gives one rule for every tool and makes a refused call impossible to mistake for a result.
- Calls a script starts together run concurrently when the same rule that parallelises a model's reply allows it (read-only, inside the project, not denied); anything else is served one at a time.
- What returns is the last expression's value (strings as they are, the rest as JSON, promises in the keys of a returned object awaited) and the `self.log` lines, each cut to 20,000 characters keeping head and tail. `console.*` goes to a ring the script reads with `console.tail`.
- Limits: 30 s of the script's own JavaScript (time waiting on tools excluded), a whole-script timeout of at most 10 minutes, 1,000 tool calls, 512 MB of memory; the user's interrupt stops the script.

Two modes:

- `experimental-code-mode` (on unless turned off, like every flag) adds `REPL` beside the other tools.
- `experimental-code-mode-only` is the first opt-in flag: off until the owner turns it on, and a project cannot turn it on. With it, the model is offered `REPL` plus the turn-level tools (`AskUserQuestion`, plan mode, worktrees); every other tool is described inside `REPL`'s description as a TypeScript-style `declare function`, generated from its JSON schema with its documentation, sorted by name so the same tools always give the same text and the prompt cache holds. The request still carries every tool, so a script's calls pass the same "offered" check as before. Deferred tools are not declared; a script finds them with `searchTools(words)`, which loads them as ToolSearch does and returns their declarations.

Helpers live in `content/repl/helpers.js`, evaluated once per context: `` $`cmd` `` runs a command through Bash with interpolated values shell-quoted. There is no `file()`/`write()` helper on purpose: Read numbers lines and cuts very long ones, so its text cannot be written back safely, and a raw read beside it would skip what guards Read (PreToolUse hooks, the refusal of secret files, redaction, which would itself corrupt a file written back). Scripts use `Read`, `Edit` and `Write` as the model does.

A script's tool calls are shown the way a sub-agent's are: on its first call the script opens an agent lane of kind `REPL` (`AgentSpawned`), its calls travel as `AgentEvent`s inside it, and the lane ends with the script. That reuses what the TUI, the trace and the console already draw, with no change to the protocol; a script that calls no tool opens no lane.

Claude Code detaches a script that runs past about 30 seconds and keeps several scripts running in one context, telling their logs apart with `AsyncLocalStorage`. QuickJS has nothing like it, so two scripts sharing a context could not keep their `self.log` lines apart. Here a long script is sent to the background explicitly with `run_in_background: true`, like Bash and Agent: it becomes a background task of the run (TaskOutput, TaskStop, TaskList, cancelled when the run ends), in a fresh context of its own and a forked workspace, so neither its variables nor its tool calls touch the model's.

## Consequences

- Long, mechanical work costs one model turn, and only what the script chooses to return enters the context.
- The permission model is unchanged: a script can do nothing its tools could not, and asks the user exactly as the tools would.
- Like every feature flag, it is on unless turned off; the extra tool description costs prompt tokens on every request.
- A script's lane counts among the run's agent lanes: the TUI and the trace show it as an agent of kind `REPL`.
- A background script cannot read what the foreground scripts declared; it gets what it needs through its own code.
- A `const` cannot be redeclared in a later call; the model is told to pick another name.
- A context lives only in the process: it is lost on restart, and a resumed session starts with an empty one. The first script in a new context of a conversation that already ran scripts gets a note saying so.
- Calls that finished scripts leave unawaited are not answered; their effects (a write already served) are not undone.
