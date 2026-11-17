---
title: Sessions
description: Continue, resume, fork, branch, rewind, retry, compact, export and copy conversations, and where they are stored.
---

Every conversation with {{NAME}} is a session: it is saved as you go, gets a title, and can be continued later, copied, cut back to an earlier prompt or summarized. This page shows how.

## Continue where you left off

When you quit after a conversation, the terminal prints:

```text
Continue this conversation with: {{COMMAND}} -c
```

| Command | What it does |
|---|---|
| `{{COMMAND}} -c` / `--continue` | Continue the last conversation in this folder. With none, it says so and starts a new one. |
| `{{COMMAND}} -r` / `--resume` | Open the chat on the picker of this folder's earlier conversations. |
| `{{COMMAND}} -r <id>` | Continue the conversation with that session id. |
| `{{COMMAND}} -c --fork-session` | Continue in a copy, leaving the original as it is (also with `-r`). |

`-c` and `-r` also work with `-p` for one-shot answers; see [Headless](./headless).

```bash
{{COMMAND}} -c "now add the docs"
{{COMMAND}} -r 2b0c…e41 --fork-session
```

When a conversation is continued, the chat says `Continuing the last conversation in this folder.` (or `Continuing an earlier conversation.`) and then shows a short recap: `Where you left off: …`.

> [!TIP]
> `/status` shows the current session id.

## Resume a conversation

Inside the chat, `/resume` (alias `/continue`) opens **Resume a conversation**: this folder's earlier conversations, newest first, each with its title (or `Untitled`) and when it was last used (`just now`, `5m ago`, `2h ago`, `3d ago`).

```text
/resume
/resume login bug
```

With words, only conversations whose title contains every one of them (in any case) are listed. The list includes conversations run in this folder by other clients, such as the browser, but leaves out the current one and archived ones. `↑`/`↓` and `Enter` pick; `Esc` closes. `/resume` is not available while the agent works.

## Titles

After the first run, a background task gives the conversation a short title. The title shows at the start of the status line, in the terminal's window title (`<title> · {{NAME}}`), in `/resume`, in `/export`'s file name and in the [trace console](./trace-console).

## Start fresh

`/clear` (also `/new`, `/reset`) starts a new conversation with the same model. The old one is kept and listed in `/resume`.

## Fork a conversation

`/fork` makes the next message go on in a copy of this conversation; the original stays as it is and can be resumed later. It needs at least one message first. `/branch` with no argument does the same. On the command line, `--fork-session` with `-c` or `-r` forks the conversation you continue.

## Branch from an earlier prompt

`/branch <n>` starts a new conversation from before prompt number `n` of this one, keeping the original:

```text
/branch 2
```

The chat cuts the conversation back to before that prompt, puts the prompt in the input for you to edit, and says the original stays in `/resume`. Files on disk are not changed. Prompts are numbered oldest first. Branching needs the `branches` feature, and is not available while the agent works. See [Branches and git](./branches-and-git).

## Rewind

Each prompt you send is a checkpoint. Rewinding goes back to before one of them:

- `Esc` `Esc` on an empty prompt (the second press within a second), or
- `/rewind` (alias `/undo`).

A **Rewind to before…** picker lists this conversation's prompts, newest first. Picking one:

1. Restores the files the agent changed since that prompt.
2. Cuts the conversation back to before it.
3. Puts that prompt back in the input, so you can change it and send it again.

The chat reports what happened, for example `Rewound: 3 files restored. Changes made with Bash are not undone.`

> [!WARNING]
> Rewind restores what the agent's file tools changed. Changes made with Bash commands are not undone. The `destructive-snapshot` feature keeps untracked files before commands like `rm -r` or `git reset --hard` so rewind can bring them back; see [Sandbox and safety](./sandbox-and-safety).

If the conversation was compacted after that prompt, the files are restored but the conversation is kept as it is (`the conversation was compacted since, so it is kept`). Rewind is not available while the agent works.

## Retry

`/retry` sends the last prompt again from where it started: it rewinds to before that prompt (restoring files) and sends it once more. With no checkpoint it simply sends the last prompt again. After a failed run the chat suggests it (`/retry sends it again from where it started.`), and when the provider was unavailable it retries once on its own after 30 seconds.

## Compact the conversation

Long conversations fill the model's context window. Compacting replaces the history with a summary so you can keep going:

```text
/compact
/compact keep the decisions about the API and the open TODOs
```

The text after `/compact` tells the summary what to keep. The chat shows `Compacting the conversation…` while it works. Compaction also happens on its own when the context gets close to full. `/context` shows how much is used and how much is free before compacting; the status line shows `NN% context left`.

`/compact` is not available while the agent works.

## Export and copy

| Command | What it does |
|---|---|
| `/export` | Writes the conversation as Markdown in the project folder, named after the title (`fix-the-login-page.md`) or `conversation.md`. |
| `/export <path>` | Writes it to that path (relative to the project folder). |
| `/copy` | Copies the agent's last answer to the clipboard (`pbcopy`, `wl-copy` or `xclip`). |

The export starts with the title as a heading and contains your prompts as quotes, the answers, one line per tool call (with `(failed)` when it failed), sub-agents, plans and diffs. Notices, reasoning and errors are left out.

## Parallel sessions

`←` on an empty prompt opens the sessions list; `n` there starts a new session while the current one keeps working. See [Chat](./chat#parallel-sessions).

## Where sessions are stored

Sessions live in your data folder, not in the project:

| System | Data folder |
|---|---|
| macOS | `~/Library/Application Support/{{NAME}}` |
| Linux | `$XDG_CONFIG_HOME/{{COMMAND}}`, by default `~/.config/{{COMMAND}}` |
| Windows | `%LOCALAPPDATA%\{{NAME}}` for `native-sessions/`, `%APPDATA%\{{NAME}}` for `users/` |

Inside it:

| Path | Holds |
|---|---|
| `native-sessions/` | Each session's conversation and checkpoints. |
| `users/` | Per-user settings of the chat (last model, mode, effort, prompt history, the last session per folder, titles) and the session catalog that `/resume` and the trace console list. |

See [Files and directories](./files-and-directories). `/factory-reset` erases all of it.
