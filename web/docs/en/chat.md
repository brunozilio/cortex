---
title: Chat
description: Use the terminal chat: write prompts, mention files, attach images, run shell commands, steer the agent and read what it does.
---

The chat is where you work with {{NAME}}. You type a prompt, the agent works, and you follow along, answer its questions and steer it as it goes. This page covers everything the prompt and the screen do. For the keys alone, see [Keyboard shortcuts](./keyboard-shortcuts); for commands, see [Slash commands](./slash-commands).

## Start a chat

```bash
{{COMMAND}}                          # chat in the current folder
{{COMMAND}} "fix the failing test"   # open the chat and send this prompt
{{COMMAND}} -C ../other-project      # chat in another folder
{{COMMAND}} -c                       # continue the last conversation in this folder
{{COMMAND}} --inline                 # chat at the bottom of the terminal
```

Other start-up flags (`-m`, `--mode`, `--add-dir`, `-r`, `--fork-session`, `--channels`…) are in the [CLI reference](./cli-reference).

## The screen

| Area | What it shows |
|---|---|
| Conversation | Your prompts (marked `>`), the agent's answers rendered as Markdown with highlighted code, reasoning (`✻ Thinking…`), tool calls (`●` with a `⎿` result), sub-agents, plans, diffs and notices. |
| Panel | What needs your attention: a question or permission request, a picker, `@` or `/` completions, the agent's task list, or on a fresh chat a few hints. |
| Prompt | A rounded box where you type. It grows up to 10 lines. |
| Status line | One line at the bottom (see [Status line](#status-line)). |

Long content is folded: a tool's output to its first 3 lines (`… +N lines (ctrl+o to expand)`), reasoning to its last 3 lines while the agent thinks and to a `✻ Thought` line once done, diffs and edits to 20 lines, a sub-agent to its last 3 steps. `Ctrl+O` unfolds or folds all of it at once.

Colours follow the terminal's background (from `COLORFGBG`, or `{{ENV}}_THEME=light|dark`) and use true colour where `COLORTERM` says so; `{{ENV}}_REDUCED_MOTION=1` turns the animations off. See [Environment variables](./environment-variables).

## Write a prompt

Type and press `Enter` to send. The empty prompt shows a hint of what you can do next: `Try "explain this project"` on a new chat, `Reply to go on · / for commands` after a reply, `Say what to do next, or /retry` after a failure.

### Multi-line prompts

| Key | Effect |
|---|---|
| `Alt+Enter` | New line (works everywhere). |
| `Ctrl+J` | New line (works everywhere). |
| `Shift+Enter` | New line, in terminals that report it. |
| `Ctrl+G` | Edit the prompt in your editor (`$VISUAL`, else `$EDITOR`, else `vi`). |

{{NAME}} asks the terminal to report modified keys when it can. If `Shift+Enter` does nothing, run `/terminal-setup`: use a terminal that reports it (Ghostty, kitty, WezTerm, iTerm2 with "Report modifiers using CSI u"), or map `Shift+Enter` to send `\n` in your terminal's key settings.

With `Ctrl+G` the prompt opens in a temporary Markdown file; when the editor exits successfully its text (trailing whitespace trimmed) replaces the prompt. If the editor fails, the prompt stays as it was.

### Long pastes

A paste of 5 or more lines, or 1,000 or more characters, is folded into a placeholder such as `[Pasted text #1 +6 lines]`. You can type around it; the whole text is sent in its place.

### Prompt history

`↑` on the first line of the prompt recalls earlier prompts, `↓` on the last line goes forward again and back to an empty prompt. History is kept per folder (the last 100 prompts) and survives restarts. `Ctrl+R` searches it: type part of a prompt, `Ctrl+R` again for older matches, `Enter` (or `Tab`, `→`) takes the match, `Esc` leaves.

### Vim keys

`/vim` turns vim-style editing on or off (the choice is remembered). `Esc` switches to normal mode and `i` back to insert. The supported keys are listed in [Keyboard shortcuts](./keyboard-shortcuts#vim-mode).

## Mention files with @

Type `@` followed by part of a file name: the panel offers up to 8 matching project files, ranked by how well the file's own name matches, then the path, then letters in order. `↑`/`↓` choose, `Tab` completes it to `@path/to/file `.

```text
why does @src/main.rs exit early?
```

When you send the prompt, the text of each mentioned file goes with it, wrapped in a `<file path="...">` block:

- Up to 5 different files per prompt.
- Up to 50,000 characters of each; longer files are cut and marked `[… cut]`.
- A mention that is not a readable file is left as it is.
- Trailing punctuation (`,` `.` `:` `;` `)` `?` `!`) is not part of the path.

The file list comes from git (tracked plus untracked files that are not ignored), so completions need a git repository. A mention you type by hand works in any folder.

## Attach images

| How | Result |
|---|---|
| `Ctrl+V` | Attaches the image on the clipboard (macOS via `osascript`, Linux via `wl-paste` or `xclip`). |
| Drop an image file on the terminal | The dropped path is recognised, quoted or escaped, `~/` or `file://` included. |
| Paste an image file's path | Same as dropping it. |

Each image is inserted as a placeholder like `[Image #1]`. Only the images whose placeholders are still in the prompt when you send it go with it, so deleting a placeholder drops its image. PNG, JPEG, GIF and WebP are supported, up to 20 MiB each. If the clipboard holds no image, the chat says "No image on the clipboard."

Dropping a file that is not an image inserts its path as text.

## Run shell commands with !

Start a prompt with `!` to run a command yourself, without the agent:

```text
!cargo test -p domain
```

Typing `!` on an empty prompt switches to **shell mode**: the prompt's border and mark turn pink (`! `) and the hint reads "Run a shell command · its output goes with your next message · esc leaves". `Backspace` or `Esc` on the empty prompt leaves shell mode. Pasting text that starts with `!` into an empty prompt also enters it.

The command runs in the project folder and its output shows in the conversation as a `!` tool entry, marked failed when it exits with an error. The command and its output are not sent on their own: they go, once, at the start of your next prompt, introduced as "I ran this in the shell myself", so the agent sees what you saw.

Commands the agent left running in the background are listed with `/bashes`; `/bashes kill <id>` stops one. `Ctrl+B` sends the agent's currently running command to the background.

## Remember a note with #

Start a prompt with `#` to save a note to the project's memory instead of sending it:

```text
# tests run with cargo nextest
```

The chat answers `Remembered: tests run with cargo nextest`. See [Memory](./memory).

## Slash commands and skills

A prompt that starts with `/` and a command-shaped word is a command. Typing `/` shows matching commands, their aliases and your skills in the panel; `↑`/`↓` choose and `Tab` completes. A path such as `/Users/me/notes.txt what is this?` is sent as a normal prompt. A typo gets a suggestion: `/modle is not a command or skill. Did you mean /model?`

Any `/name` that is not built in runs the skill or custom command of that name, with the rest of the line as its arguments. See [Slash commands](./slash-commands) and [Skills](./skills).

## While the agent works

A spinner line shows what the agent is doing, the time and the tokens written: `Working… (12s · ↓ 1.2k tokens · esc to interrupt)`.

### Interrupt

`Esc` or `Ctrl+C` stops the current run. Anything you queued goes back into the prompt so you can edit it.

### Steer

You do not have to wait. Type a message and press `Enter` while the agent works: it is shown as `› your message  (queued)` and handed to the agent inside the same run, at its next step. The prompt hint says "Type to message the agent as it works".

- `↑` on an empty prompt takes back messages not yet handed over, so you can edit them (the hint reads "↑ edits what is queued · esc interrupts").
- If the run ends before the agent took a message, it is sent as the next prompt.
- If you interrupt the run, queued messages return to the prompt.

### Ask a side question

`/btw <question>` asks something without interrupting and without adding to the conversation. The answer appears in the panel; `Esc`, `Enter` or `Space` closes it. While the agent works, the side question sees the conversation up to the agent's last finished step.

### Answer questions and permission requests

When the agent needs you, the panel turns yellow and shows how long it has been waiting (`the agent waits for your answer · 8s`); the timer of the run pauses meanwhile.

| Request | Keys |
|---|---|
| Permission (`Allow Bash: cargo test?`) | `y` yes · `a` yes and don't ask again (when offered) · `n` no · `↑`/`↓` + `Enter` · `Esc` denies |
| "No, and tell the agent what to do instead" | Type the note, `Enter` sends it, `Esc` just says no |
| Questions from the agent | A number picks, `↑`/`↓` + `Enter`, `Space` toggles in a multiple choice, `Esc` skips |
| Plan approval | **Yes, and auto-accept edits**, **Yes, and manually approve edits**, **No, keep planning**; a number or `↑`/`↓` + `Enter`; `Esc` keeps planning |

`PgUp`/`PgDn` scroll the conversation while a request waits, for example to read a long plan. If the terminal is not focused when the agent asks or finishes, the chat rings the terminal bell.

### Tasks

For work with several steps the agent keeps a task list, shown in the panel as `Tasks  2 of 5 done · ctrl+t hides` with `✓` done, `●` in progress and `○` pending. `Ctrl+T` shows or hides it; `/todos` (or `/tasks`) prints it into the conversation.

## After a run

- **Title**: after the first run, the conversation gets a short title, shown in the status line, the terminal's title and `/resume`.
- **Suggested next prompt**: a suggestion may appear greyed in the empty prompt, followed by `(tab)`; `Tab` takes it.
- **Heads-up**: the agent may add a `Heads-up:` notice about something you should know.
- **Failure**: the status line shows `✗ last run failed` and the chat says `/retry sends it again from where it started.` When the provider was unavailable, the chat retries once by itself after 30 seconds (`↻ Retrying in 30s · the provider was unavailable · esc cancels`); `Esc` on an empty prompt cancels it.

## Review changes

| Command | What it shows |
|---|---|
| `/diff` | The project's uncommitted changes, inline in the conversation, plus untracked files (`● New, not yet in git: …`). |
| `/diffs` | A full-screen diff browser. |

The diff browser starts on uncommitted changes; `Tab` cycles the scope: uncommitted → unstaged → staged → branch (everything since the branch left `origin/HEAD`, `main`, `master` or its upstream). Keys:

| Key | Action |
|---|---|
| `↑` / `↓` | Previous / next file |
| `k` / `j` | Scroll a line up / down |
| `PgUp` / `PgDn` or `Space` | Scroll a page |
| `n` / `p` | Next / previous hunk |
| `Tab` | Next scope |
| `r` | Refresh |
| `Esc` or `q` | Close |

Both need a git repository.

## Parallel sessions

`←` on an empty prompt (outside shell mode) opens the **Sessions** list: the current conversation (`· here`) and any you left running, each marked `working`, `needs you`, `done` or `idle`, then this folder's earlier conversations with when they were last used. `/fleet` (or `/sessions`) opens it too. `↑`/`↓` move; `Enter` switches to the chosen session, or continues the chosen earlier conversation, and the one you leave keeps working; `n` starts a new session and leaves the current one working on its own; `Esc` goes back.

## Status line

The bottom line shows, separated by `·`:

1. The conversation's title, once it has one.
2. The model.
3. The permission mode (`ask`, `edits` in yellow, `plan` in blue, `bypass` in red).
4. The effort (`medium effort`, or `auto effort`).
5. `✗ last run failed`, after a failure.
6. Feature markers in green: a budget (`$0.42/$5.00`), verification (`verify (yours)`), the prompt cache countdown (`cache 1:30`, then `cache cold: …`).
7. Context left (`72% context left`).
8. Commands in the background (`2 background`).
9. The session's cost (`$0.42`).

On the right, when there is room: `shift+tab mode · /help`. The line also shows `Press Ctrl+C again to quit` and `Press Esc again to clear the prompt` (or, on an empty prompt, `Press Esc again to go back to an earlier prompt`) when those are armed.

`/statusline <command>` replaces it with your own command's output; see [Status line](./statusline).

## Mouse and scrolling

In full-screen mode the chat captures the mouse: the wheel scrolls 3 lines, `PgUp`/`PgDn` scroll 10. Dragging selects text, and releasing copies it to the clipboard (with `pbcopy`, `wl-copy` or `xclip`).

## Inline mode

```bash
{{COMMAND}} --inline
```

With `--inline` the chat takes only the bottom 16 lines of the terminal instead of the whole screen. Each finished exchange is printed above it into the terminal's own scrollback, once the agent is idle, so you scroll and select with the terminal as usual (the mouse is not captured).

## Quit

`Ctrl+C` on an empty prompt arms quitting (`Press Ctrl+C again to quit`); press it again within 2 seconds. `Ctrl+D` on an empty prompt, `/exit` or `/quit` quit at once. After a conversation, the terminal prints how to continue it: `{{COMMAND}} -c`.
