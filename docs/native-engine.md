# Native engine

How the app's own engine (`agent-engine`) is configured and tested.
Its architecture is in [architecture.md](architecture.md) (Engines).

## Providers

Each provider is one folder, `agent-engine/src/providers/<name>/`, with a
README saying how it signs in and what it speaks; `{command} login` lists
the ones built in. A provider names its vendor (prices, short model names,
reasoning rules in `llm-providers/src/vendors/<vendor>/`) and its wire (the
shared ones in `agent-engine/src/model/wires/`, or its own `wire/`).
Removing the folder removes the provider; nothing else names it.

The browser must reach the machine running the backend; sign-in waits up to
five minutes. Credentials stay in the backend's `SecretStore`; tokens are
renewed before they expire and never reach the client. What providers
share is in `llm-providers`; the client and transport in
`agent-engine/src/model/client`; storage, sign-in and discovery in
`backend/src/providers/`. HTTP or JSON errors never
become a made-up catalog.

`effort` reaches every provider that supports it (`reasoning.effort`,
`output_config.effort`, or a thinking budget within `max_tokens` on older
models); levels a catalog refuses are rejected before HTTP. Switching
provider resends only compatible reasoning signatures.

## Project settings

`.{command}/settings.json` (project, then user; the project wins). Nothing
in `.claude/` or `CLAUDE.md` is read:

- `outputStyle`; custom styles in `.{command}/output-styles/<name>.md`
  (`keep-coding-instructions: true` keeps the engineering instructions).
- `jsonSchema`: the model answers through `StructuredOutput`, validated and
  emitted as `EngineEvent::StructuredOutput`.
- `agent`: a definition run as the main agent (`initialPrompt` only before
  the first turn; its worktree persists for the session).
- `autoMemoryEnabled: false` (or `{COMMAND}_DISABLE_AUTO_MEMORY=1`) turns
  agent memory off.

Rules in `.{command}/rules` may carry `paths` globs; new
rules found during a call are returned before the batch runs. Instruction
files (`{COMMAND}.md`, `.{command}/{COMMAND}.md`, `{COMMAND}.local.md`,
`AGENTS.md`; the user's `~/.{command}/{COMMAND}.md`) load from the ancestors and, nested, when a tool touches their path;
`@path` imports are relative, inside the trusted folder, five levels deep,
1 MiB per file and 4 MiB in all. `/skill args` and Markdown commands are
expanded before the model call (`$ARGUMENTS`). Plan mode blocks mutations,
internal MCP tools included. The prompt identifies as `agent-engine/1`.

## Custom agents and hooks

Agents honour `tools`, `disallowedTools`, `mcpServers`, `permissionMode`,
`maxTurns`, `model`, `effort`, `skills`, `background`, `omitClaudeMd`,
`autoCompactWindow`, `memory` (`.{command}/agent-memory[-local]`, index
capped at 200 lines / 25 KB), `isolation: worktree` (removed only when
untouched) and hooks. Omitted tools stay blocked; hooks never lift Plan mode
or the parent's permission ceiling. Before every model call (main,
sub-agent, compaction, hook) the policy is checked again for the model
actually used.

Hook handlers `command`, `script`, `http`, `prompt` and `agent` run on
PreToolUse, PostToolUse, PostToolUseFailure, PermissionRequest, Stop,
SubagentStart, SubagentStop, PreCompact, PostCompact, UserPromptSubmit,
SessionStart, SessionEnd, Notification, ModelRequest, ModelResponse and
ModelError;
anything else is refused, and a refusal names the file, event, hook and
reason. Exit code 2 or a JSON decision blocks; PreToolUse may rewrite
arguments (checked again); Stop may demand more work. Hooks run with the
user's environment and limits on time, output and turns.

A `script` hook (a `.js` file) gets the same `ctx` as a script tool. On
events that run with a model at hand (the tool, permission, stop, session
and compaction ones), `ctx.llm` asks the run's model and `ctx.tool` runs a
read-only tool (Read, Grep, Glob, LSP…) inside the project; anything else
is refused. The script runs on its own thread and each such call is served
by the caller (`hooks/script_hook.rs`, `tools/script_bridge.rs`).

The three model hooks intercept every model call of the loop, like a
proxy: ModelRequest before it is sent, ModelResponse when a reply
arrives, ModelError when it fails (after the provider's own retries).
Their input has `model`, `kind` (`main` or `sub-agent`, also the
matcher's subject), `mode` and `messages` (a count), plus:

- ModelRequest: `effort`, `maxTokens`, `tools` (names). It may answer
  `{"hookSpecificOutput": {"model": "<provider>:<model>"}}` to route the
  call (see `domain::routing`), `"effort": "low"` to change the effort,
  `"additionalContext"` (or a top-level `systemMessage`) to append to the
  system prompt of this call, or `{"decision": "block"}` to stop the run.
- ModelResponse: `text`, `toolCalls` (`name`, `input`), `stopReason`,
  `usage` and `rejections`. `"text"` replaces the reply's text (tool calls
  are kept); `{"decision": "block", "reason": …}` drops the reply and asks
  again with the reason added to the system prompt, at most
  `domain::interception::MAX_REJECTIONS` times before the run fails. While
  a ModelResponse hook is configured, reply text is shown when the hook
  has answered, not streamed.
- ModelError: `error`, `errorKind` (`overloaded`, `unauthorized`,
  `network`, `provider`, `too_long`), `attempt`, `tried` (the models that
  failed since the last answer) and `fallbacks` (the settings' chain).
  `"model"` forwards the failed call to that model, `{"decision":
  "block"}` ends the run with the error, and no answer leaves it to the
  `fallbackModel` chain, which takes only overloads. The policy is
  `domain::fallback`: a model is not tried twice before an answer arrives,
  and at most `MAX_SWITCHES` models are tried in a row.

Only command, script and http handlers run on them, so a model hook never
asks a model.

## Tests against real accounts

Ordinary tests are deterministic. With a provider connected (`{command} login`):

```sh
{COMMAND}_LIVE_PROVIDER=<provider id> {COMMAND}_LIVE_MODEL=<model> \
  cargo test -p backend --test live_memory -- --ignored --nocapture
```

`{COMMAND}_LIVE_CREDENTIAL_FILE` supplies a private credential in isolated
environments; never put tokens in arguments or logs. These runs cost real
usage.
