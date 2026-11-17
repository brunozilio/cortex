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

With words, only conversations whose title contains every one of them (in any case) are listed. The list includes conversations run in this folder by other clients, such as the browser, but leaves out the current one. `↑`/`↓` and `Enter` pick; `Esc` closes. `/resume` is not available while the agent works.

## Titles

After the first run, a background task gives the conversation a short title. The title shows at the start of the status line, in the terminal's window title (`<title> · {{NAME}}`), in `/resume`, in `/export`'s file name and in the [trace console](./trace-console).

## Start fresh

`/clear` (also `/new`, `/reset`) starts a new conversation with the same model. The old one is kept and listed in `/resume`.

## Fork a conversation

`/fork` makes the next message go on in a copy of this conversation; the original stays as it is and can be resumed later. It needs at least one message first. `/branch` with no argument lists the prompts you can branch from. On the command line, `--fork-session` with `-c` or `-r` forks the conversation you continue.

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

A file you changed after the agent left it is not overwritten: the rewind leaves it as it is and lists it with the reason (`Left as they are: notes.txt (it changed after the agent left it)`). A file larger than 10 MiB, or one that could not be read, has no earlier version kept: the tool that wrote it says so in its result (`/rewind cannot put back the earlier version of …`), and a rewind leaves it as it is and lists it with that reason. A file that cannot be put back (a folder turned read-only, a full disk) is listed with the error, and every other file is still restored. The conversation is saved before the list of checkpoints, so a rewind that fails to save never loses the checkpoints while the conversation stays as it was; run it again, and the files already back count as restored. Files are put back whole (a crash mid-way never leaves a half-written file), keep their permissions, and a link put in a file's place is replaced rather than followed. A file the agent edited through a symbolic link is saved and restored where the link leads, and the link stays a link. Each file's state is saved before the agent's first write to it, so a prompt interrupted by a crash keeps it; but what the agent left in each file was never written down, so a rewind after the crash cannot tell your later fixes from the agent's work: it lists those files as left as they are, and you can restore them by hand.

A conversation file whose last line was cut mid-write (even in the middle of a character) is repaired on the next save: the cut line is dropped and saving goes on.

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
| Windows | `%LOCALAPPDATA%\{{NAME}}` for `native-sessions/`, `%APPDATA%\{{NAME}}` for `app.db` and `users/` |

Inside it:

| Path | Holds |
|---|---|
| `native-sessions/` | Each session's conversation, trace and checkpoints. |
| `app.db` | A SQLite database with the list of conversations (folder, title, status, last use) that `/resume`, the sessions list and the trace console show, and your memories. |
| `users/` | Per-user settings of the chat: last model, mode, effort, prompt history, the last session per folder and its title. |

See [Files and directories](./files-and-directories). `/factory-reset` erases all of it.
