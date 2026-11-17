# 0011 Code mode: the agent scripts its tools in a persistent JavaScript context

Status: Accepted, 2026-11-16

## Context

Every tool call is a model round trip: the whole conversation is read again, the model decides the next call, and the call's full output lands in the context and stays there. A task such as "find every caller of this function and change the argument" costs one turn per file and fills the context with files the model only needed to filter.

Other agents answer this by letting the model write code that calls tools as functions. Claude Code ships it as an experimental "Code Mode": a `REPL` tool backed by a sealed, persistent JavaScript context in which the tools are async functions, with output limited to what the script returns or logs. Anthropic's API has the same idea server-side ("programmatic tool calling"), but it runs in Anthropic's container and only for Anthropic models; this app speaks to several providers and runs tools on the user's machine.

The app already embeds QuickJS (`crates/script`) for script tools, hooks and prompts, and already routes a script's `ctx.tool(...)` through the agent's ordinary tool call. The goal is the same behaviour as Claude Code's REPL, built on that.

## Decision

Add a `REPL` tool, behind the `experimental-code-mode` feature flag, that runs locally for every provider.

**The context.** One QuickJS context per session and agent, on its own thread (`script::session::Session`). Scripts are evaluated as global async code in strict mode, so top-level `await` works and declarations persist between calls. There is no module loader and no host object: the only way out of the context is a tool call. Contexts are kept in a registry; past eight, one idle for ten minutes is let go, and one in use never is.

**Tools as functions.** Every tool in the request, except `REPL` itself and the tools that change the turn (plan mode, worktrees), is a non-writable global async function, also reachable as `tools["name"]`. A call is an ordinary `Agent::call`, so hooks, permission rules, asks, redaction and the transcript events apply to each one, and inner calls do not enter the conversation. As in Claude Code, a built-in tool's call resolves to its text, or to `{ error }` when it fails or is refused, and an MCP tool's call throws a `ToolError`. Read-only calls a script starts together run concurrently under the rule that parallelises a model's reply; the rest are served one at a time.

**Turn state.** Two pieces of turn state describe what the model itself did, so a script's calls are kept out of them: Read's record of lines the model has already seen (a script's read would otherwise make the model's own later read answer "unchanged") and the repeated-call guard (a script loops by design). Both are set aside while a script is served and restored after. Calls refused while a script runs are listed after its output.

**Helpers** (`content/repl/helpers.js`, evaluated once per context): `file(path).text()/.json()/.lines()/.exists()`, `write(path, text)`, `edit(path, old, new, { all })` and `` $`cmd` `` (Bash, values shell-quoted, throws on a failing exit, `.nothrow()` gives `{ stdout, exitCode }`). Each goes through its tool. `file()` needs the file's raw text, which Read does not give (it numbers lines, cuts long ones and answers "unchanged" on a second read), so the agent serves it as a Read call with a thread-local marker set (`tools::raw_text`): only the thread serving a helper sets it, so the model cannot ask for it. Under the marker Read returns the whole text, and the call skips what would change it (hook context appended to the output, injection neutralising, redaction); everything else about a Read call still applies, PreToolUse hooks and the refusal of secret files included. The text stays inside the script; what the script returns to the model is redacted and neutralised as the `REPL` call's own output, and the tool row shown in the TUI and the trace is redacted.

**Output.** What returns is the last expression's value (strings as they are, the rest as JSON, promises in the keys of a returned object awaited) and the `self.log` lines, each cut to 20,000 characters keeping head and tail. `console.*` goes to a ring the script reads with `console.tail`.

**Scripts side by side.** As in Claude Code, a script still running after 30 seconds is dispatched: the call returns `(dispatched #N)` and the script goes on in the same context, beside the scripts sent after it, as a background task of the run (TaskOutput, TaskStop, TaskList; uncollected ones are awaited and handed over before the turn ends). Claude Code tells concurrent scripts' logs apart with `AsyncLocalStorage`, which QuickJS lacks. Here each script is compiled under its own file name, `script-N`, and `self.log` and each tool call read the JavaScript stack: the innermost frame of a script still running owns it, and when none is on the stack (a function an earlier script declared, resumed after an `await`), the most recent running script does. One thread drives every running script's promise, delivers tool results, and ends a script on its own interrupt or timeout; when no call is outstanding and no job is pending, the scripts still waiting can never settle and end with that error.

**Limits.** 30 s of uninterrupted JavaScript (time waiting on tools excluded), a whole-script timeout of at most 10 minutes, 1,000 tool calls per script, 512 MB of memory; the user's interrupt stops the scripts.

**Two modes.** `experimental-code-mode` (on unless turned off, like every flag) adds `REPL` beside the other tools. `experimental-code-mode-only` is the first opt-in flag: off until the owner turns it on, and a project cannot turn it on. With it, the model is offered `REPL` plus the turn-level tools (`AskUserQuestion`, plan mode, worktrees); every other tool is described inside `REPL`'s description as a TypeScript-style `declare function`, generated from its JSON schema with its documentation, sorted by name so the same tools always give the same text and the prompt cache holds. The request still carries every tool, so a script's calls pass the same "offered" check as before. Deferred tools are not declared; a script finds them with `searchTools(words)`, which loads them as ToolSearch does and returns their declarations.

**Display.** A script's tool calls are shown the way a sub-agent's are: on its first call the script opens an agent lane of kind `REPL` (`AgentSpawned`), its calls travel as `AgentEvent`s inside it, and the lane ends with the script. That reuses what the TUI, the trace and the console already draw, with no change to the protocol; a script that calls no tool opens no lane.

## Consequences

- Long, mechanical work costs one model turn, and only what the script chooses to return enters the context.
- The permission model is unchanged: a script can do nothing its tools could not, and asks the user exactly as the tools would.
- Like every feature flag, `experimental-code-mode` is on unless turned off; the extra tool description costs prompt tokens on every request.
- A script's lane counts among the run's agent lanes: the TUI and the trace show it as an agent of kind `REPL`.
- Attribution by stack is a heuristic: a log or call made by a function an earlier script declared, after an `await`, goes to the most recent running script.
- A script's raw text never passes the redaction a tool result gets; it is redacted only on its way to the model, in the `REPL` output.
- A `const` cannot be redeclared in a later call; the model is told to pick another name.
- A context lives only in the process: it is lost on restart, and a resumed session starts with an empty one. The first script in a new context of a conversation that already ran scripts gets a note saying so.
- Calls that finished scripts leave unawaited are not answered; their effects (a write already served) are not undone.
