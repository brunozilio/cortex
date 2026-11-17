---
title: Status line
description: Replace the line under the prompt with the output of your own command, fed the session's state as JSON.
---

The status line is the line under the prompt. By default it shows the conversation's title, the model, the permission mode and the effort. With `/statusline` you can draw it with your own command instead: {{NAME}} runs the command, passes it the session's state as JSON on stdin, and shows the first line it prints.

## Quick start

In the chat:

```text
/statusline jq -r '"\(.model.display_name) · \(.mode) · $\(.cost.total_cost_usd)"'
```

The line under the prompt now reads something like:

```text
Opus · edits · $0.42
```

Turn it off again with:

```text
/statusline off
```

## The `/statusline` command

| Command | What it does |
|---|---|
| `/statusline` | Shows the current command, or explains how to set one |
| `/statusline <command>` | Draws the status line with that command |
| `/statusline off` | Goes back to the usual status line |

The command is remembered with the chat's own preferences (like the model and vim keys, in {{NAME}}'s data folder; see [Files and directories](./files-and-directories)), so it stays across chats and folders. `/config` shows it under *Status line*.

## How the command runs

- It runs with `sh -c` in the project folder.
- It gets the state below as one JSON object on stdin.
- The first line of its stdout is shown, dimmed. Further lines are ignored, and so is stderr.
- It runs again at most every 3 seconds while the chat is open.
- Until it has printed something, the usual status line stays.
- If it cannot be started, the line says `status line: could not run <command>`.
- If it runs for more than 5 seconds it is stopped and the line says `status line: the command took too long (over 5s) and was stopped`.
- At most 4 KB of its output is read.

## Input JSON

```json
{
  "model": { "id": "provider-model-id", "display_name": "Opus" },
  "workspace": { "current_dir": "/home/ana/code/app" },
  "session_id": "4f1c…",
  "mode": "edits",
  "output_style": "default",
  "cost": { "total_cost_usd": 0.42 },
  "context": { "used": 48210, "window": 200000 },
  "busy": false
}
```

| Field | Value |
|---|---|
| `model.id` | The model's id |
| `model.display_name` | The model's name as the chat shows it |
| `workspace.current_dir` | The project folder |
| `session_id` | The conversation's id, or `null` before it starts |
| `mode` | `ask`, `edits`, `plan` or `bypass` |
| `output_style` | The output style's name, `default` when none is set |
| `cost.total_cost_usd` | What the conversation has cost so far, in US dollars |
| `context.used` | Tokens of the context window in use |
| `context.window` | The model's context window, when known |
| `busy` | `true` while the agent is working |

## Examples

### A script with git information

`~/bin/{{COMMAND}}-status.sh`:

```bash
#!/bin/sh
input=$(cat)
model=$(printf '%s' "$input" | jq -r '.model.display_name')
dir=$(printf '%s' "$input" | jq -r '.workspace.current_dir')
branch=$(git -C "$dir" branch --show-current 2>/dev/null)
used=$(printf '%s' "$input" | jq -r '.context.used // 0')
window=$(printf '%s' "$input" | jq -r '.context.window // 0')
percent=0
if [ "$window" -gt 0 ]; then percent=$((used * 100 / window)); fi
echo "$model · ${branch:-no git} · context ${percent}%"
```

```text
/statusline ~/bin/{{COMMAND}}-status.sh
```

### Show when the agent is working

```text
/statusline jq -r 'if .busy then "working… \(.mode)" else "ready · \(.mode)" end'
```

> [!TIP]
> Keep the command fast: it runs every few seconds. Cache slow information, such as a network call, in a file and refresh it in the background.

## See also

- [Chat](./chat) for the rest of the screen.
- [Slash commands](./slash-commands).
- [Budget and cost](./budget-and-cost) for what `cost` counts.
