---
title: Hooks
description: Run your own commands, scripts, web requests or model checks at fixed points of a run to block, allow, rewrite or add context to what the agent does.
---

Hooks let you plug your own logic into {{NAME}}'s agent loop. A hook runs at a fixed point (before a tool call, when a prompt is sent, when the agent wants to stop, before a model call…), receives a JSON description of what is happening, and can:

- **block** the action, with a reason the agent sees;
- **allow** or **ask** for a tool call, overriding the usual permission prompt;
- **rewrite** the tool input, the tool output, the prompt, the model, the system prompt or the reply;
- **add context** the agent reads.

Hooks are deterministic: unlike instructions, the agent cannot forget or ignore them.

## Quick start

Refuse every `rm -rf` the agent tries to run. Save this as `~/{{DIR}}/hooks/no-rm-rf.js`:

```javascript
export const event = "PreToolUse";
export const matcher = "^Bash$";

export default function (input) {
  if (input.tool_input.command.includes("rm -rf")) {
    return { decision: "block", reason: "rm -rf is not allowed here; delete files one by one." };
  }
}
```

The next prompt picks it up. Run `/hooks` in the chat to see every hook that a run in this folder fires.

The same hook as a shell command in `~/{{DIR}}/settings.json`:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "^Bash$",
        "hooks": [
          { "type": "command", "command": "~/bin/no-rm-rf.sh", "timeout": 10 }
        ]
      }
    ]
  }
}
```

```bash
#!/bin/sh
if jq -e '.tool_input.command | contains("rm -rf")' >/dev/null; then
  echo "rm -rf is not allowed here; delete files one by one." >&2
  exit 2
fi
```

Exit code `2` blocks the call and the text on stderr is the reason the agent sees.

## Where hooks come from

All of these are combined; none replaces another.

| Source | Applies to |
|---|---|
| `~/{{DIR}}/settings.json`, under `"hooks"` | You, every project |
| `{{DIR}}/settings.json` and `{{DIR}}/settings.local.json`, under `"hooks"`, in the project and every folder above it | That project, unless git tracks the file (see [below](#hooks-a-repository-versions)) |
| `~/{{DIR}}/managed-settings.json`, under `"hooks"` | You, every project |
| `~/{{DIR}}/hooks/*.js` | You, every project |
| `{{DIR}}/hooks/*.js` in the project and every folder above it | That project, unless git tracks the file (see [below](#hooks-a-repository-versions)) |
| `hooks/hooks.json` in an installed plugin | Wherever the plugin is installed (see [Plugins](./plugins)) |
| `hooks` in a custom agent's frontmatter | Only while that agent runs (see [Agents](./agents)) |

Hooks are read again at the start of every prompt, so a change applies from your next message.

### Hooks a repository versions

A hook runs commands on your computer with your permissions, so {{NAME}} does not run hooks that arrive with a repository. A project file that git tracks (a checked-in `{{DIR}}/settings.json`, a committed `{{DIR}}/settings.local.json`, or a script in `{{DIR}}/hooks/`) has its hooks **skipped**. Everything else in that file still applies: its deny and ask rules, for example, hold as before.

When hooks are skipped, {{NAME}} tells you which files they came from:

- `/hooks` and `/permissions` end with a note naming the files;
- with `-p`, the same note is printed once to stderr before the first answer.

These hooks still run:

- your own, in `~/{{DIR}}/settings.json`, `~/{{DIR}}/managed-settings.json` and `~/{{DIR}}/hooks/`;
- project hooks in files git does not track, such as a `{{DIR}}/settings.local.json` you keep out of version control or a script you have not added;
- every project hook when the folder is not a git repository.

To run a repository's versioned hooks as before, set `trustProjectHooks` in **your** settings file, `~/{{DIR}}/settings.json`:

```json
{ "trustProjectHooks": true }
```

The key is read only from your own files in `~/{{DIR}}/`. A project cannot turn it on for itself.

> [!WARNING]
> `trustProjectHooks` applies to every repository you open. With it on, a checked-in `{{DIR}}/settings.json` or `{{DIR}}/hooks/` runs commands on your computer with your permissions: read them before working in a repository you do not trust.

> [!NOTE]
> Hooks in another tool's folders, such as `.claude/settings.json`, are not read.

### Reviewing hooks

`/hooks` lists every hook a run here fires, with its event, matcher (`*` for all) and what it runs: the command, the URL, or `prompt:`/`agent:` followed by the start of the prompt. A run with no hooks shows *No hooks. Add them under "hooks" in {{DIR}}/settings.json.* When a repository's versioned hooks were skipped, the list ends with a note naming the files they came from (see [Hooks a repository versions](#hooks-a-repository-versions)).

## Configuration format

In a settings file, `"hooks"` maps an event name to a list of **groups**. Each group has an optional `matcher` and a list of **handlers**:

```json
{
  "hooks": {
    "<Event>": [
      {
        "matcher": "<regex>",
        "hooks": [
          { "type": "command", "command": "…" }
        ]
      }
    ]
  }
}
```

A group accepts only `matcher` and `hooks`. A handler accepts:

| Field | Used by | Meaning |
|---|---|---|
| `type` | all | `command`, `script`, `http`, `prompt` or `agent` |
| `command` | `command`, `script` | The shell command, or the path of a `.js` file |
| `url` | `http` | An `http://` or `https://` address, without a user or password |
| `headers` | `http` | Request headers; values may use `$VAR` or `${VAR}` |
| `allowed_env_vars` | `http` | The environment variables `headers` may read |
| `prompt` | `prompt`, `agent` | What the model should check |
| `model` | `prompt`, `agent` | The model that checks it (the run's model by default) |
| `timeout` | all | Seconds, 1 to 600; 600 by default |
| `status_message` | all | Accepted; not shown by the chat today |

A settings file with an unknown event, an unknown handler type, a matcher that is not a valid regular expression, a missing `command`/`url`/`prompt`, or a timeout outside 1–600 is refused with a message that names the event and the handler number, for example `SessionStart hook #2: a prompt hook cannot run on SessionStart; use a command hook`. The run does not start until the file is fixed. A plugin's broken `hooks.json` is left out instead, with a warning in the log.

### Matchers

`matcher` is a regular expression tested against the event's subject (see the table below). An empty matcher, a missing one or `*` matches everything. The match is not anchored: `Edit` also matches `NotebookEdit`, so write `^Edit$` for one tool, or `^(Edit|Write)$` for several.

| Event | The matcher is tested against |
|---|---|
| `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `PermissionRequest` | The tool's name: `Bash`, `Edit`, `mcp__jira__search`… |
| `Stop`, `SubagentStop` | The agent's name: `main` for the conversation, else the sub-agent's (`general-purpose`, `Explore`, a custom name…) |
| `SubagentStart` | The sub-agent's kind |
| `SessionStart` | `startup` or `resume` |
| `PreCompact`, `PostCompact` | `manual` or `auto` |
| `Notification` | The notification type |
| `ModelRequest`, `ModelResponse`, `ModelError` | `main` or `sub-agent` |
| `UserPromptSubmit`, `SessionEnd`, `Brief` | An empty string: leave the matcher empty |

Groups run in order; inside a group, handlers run in order. As soon as one blocks, the rest are skipped.

## Hook events

| Event | When it fires | Can it block? |
|---|---|---|
| [`Brief`](#brief) | Before each prompt, once skills, agents, rules and instruction files are found | Yes: the run does not start |
| [`SessionStart`](#sessionstart) | At the start of each run | Yes: the run does not start |
| [`UserPromptSubmit`](#userpromptsubmit) | When the prompt is sent, before the model sees it | Yes: the run does not start |
| [`ModelRequest`](#modelrequest) | Before every model call | Yes: the run fails |
| [`ModelResponse`](#modelresponse) | When a reply arrives, before it is shown or acted on | Yes: the model answers again |
| [`ModelError`](#modelerror) | When a model call fails | Yes: the run fails with the error |
| [`PreToolUse`](#pretooluse) | Before a tool runs | Yes: the call is refused |
| [`PermissionRequest`](#permissionrequest) | When the agent would ask you for permission | Yes: the call is refused |
| [`PostToolUse`](#posttooluse) | After a tool succeeds | Gives feedback: the result is marked failed |
| [`PostToolUseFailure`](#posttoolusefailure) | After a tool fails | Gives feedback |
| [`Notification`](#notification) | When {{NAME}} needs your attention | No |
| [`SubagentStart`](#subagentstart) | When a sub-agent starts | Yes: the sub-agent fails |
| [`SubagentStop`](#subagentstop) | When a sub-agent wants to finish | Yes: it keeps working |
| [`Stop`](#stop) | When the main agent wants to finish | Yes: it keeps working |
| [`PreCompact`](#precompact) | Before the conversation is summarised | Yes: compaction is cancelled |
| [`PostCompact`](#postcompact) | After the conversation is summarised | No |
| [`SessionEnd`](#sessionend) | At the end of each run | No |

## Hook input

A hook receives a JSON object. Every event includes:

| Field | Value |
|---|---|
| `hook_event_name` | The event, such as `PreToolUse` |
| `session_id` | The conversation's id |
| `agent_type` | The running agent: `main` for the conversation, else the sub-agent's name |
| `cwd` | The project folder |

Each event adds its own fields, listed below. A command hook reads the object on stdin; an HTTP hook receives it as the request body; a script hook receives it as its first argument.

## Hook output

### Command hooks: exit codes

| Exit code | Effect |
|---|---|
| `0` | Success. stdout is read as JSON (see below). For `SessionStart` and `UserPromptSubmit`, stdout that is not JSON is added as context for the agent. |
| `2` | Block. stderr is the reason. The remaining hooks of the event are skipped. |
| Anything else | The hook failed. `Hook failed: <stderr>` is added to the context the agent sees, and the next hook runs. For `Brief`, `ModelRequest`, `ModelResponse` and `ModelError` the failure is only written to the log. |

A hook that times out or is cancelled is an error, which [each event handles in its own way](#errors-and-timeouts).

### JSON output

A hook that exits `0` (or an HTTP hook that answers `2xx`, or a script that returns a value) may print a JSON object:

```json
{
  "decision": "block",
  "reason": "Why it was blocked",
  "continue": false,
  "stopReason": "Why it was stopped",
  "systemMessage": "Text added to the agent's context",
  "hookSpecificOutput": {
    "additionalContext": "More text added to the agent's context",
    "permissionDecision": "allow",
    "permissionDecisionReason": "Why",
    "updatedInput": {},
    "updatedToolOutput": "…",
    "decision": { "behavior": "allow", "updatedInput": {} },
    "updatedPrompt": "…",
    "model": "provider:model",
    "effort": "high",
    "system": "…",
    "history": [],
    "text": "…"
  }
}
```

| Field | Effect |
|---|---|
| `decision: "block"`, `continue: false` or `hookSpecificOutput.permissionDecision: "deny"` | Blocks. The reason is `reason`, else `stopReason`, else `permissionDecisionReason`. |
| `systemMessage`, `hookSpecificOutput.additionalContext` | Added to the context the agent sees (where depends on the event). |
| `hookSpecificOutput.permissionDecision` | `PreToolUse`: `allow`, `deny` or `ask`. |
| `hookSpecificOutput.decision.behavior` | `PermissionRequest`: `allow`, `deny` or `ask`. |
| `hookSpecificOutput.updatedInput` | `PreToolUse`: replaces the tool's input (must be an object). |
| `hookSpecificOutput.decision.updatedInput` | `PermissionRequest`: proposes new input. |
| `hookSpecificOutput.updatedToolOutput` | `PostToolUse`: replaces the tool's result. |
| `hookSpecificOutput.updatedPrompt` | `UserPromptSubmit`: replaces the prompt. |
| `hookSpecificOutput.model` | `ModelRequest`, `ModelError`: the model to use. |
| `hookSpecificOutput.effort` | `ModelRequest`: `low`, `medium`, `high`, `xhigh` or `max`. |
| `hookSpecificOutput.system`, `hookSpecificOutput.history` | `ModelRequest`: replace the system prompt or the messages. |
| `hookSpecificOutput.text` | `ModelResponse`: replaces the reply's text. |

A permission decision other than `allow`, `deny` or `ask` is an error.

### Errors and timeouts

When a hook cannot run, times out or is cancelled:

| Event | What happens |
|---|---|
| `PreToolUse` | The tool call is refused: `Hook blocked <tool>: hook cancelled or timed out` |
| `PostToolUse`, `PostToolUseFailure` | The result is marked failed, with `Hook failed: …` added |
| `PermissionRequest` | The tool call fails with the error |
| `SessionStart`, `UserPromptSubmit`, `Stop`, `SubagentStop`, `Brief` | The run stops with the error |
| `SubagentStart` | The sub-agent fails |
| `PreCompact`, `PostCompact` | Compaction fails |
| `ModelRequest`, `ModelResponse`, `ModelError` | Logged; the call goes on as if there were no hook |
| `Notification`, `SessionEnd` | Logged and ignored |

## Handler types

### `command`

Runs a shell command (`/bin/sh -c` on macOS and Linux). The command:

- runs in the project folder;
- gets the input JSON on stdin;
- gets `{{ENV}}_PROJECT_DIR` set to the project folder;
- is confined like the agent's `Bash` tool when the sandbox is on (see [Sandbox and safety](./sandbox-and-safety));
- has stdout and stderr read up to 64 KiB each.

```json
{ "type": "command", "command": "\"${{{ENV}}_PROJECT_DIR}\"/scripts/check.sh", "timeout": 30 }
```

### `script`

Runs a JavaScript file in {{NAME}}'s built-in script runtime, without starting a process. `command` must be the path of a `.js` file; use an absolute path. The file's default export is called with the input and a context object, and its return value is the hook's output:

```javascript
export default function (input, ctx) {
  if (input.tool_input.command.startsWith("git push")) {
    return { decision: "block", reason: "Push from your own terminal." };
  }
}
```

| Return value | Treated as |
|---|---|
| Nothing (`undefined` or `null`) | Exit `0`, no output |
| A string | Exit `0`, that string as stdout |
| An object | Exit `0`, the object as JSON output |
| A thrown error | A failed hook (like a non-zero exit), the error as stderr |

The context gives `ctx.cwd`, `ctx.session` and `ctx.agent`, plus file, process and network helpers (`ctx.readFile`, `ctx.writeFile`, `ctx.exec`, `ctx.fetch`, `ctx.log`) limited by the `scripts` setting. While the event runs alongside the model (tool, prompt, stop and compaction events), the script can also call `ctx.llm(…)` to ask the model and `ctx.tool(name, input)` to use a read-only tool inside the project.

The type definitions for every event's input and output are in {{NAME}}'s `content/types.d.ts` (`Hook<"PreToolUse">` and so on).

### Hook script folders

Instead of listing a script in settings, drop it in a hooks folder (`~/{{DIR}}/hooks/` or `{{DIR}}/hooks/`). The file names its event, and optionally a matcher and a timeout, with exports:

```javascript
export const event = "PostToolUse";
export const matcher = "^(Edit|Write)$";
export const timeout = 30;

export default (input, ctx) => {
  const result = ctx.exec(`npx prettier --check ${JSON.stringify(input.tool_input.file_path)}`);
  if (result.code !== 0) {
    return { hookSpecificOutput: { additionalContext: "Run prettier on that file." } };
  }
};
```

| Export | Meaning |
|---|---|
| `event` | One event name, or a list of them. Required. |
| `matcher` | Like a settings matcher. Empty by default. |
| `timeout` | Seconds, 1 to 600. 60 by default. |
| `default` | The hook function. A file whose default export is missing or an empty function is skipped. |

{{NAME}} ships one empty template per event. A file in `~/{{DIR}}/hooks/` with the same name as a template (for example `PreToolUse.js`) takes its place.

> [!NOTE]
> A script in a project's `{{DIR}}/hooks/` cannot read or write files, start processes or reach the network (`ctx.readFile`, `ctx.exec`, `ctx.fetch`…) until one of your own settings files grants it. Scripts in `~/{{DIR}}/hooks/` can. See [`scripts`](./settings#scripts).

### `http`

POSTs the input JSON to a URL. A `2xx` answer's body is read like a command's stdout (up to 64 KiB, UTF-8). Any other status is a failed hook. An HTTP hook blocks by answering JSON such as `{"decision": "block", "reason": "…"}`.

```json
{
  "type": "http",
  "url": "https://hooks.example.com/agent",
  "headers": { "Authorization": "Bearer ${HOOK_TOKEN}" },
  "allowed_env_vars": ["HOOK_TOKEN"],
  "timeout": 15
}
```

A header can read an environment variable as `$NAME` or `${NAME}` only if `allowed_env_vars` lists it; any other variable becomes empty.

### `prompt` and `agent`

Ask a model whether a condition holds. The model gets your `prompt` and the hook input (where the prompt says `$ARGUMENTS`, or appended after it), and must answer a JSON object `{"ok": true|false, "reason": "…"}`. `ok: false` blocks with that reason; an answer that is not such an object is an error.

- `prompt` asks the model once.
- `agent` runs a small read-only agent first: it can use `Read`, `Glob` and `Grep` (those the run offers), in plan mode, for up to 50 turns.

```json
{
  "Stop": [
    {
      "hooks": [
        {
          "type": "agent",
          "prompt": "Check that every file the agent changed has a matching test. $ARGUMENTS",
          "timeout": 120
        }
      ]
    }
  ]
}
```

`prompt` and `agent` hooks work on events that run alongside the model: `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `PermissionRequest`, `UserPromptSubmit`, `Stop` and `SubagentStop`. They are refused on `Brief`, `SessionStart`, `SessionEnd`, `SubagentStart`, `PreCompact`, `PostCompact`, `ModelRequest`, `ModelResponse` and `ModelError`.

## Event reference

### Brief

Fires before each prompt, once {{NAME}} has found the skills, agents, rules and instruction files, and before the system prompt is built. Use it to add, change or remove them, for example to inject team rules from a server.

Input adds:

| Field | Value |
|---|---|
| `skills` | `[{ name, description, path, body, disableModelInvocation }]` |
| `agents` | `[{ name, description, instructions, config }]` |
| `rules` | `[{ path, base, text, patterns, error }]` |
| `instructions` | `[{ path, scope, text }]`, `scope` being `User`, `Project` or `Local` |

Output: return any of `skills`, `agents`, `rules` and `instructions` at the top level of the JSON (not under `hookSpecificOutput`). A list you return replaces that list; one you leave out stays as it was.

- `skills`: each entry needs a `name`. An entry with a known name keeps that skill and changes the fields you give (`description`, `body`, `disableModelInvocation`); a new name needs a `description` and a `body`. Skills you leave out of the list are removed.
- `agents`, `rules`, `instructions`: the list replaces the current one. An agent name may appear once.

Brief hooks run one after the other, each seeing the previous one's result. A block stops the session: `a Brief hook stopped the session: <reason>`.

```javascript
export const event = "Brief";
export default (input) => ({
  rules: [...input.rules, { path: "/team/rust.md", text: "No unwrap.", patterns: ["**/*.rs"] }],
  agents: input.agents.filter((agent) => agent.name !== "Plan"),
});
```

### SessionStart

Fires at the start of each run (each prompt you send).

| Input field | Value |
|---|---|
| `source` | `resume` when the run continues an earlier conversation, else `startup` |
| `model` | The model's id |

Context from the hook (JSON `additionalContext`/`systemMessage`, or plain stdout) is added to your message. A block stops the run with the reason.

### UserPromptSubmit

Fires when a prompt is sent, after any `/skill` or command in it has been expanded, before the model sees it.

| Input field | Value |
|---|---|
| `prompt` | The prompt's text |

- `hookSpecificOutput.updatedPrompt` replaces the prompt.
- Context (JSON or plain stdout) is added to the message.
- A block stops the run with the reason.

```bash
#!/bin/sh
echo "Current branch: $(git branch --show-current)"
```

### ModelRequest

Fires before every model call, for the main agent and sub-agents.

| Input field | Value |
|---|---|
| `model` | The model about to be called |
| `kind` | `main` or `sub-agent` |
| `mode` | `ask`, `edits`, `plan` or `bypass` |
| `messages` | How many messages are sent |
| `effort` | The effort level |
| `maxTokens` | The reply's token limit |
| `tools` | The names of the tools offered |
| `system` | The system prompt |
| `history` | The messages, as `{ role, blocks }` objects |

Output (`hookSpecificOutput`): `model` sends the call to another model (`provider:model` to change provider), `effort` changes the effort, `system` and `history` replace the system prompt and the messages, `additionalContext` is appended to the system prompt of that call. A block stops the run: `A ModelRequest hook blocked the call: <reason>`.

```javascript
export const event = "ModelRequest";
export const matcher = "sub-agent";
export default () => ({ hookSpecificOutput: { model: "cheap-provider:small-model" } });
```

### ModelResponse

Fires when a reply arrives, before it is shown or acted on. While a `ModelResponse` hook is configured, the reply's text appears once the hook has answered instead of streaming.

Input adds the `ModelRequest` fields `model`, `kind`, `mode` and `messages`, and:

| Input field | Value |
|---|---|
| `text` | The reply's text |
| `toolCalls` | `[{ name, input }]` |
| `stopReason` | `end_turn`, `tool_use`, `max_tokens`, `refusal` or `pause_turn` |
| `usage` | `{ input, output }` tokens |
| `rejections` | How many replies were rejected in a row |

- `hookSpecificOutput.text` replaces the reply's text (for example to redact a secret).
- A block drops the reply and asks the model again, telling it the reason. The model is asked again at most twice in a row; a further rejection fails the run.

```javascript
export const event = "ModelResponse";
export default (input) => {
  const text = input.text.replace(/sk-[A-Za-z0-9]{8,}/g, "[redacted]");
  if (text !== input.text) return { hookSpecificOutput: { text } };
};
```

### ModelError

Fires when a model call fails, before the `fallbackModel` list is used.

Input adds `model`, `kind`, `mode`, `messages` and:

| Input field | Value |
|---|---|
| `error` | The error message |
| `errorKind` | `overloaded`, `unauthorized`, `network`, `provider` or `too_long` |
| `attempt` | How many models have failed since the last answer |
| `tried` | The models that failed since the last answer |
| `fallbacks` | The `fallbackModel` list from settings |

Answer `hookSpecificOutput.model` to forward the call to that model, block to stop the run with the error, or nothing to let `fallbackModel` decide. A model already in `tried` is not tried again before a call succeeds.

```javascript
export const event = "ModelError";
export const matcher = "main";
const chain = ["backup-provider:other-model", "third-provider:model"];
export default (input) => {
  if (input.errorKind === "too_long") return;
  const next = chain.find((model) => !input.tried.includes(model));
  if (next) return { hookSpecificOutput: { model: next } };
};
```

See [Models and providers](./models-and-providers) for `fallbackModel`.

### PreToolUse

Fires before a tool runs, for tools the agent is offered.

| Input field | Value |
|---|---|
| `tool_name` | The tool, such as `Bash` or `mcp__github__create_issue` |
| `tool_use_id` | The call's id |
| `tool_input` | The tool's arguments |

- A block refuses the call; the agent sees `Hook blocked <tool>: <reason>`.
- `permissionDecision: "allow"` runs the call without asking, unless a permission rule says to ask. `"ask"` always asks you. `"deny"` refuses it. A `deny` rule in settings always wins.
- `updatedInput` replaces the arguments. If the new input touches a path with rules the agent has not seen yet, the call is turned back so the agent can read them first.
- Context is appended to the tool's result as `Hook context: …`.

```json
{
  "hookSpecificOutput": {
    "permissionDecision": "allow",
    "permissionDecisionReason": "Read-only git command"
  }
}
```

### PermissionRequest

Fires when a tool call would show you a permission prompt (after rules and modes have been checked), before the prompt appears.

Input: `tool_name`, `tool_input`, `tool_use_id`.

Answer with `hookSpecificOutput.decision`:

- `{"behavior": "allow"}` approves the call without asking, unless a permission rule says to ask;
- `{"behavior": "deny"}` refuses it (`permissionDecisionReason` is the reason);
- `{"behavior": "ask"}` shows the prompt as usual.

If `decision.updatedInput` differs from the input, the call is not run: the agent is asked to issue it again with those arguments, so path rules are checked on them.

### PostToolUse

Fires after a tool returns successfully.

Input: `tool_name`, `tool_use_id`, `tool_input` and `tool_response` (the result's text).

- `updatedToolOutput` replaces the result the agent sees (a string, or any JSON value, which is passed as text).
- A block marks the result as failed and appends `Hook feedback: <reason>`, so the agent can correct itself.
- Context is appended to the result.

```javascript
export const event = "PostToolUse";
export const matcher = "^(Edit|Write)$";
export default (input, ctx) => {
  const out = ctx.exec("cargo clippy --quiet -- -D warnings");
  if (out.code !== 0) return { decision: "block", reason: out.stderr.slice(0, 2000) };
};
```

### PostToolUseFailure

Like `PostToolUse`, for a tool call that failed (including one a `PreToolUse` hook blocked). Same input and output.

### Notification

Fires when {{NAME}} needs you. Its output is ignored, which makes it a good place for desktop notifications or sounds.

| Input field | Value |
|---|---|
| `notification_type` | `permission_prompt`, `elicitation_dialog` (the agent has questions) or `plan_approval` (a plan is ready) |
| `message` | A short text, such as *{{NAME}} needs your permission to use Bash* |

```json
{
  "hooks": {
    "Notification": [
      { "hooks": [{ "type": "command", "command": "osascript -e 'display notification \"{{NAME}} needs you\"'" }] }
    ]
  }
}
```

### SubagentStart

Fires when a sub-agent starts (not when a background agent is resumed).

| Input field | Value |
|---|---|
| `agent_type` | The sub-agent's kind |
| `prompt` | The task it was given |

Context is added to the sub-agent's conversation. A block makes the sub-agent fail with the reason.

### SubagentStop

Fires when a sub-agent wants to finish. Same input and behaviour as [`Stop`](#stop). A `Stop` hook declared in a custom agent's frontmatter runs here.

### Stop

Fires when the main agent is about to end its turn.

| Input field | Value |
|---|---|
| `last_assistant_message` | The agent's last message |
| `stop_hook_active` | `true` when the agent is already continuing because a stop hook blocked |

A block keeps the agent working: it receives `Completion blocked by hook: <reason>` plus any context. Check `stop_hook_active` to avoid sending the agent around in circles.

```bash
#!/bin/sh
input=$(cat)
if [ "$(printf '%s' "$input" | jq -r .stop_hook_active)" = "true" ]; then exit 0; fi
if ! cargo test --quiet >/dev/null 2>&1; then
  echo "The tests fail; fix them before finishing." >&2
  exit 2
fi
```

### PreCompact

Fires before the conversation is summarised, by `/compact` or automatically.

| Input field | Value |
|---|---|
| `trigger` | `manual` or `auto` |
| `custom_instructions` | What `/compact <text>` asked to keep (empty otherwise) |
| `context_tokens` | The conversation's estimated size |

A block cancels the compaction: `Compaction blocked by hook: <reason>`.

### PostCompact

Fires after the conversation was summarised. Same input as `PreCompact`. Context is added to the conversation after the summary.

### SessionEnd

Fires at the end of each run, whatever its outcome. Input has only the common fields. Its output is ignored.

## Environment variables

| Variable | Set for | Value |
|---|---|---|
| `{{ENV}}_PROJECT_DIR` | `command` hooks | The project folder |
| `{{ENV}}_PLUGIN_ROOT` | Plugin `hooks.json` | Written as `${{{ENV}}_PLUGIN_ROOT}`, replaced by the plugin's folder before the hook runs |

An HTTP hook's `headers` can read only the variables listed in its `allowed_env_vars`.

## Examples

### Format every edited file

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "^(Edit|Write)$",
        "hooks": [
          { "type": "command", "command": "jq -r .tool_input.file_path | xargs npx prettier --write >/dev/null" }
        ]
      }
    ]
  }
}
```

### Allow read-only git commands without asking

```javascript
export const event = "PreToolUse";
export const matcher = "^Bash$";
export default (input) => {
  if (/^git (status|diff|log|show)\b/.test(input.tool_input.command)) {
    return { hookSpecificOutput: { permissionDecision: "allow" } };
  }
};
```

### Protect files

```javascript
export const event = "PreToolUse";
export const matcher = "^(Edit|Write|NotebookEdit)$";
export default (input) => {
  const path = input.tool_input.file_path ?? "";
  if (path.endsWith(".env") || path.includes("/migrations/")) {
    return { decision: "block", reason: `${path} is protected.` };
  }
};
```

### Ask a model whether a command is safe

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "^Bash$",
        "hooks": [
          {
            "type": "prompt",
            "prompt": "Does this shell command only read files, without deleting or uploading anything? $ARGUMENTS",
            "timeout": 30
          }
        ]
      }
    ]
  }
}
```

## See also

- [Permissions](./permissions) for allow, ask and deny rules.
- [Settings](./settings) for the files hooks live in.
- [Plugins](./plugins) to share hooks.
- [Agents](./agents) for hooks scoped to one agent.
