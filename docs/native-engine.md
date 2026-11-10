# Native engine

How Superfik's own engine (`superfik-native`) is configured and tested.
Its architecture is in [architecture.md](architecture.md) (Engines).

## Providers

| Provider | Id | Sign-in |
| --- | --- | --- |
| OpenAI API | `openai` | API key |
| ChatGPT Subscription | `openai-subscription` | browser, callback `http://localhost:1455/auth/callback` |
| Claude Subscription | `claude-subscription` | browser, callback `http://localhost:<free port>/callback` |
| Anthropic API | `anthropic` | API key |

The browser must reach the machine running the backend; sign-in waits up to
five minutes. Credentials stay in the backend's `SecretStore`; tokens are
renewed before they expire (ChatGPT also once after a 401) and never reach
the client. Contracts are pure in `superfik-providers`; HTTP in
`superfik-native/src/model/providers`; storage, sign-in and discovery in
`superfik-backend/src/providers/`. HTTP or JSON errors never
become a made-up catalog.

`effort` reaches every provider that supports it (`reasoning.effort`,
`output_config.effort`, or a thinking budget within `max_tokens` on older
models); levels a catalog refuses are rejected before HTTP. Switching
provider resends only compatible reasoning signatures.

## Project settings

`.superfik/settings.json` (project, then user; `.claude` equivalents are
read for compatibility, the native file and the project win):

- `outputStyle`; custom styles in `.superfik/output-styles/<name>.md`
  (`keep-coding-instructions: true` keeps the engineering instructions).
- `jsonSchema`: the model answers through `StructuredOutput`, validated and
  emitted as `EngineEvent::StructuredOutput`.
- `agent`: a definition run as the main agent (`initialPrompt` only before
  the first turn; its worktree persists for the session).
- `autoMemoryEnabled: false` (or `SUPERFIK_DISABLE_AUTO_MEMORY=1`) turns
  agent memory off.

Rules in `.superfik/rules` or `.claude/rules` may carry `paths` globs; new
rules found during a call are returned before the batch runs. Instruction
files load from the ancestors and, nested, when a tool touches their path;
`@path` imports are relative, inside the trusted folder, five levels deep,
1 MiB per file and 4 MiB in all. `/skill args` and Markdown commands are
expanded before the model call (`$ARGUMENTS`). Plan mode blocks mutations,
internal MCP tools included. The prompt identifies as `superfik-native/1`.

## Custom agents and hooks

Agents honour `tools`, `disallowedTools`, `mcpServers`, `permissionMode`,
`maxTurns`, `model`, `effort`, `skills`, `background`, `omitClaudeMd`,
`autoCompactWindow`, `memory` (`.superfik/agent-memory[-local]`, index
capped at 200 lines / 25 KB), `isolation: worktree` (removed only when
untouched) and hooks. Omitted tools stay blocked; hooks never lift Plan mode
or the parent's permission ceiling. Before every model call (main,
sub-agent, compaction, hook) the policy is checked again for the model
actually used.

Hook handlers `command`, `http`, `prompt` and `agent` run on PreToolUse,
PostToolUse, PostToolUseFailure, PermissionRequest, Stop, SubagentStart,
SubagentStop, PreCompact, PostCompact, UserPromptSubmit, SessionStart and
SessionEnd; anything else is refused. Exit code 2 or a JSON decision
blocks; PreToolUse may rewrite arguments (checked again); Stop may demand
more work. Hooks run with the user's environment and limits on time,
output and turns.

## Tests against real accounts

Ordinary tests are deterministic. With a provider connected (`superfik login`):

```sh
SUPERFIK_LIVE_PROVIDER=claude-subscription [SUPERFIK_LIVE_MODEL=…] \
  cargo test -p superfik-backend --test live_memory -- --ignored --nocapture
```

`SUPERFIK_LIVE_CREDENTIAL_FILE` supplies a private credential in isolated
environments; never put tokens in arguments or logs. These runs cost real
usage.
