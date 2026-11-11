# Native engine

How the app's own engine (`agent-engine`) is configured and tested.
Its architecture is in [architecture.md](architecture.md) (Engines).

## Providers

| Provider | Id | Sign-in |
| --- | --- | --- |
| OpenAI API | `openai` | API key |
| ChatGPT Subscription | `openai-subscription` | browser, callback `http://localhost:1455/auth/callback` |
| Claude Subscription | `claude-subscription` | browser, callback `http://localhost:<free port>/callback` |
| Anthropic API | `anthropic` | API key |
| GitHub Copilot Subscription | `copilot-subscription` | GitHub device flow (code typed at `github.com/login/device`), or a GitHub token |
| Kiro Subscription | `kiro-subscription` | browser via `app.kiro.dev/signin`, callback `http://localhost:49153/oauth/callback` |

Kiro is spoken as Kiro CLI 3.1.0 does. Sign-in opens Kiro's portal with
a PKCE challenge and `redirect_uri=http://localhost:49153` (Kiro's sign-in
accepts only that port); the portal returns
`/oauth/callback?login_option=…&code=…`, and Google or GitHub codes are
exchanged at `prod.us-east-1.auth.desktop.kiro.dev/oauth/token` for the
redirect including `?login_option=…`. The answer's `profileArn` names the
account and its region; tokens refresh at `/refreshToken` (also on a 401
or 403). Organization (IAM Identity Center) and Builder ID sign-ins are
refused for now. Turns are `GenerateAssistantResponse` on
`runtime.<region>.kiro.dev` (AWS JSON 1.0, `x-amz-target`), answered as
AWS event-stream frames; the catalog is `ListAvailableModels` on
`management.<region>.kiro.dev` with `origin: KIRO_CLI`. Kiro has no system
field, so the system prompt opens the first user message, and it reports
context use as a share of the window, from which the prompt's size is
estimated.

GitHub Copilot is spoken as the Copilot CLI does (client
`Ov23ctDVkRmgkPke0Mmm`, `Copilot-Integration-Id: copilot-developer-cli`):
the GitHub token goes as is to the Copilot API named by
`copilot_internal/user` (`endpoints.api`); it is never exchanged or
refreshed, and classic `ghp_` tokens are refused. `GET /models` lists what
the picker shows; each model runs on the best API its
`supported_endpoints` offer (`/v1/messages`, then `/responses`, then
`/chat/completions`). Claude on `/v1/messages` follows the CLI's
`getAnthropicRequestParams`: a model with reasoning effort gets adaptive
thinking and `output_config.effort` (clamped to the levels it offers), one
that only thinks gets a 1024/2048/4096/32000-token budget. `X-Initiator` is `user` only when the turn starts
with the user's own message, so tool round-trips are not premium requests.

The browser must reach the machine running the backend; sign-in waits up to
five minutes. Credentials stay in the backend's `SecretStore`; tokens are
renewed before they expire (ChatGPT also once after a 401) and never reach
the client. Contracts are pure in `llm-providers`; HTTP in
`agent-engine/src/model/providers`; storage, sign-in and discovery in
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
  `{"hookSpecificOutput": {"model": "openai:gpt-6.1-sol"}}` to route the
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
{COMMAND}_LIVE_PROVIDER=claude-subscription [{COMMAND}_LIVE_MODEL=…] \
  cargo test -p backend --test live_memory -- --ignored --nocapture
```

`{COMMAND}_LIVE_CREDENTIAL_FILE` supplies a private credential in isolated
environments; never put tokens in arguments or logs. These runs cost real
usage.
