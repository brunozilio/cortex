---
title: Headless mode
description: Run {{NAME}} without the chat with --print, read its answer as text or JSON, and use it in scripts and CI.
---

Print mode runs one request without opening the chat: {{NAME}} works on the prompt, prints the answer and exits. It is the way to use {{NAME}} from scripts, Git hooks and CI.

## Get one answer

```bash
{{COMMAND}} -p "explain src/main.rs"
```

`-p` (or `--print`) takes the prompt from the command line; every word after the options is joined with spaces. The answer streams to standard output as it is written. The tools the agent starts are shown on standard error, dimmed, as `● <Tool>(<summary>)`, so they do not mix with the answer when you redirect it:

```bash
{{COMMAND}} -p "list the TODOs in this repository" > todos.md
```

Put `--` before a prompt that starts with a dash:

```bash
{{COMMAND}} -p -- "-v does what?"
```

## Choose what the agent may do

Nobody is there to answer a permission prompt, so in print mode every request for permission is denied, and a note says so on standard error:

```text
denied: the agent asked for permission (use --mode or --allowedTools)
```

Give the run what it needs up front:

| Option | Effect |
|---|---|
| `--mode ask` | Every tool that needs permission is denied |
| `--mode edits` | File edits are allowed; other requests are denied. This is the mode used when `--mode` is not given. |
| `--mode plan` | The agent plans without changing anything (see below) |
| `--mode bypass` | Nothing asks for permission |
| `--allowedTools "<rules>"` | These rules are allowed without asking |
| `--disallowedTools "<rules>"` | These rules are refused |

```bash
{{COMMAND}} -p --allowedTools "Bash(cargo test:*) Edit" "make the failing test pass"
```

Rules are separated by spaces or commas (not inside parentheses), and both options can be repeated. The rule syntax is described in [Permissions](./permissions).

### Project hooks, scripts and servers need `--trust`

Print mode never asks whether you trust the folder. Unless the folder is already trusted, the run loads nothing from the project that runs code or grants permissions (hooks, script tools, MCP servers, declared providers, `settings.local.json` grants), and one line on standard error says how many items were skipped. Pass `--trust` to load them for that invocation; nothing is stored. When the folder cannot be trusted (your home folder, for example), a line `--trust ignored: <why>` says so on standard error and the run goes on untrusted:

```bash
{{COMMAND}} -p --trust "run the release checks"
```

> [!WARNING]
> A script or CI job that relies on the project's hooks must pass `--trust`. Use it only where you control what the repository contains. See [Workspace trust](./sandbox-and-safety#workspace-trust).

> [!WARNING]
> `--mode bypass` lets the agent run any command without asking. Use it only in a disposable environment, such as a CI container or a scratch checkout. See [Sandbox and safety](./sandbox-and-safety).

### Plan mode in print

With `--mode plan`, when the agent finishes its plan, the plan is printed and not carried out:

```text
plan not carried out: run again with --mode edits to implement it
```

## Output formats

`--output-format` chooses what standard output carries. It needs `--print`.

| Format | Standard output |
|---|---|
| `text` (default) | The answer as it streams, then a newline |
| `json` | One JSON object with the result, when the run ends |
| `stream-json` | Every event as a JSON line, the notes as JSON lines, then the result |

### `json`

```bash
{{COMMAND}} -p --output-format json "what does this project do?"
```

```json
{
  "type": "result",
  "subtype": "success",
  "is_error": false,
  "result": "This project is ...",
  "error": null,
  "session_id": "0f9c2d3e-...",
  "total_cost_usd": 0.0123,
  "duration_ms": 8421
}
```

| Field | Type | Meaning |
|---|---|---|
| `type` | string | Always `result` |
| `subtype` | string | `success`, or `error` when the run failed or a verification failed |
| `is_error` | boolean | `true` when `subtype` is `error` |
| `result` | string | The agent's whole answer: all text from every prompt of the run, plus any plan from plan mode |
| `error` | string or null | Why the run failed, `cancelled`, or `null` |
| `session_id` | string or null | The session's id, to go on with `--resume` |
| `total_cost_usd` | number | The cost of the run, `0` when the provider reports none |
| `duration_ms` | number | How long the model work took, in milliseconds |

In `json` format, notes (denied permissions, plans not carried out, verification verdicts) are not printed.

### `stream-json`

```bash
{{COMMAND}} -p --output-format stream-json "fix the typo in README.md"
```

Each line is one JSON object. Events come as `{"type":"event","event":...}`, notes as `{"type":"note","text":...}`, and the last line is the same result object as in `json`:

```json
{"type":"event","event":{"SessionStarted":"0f9c2d3e-..."}}
{"type":"event","event":{"TextDelta":"Hel"}}
{"type":"event","event":{"TextDelta":"lo"}}
{"type":"event","event":{"Completed":{"duration_ms":7,"cost_usd":0.25}}}
{"type":"result","subtype":"success","is_error":false,"result":"Hello","error":null,"session_id":"0f9c2d3e-...","total_cost_usd":0.25,"duration_ms":7}
```

The `event` value names the event kind as its only key, with the event's data as its value; an event without data is the bare name, such as `"Cancelled"`. The kinds you will meet most often:

| Event | Data |
|---|---|
| `SessionStarted` | The session id |
| `TextDelta` | A piece of the answer's text |
| `ThinkingDelta` | A piece of the model's reasoning |
| `ToolStarted` | `id`, `name`, `summary`, `output`, `failed`, and `input` when known |
| `ToolInputDelta` | `name`, `piece`: the tool input as it streams |
| `ToolProgress` | `id`, `line` |
| `ToolLog` | `id`, `text` |
| `ToolResult` | `id`, `output`, `failed` |
| `ToolFinished` | `id` |
| `AgentSpawned` | `index`, `kind`, `summary`, `prompt`: a sub-agent started |
| `AgentEvent` | `index`, `event`: an event of that sub-agent, in the same shape |
| `AgentUpdate` | `index`, `status`, `summary` |
| `Usage` | The tokens a model call used |
| `ContextMeasured` | How full the context window is |
| `Compacted` | The conversation was compacted |
| `Asked` | The agent asked for permission or a question (denied in print mode) |
| `FellBack` | The model it went on with after a failed call |
| `AccountSwitched` | `from`, `to`: another account took over |
| `Picked` | `model`, `effort`: what automatic effort chose |
| `Verified` | A verification verdict |
| `Refused` | `tool`, `reason`: a tool call was refused |
| `Edited` | A file was changed |
| `Completed` | `duration_ms`, `cost_usd`: the run ended |
| `Failed` | Why the run failed |
| `Cancelled` | The run was cancelled |

Other event kinds can appear as {{NAME}} grows; skip the ones you do not need.

### Text format and errors

In `text` format the answer goes to standard output and everything else to standard error: tool lines, notes, and, when the run fails, `{{COMMAND}}: <error>`.

In `json` and `stream-json`, standard output always ends with a result object, even when the command could not start (for example `no model is available`): `is_error` is `true` and `error` says why. The error is also printed on standard error.

## Several prompts from standard input

`--input-format stream-json` reads the prompts from standard input instead of the command line, one JSON object per line, and answers them in turn in the same session:

```bash
printf '%s\n' \
  '{"type":"user","message":{"content":"read src/lib.rs"}}' \
  '{"type":"user","message":{"content":[{"type":"text","text":"now add a test for parse"}]}}' \
  | {{COMMAND}} -p --input-format stream-json --output-format stream-json
```

- Only lines with `"type":"user"` are prompts; other lines are skipped, and so are blank lines.
- `message.content` is either a string or a list of blocks; the `text` of each block is joined with newlines.
- Each line is answered as soon as it arrives, so another program can write a prompt, read the answer, and write the next one.
- A line that is not JSON, whose content is not text, or that cannot be read (for example invalid UTF-8) stops the command with `input line <n>: ...`; the lines before it were already answered.

## Limit the run

| Option | Effect |
|---|---|
| `--max-turns <n>` | The most model turns the run may take. Without it, the limit is 200. |
| `--verification "<criteria>"` | Before the run ends, the work is judged against these criteria |
| `--fallback-model <model>` | The model to go on with when the chosen one is overloaded |
| `--append-system-prompt "<text>"` | Text added at the end of the system prompt |
| `-m`, `--model <model>` | The model; the last one you picked otherwise |

When the agent runs out of turns, the run fails with `The agent stopped after too many steps without finishing.`

With `--verification`, each verdict is a note (`✓ verification passed (score 0.90)` or `✗ verification failed, attempt 1/3 ...`). If any verdict fails, the result's `subtype` is `error` and the command exits with 1. See [Verification and review](./verification-and-review).

```bash
{{COMMAND}} -p --mode edits \
  --verification "cargo test passes and no public API changed" \
  "fix the overflow in parse_size"
```

## Go on with a conversation

| Option | Effect |
|---|---|
| `-c`, `--continue` | Go on with the last conversation in this folder |
| `-r <id>`, `--resume <id>` | Go on with the conversation with that id. With `--print` the id is required: `--resume needs a session id with --print` |
| `--fork-session` | With `-c` or `-r`: go on in a copy, leaving the original as it is |

The `session_id` of a `json` result is the id to pass to `--resume`:

```bash
id=$({{COMMAND}} -p --output-format json "summarize the open issues in NOTES.md" | jq -r .session_id)
{{COMMAND}} -p --resume "$id" "now draft a reply to the first one"
```

In print mode, `-r` without an id stops the command with that error (exit 1): the picker it opens in the chat is not available here. `-c` with no earlier conversation in the folder starts a new one.

## Other options

`-C <path>` (`--cwd`) runs in another project folder, and `--add-dir <path>` lets the agent read another folder like the project. See the [CLI reference](./cli-reference) for every option.

## Exit codes

| Code | When |
|---|---|
| `0` | The run finished and every verification passed |
| `1` | The run failed or was cancelled, a verification failed, or the command could not start (for example `no model is available`, `--print needs a prompt`, an unknown `--input-format`) |
| `2` | The command line itself is wrong: an unknown option, a missing value, an option that needs `--print` used without it, an unknown `--output-format` or `--mode`. Also when `{{COMMAND}}` is started without `-p` and standard input or output is not a terminal: `no terminal: use -p for print mode` |

## Scripts and CI

### Fail a CI job when the agent fails

```bash
#!/usr/bin/env bash
set -euo pipefail
{{COMMAND}} -p --mode edits --max-turns 40 \
  --allowedTools "Bash(cargo test:*)" \
  --verification "cargo test passes" \
  "fix the failing tests"
```

### Read the answer and the cost with jq

```bash
out=$({{COMMAND}} -p --output-format json "is there dead code in src/?")
echo "$out" | jq -r .result
echo "cost: $(echo "$out" | jq .total_cost_usd) USD"
```

### Follow tool calls live

```bash
{{COMMAND}} -p --output-format stream-json "update the changelog" \
  | jq -c 'select(.type == "event" and .event.ToolStarted) | .event.ToolStarted | {name, summary}'
```

### Review staged changes in a Git hook

```bash
#!/bin/sh
{{COMMAND}} -p --mode ask "Review this diff for bugs. Answer OK if there are none.

$(git diff --cached)" | grep -q '^OK' || exit 1
```

> [!NOTE]
> In `text` input format the prompt is taken from the command line only; standard input is not read. To pass a file's content, put it in the prompt (`"$(cat file)"`) or ask the agent to read the file.

> [!TIP]
> Before running in CI, connect a provider on that machine with `{{COMMAND}} login`, and check it with `{{COMMAND}} doctor`. See [Models and providers](./models-and-providers) and [Troubleshooting](./troubleshooting).
