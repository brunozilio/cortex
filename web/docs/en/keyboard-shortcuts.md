---
title: Keyboard shortcuts
description: Every key the chat understands, in the prompt, in pickers, in questions, in the diff browser and in vim mode.
---

This page lists every key binding of the chat. Keys depend on what has focus: the prompt, a picker, a question from the agent, the diff browser or the sessions list. `/help` shows a summary line of the most used ones inside the chat.

> [!NOTE]
> On macOS, `Cmd` combinations reach the chat only if your terminal forwards them. `Shift+Enter` works only in terminals that report it; `/terminal-setup` explains how to get it.

## Most used

| Key | Action |
|---|---|
| `Enter` | Send the prompt |
| `Alt+Enter` / `Ctrl+J` | New line |
| `Esc` | Interrupt the agent |
| `Esc` `Esc` | Rewind (on an empty prompt) |
| `Shift+Tab` | Next permission mode |
| `Ctrl+R` | Search earlier prompts |
| `Ctrl+G` | Edit the prompt in `$EDITOR` |
| `Ctrl+V` | Attach the clipboard's image |
| `Ctrl+O` | Expand or fold output |
| `Ctrl+L` | Redraw the screen |
| `PgUp` / `PgDn` | Scroll |
| `Ctrl+C` | Interrupt, clear, or quit |

## Anywhere

These work whatever has focus.

| Key | Action |
|---|---|
| `Ctrl+C` | If the agent is working, interrupt it. Else, if the prompt has text, clear it. Else arm quitting; a second `Ctrl+C` within 2 seconds quits. |
| `Ctrl+O` | Expand or fold reasoning, tool output, sub-agent steps and diffs. |
| `Ctrl+T` | Show or hide the agent's task list. |
| `Ctrl+B` | Send the command the agent is running to the background. |

## Prompt: sending and lines

| Key | Action |
|---|---|
| `Enter` | Send the prompt (or run the command). While the agent works, the message is queued and handed to it in the same run. |
| `Alt+Enter` | Insert a new line. |
| `Shift+Enter` | Insert a new line, in terminals that report it. |
| `Ctrl+J` | Insert a new line. |
| `Tab` | Take the suggested next prompt shown in an empty prompt; otherwise complete the highlighted `@` file or `/` command. |
| `Shift+Tab` | Cycle the permission mode: ask → edits → plan → bypass → ask. |
| `Ctrl+G` | Open the prompt in `$VISUAL`, `$EDITOR` or `vi`; the edited text replaces it. |
| `Ctrl+V` | Attach the image on the clipboard as `[Image #n]`. |
| `Ctrl+R` | Search the prompt history. |
| `Ctrl+L` | Clear and redraw the screen. |
| `Ctrl+D` | Quit, when the prompt is empty. |

## Prompt: moving and deleting

| Key | Action |
|---|---|
| `←` / `→` | Move one character. |
| `Alt+←` / `Alt+→`, `Ctrl+←` / `Ctrl+→` | Move one word. |
| `Alt+B` / `Alt+F` | Move one word back / forward. |
| `Home` / `Ctrl+A`, `Cmd+←` | Start of the line. |
| `End` / `Ctrl+E`, `Cmd+→` | End of the line. |
| `↑` / `↓` | Move between lines of a multi-line prompt. |
| `Backspace` | Delete the character before the cursor. |
| `Delete` | Delete the character under the cursor. |
| `Alt+Backspace`, `Ctrl+W` | Delete the word before the cursor. |
| `Alt+D`, `Alt+Delete`, `Ctrl+Delete` | Delete the word after the cursor. |
| `Cmd+Backspace` | Delete to the start of the line. |
| `Ctrl+U` | Clear the whole prompt. |
| `Esc` `Esc` | Clear the prompt (when it has text): the first `Esc` arms it (`Press Esc again to clear the prompt`), the second clears. On an empty prompt the line says `Press Esc again to go back to an earlier prompt` instead, and a second `Esc` within a second opens the rewind list. |

## Prompt: history, completion and special starts

| Key | Action |
|---|---|
| `↑` on the first line | Recall the previous prompt from this folder's history. |
| `↓` on the last line | Go to the next prompt in history, then back to an empty prompt. |
| `↑` / `↓` while completions show | Choose among the `@` files or `/` commands offered; the list wraps around. |
| `↑` on an empty prompt while the agent works | Take back queued messages not yet handed to the agent, to edit them. |
| `!` on an empty prompt | Enter shell mode. |
| `Backspace` or `Esc` on an empty prompt in shell mode | Leave shell mode. |
| `←` on an empty prompt | Open the sessions list. |
| `Esc` while the agent works | Interrupt it. |
| `Esc` on an empty prompt while an automatic retry counts down | Cancel the retry. |
| `Esc` `Esc` on an empty prompt (within 1 second) | Open the rewind picker. |
| `PgUp` / `PgDn` | Scroll the conversation 10 lines. |

## History search (Ctrl+R)

The prompt shows `search '<query>': <match>`.

| Key | Action |
|---|---|
| Typing | Narrow the search (case-insensitive); the newest match shows. |
| `Backspace` | Delete a character of the search. |
| `Ctrl+R` | Next older match. |
| `Enter`, `Tab`, `→` | Put the match in the prompt. |
| `Esc`, `Ctrl+G` | Leave the search. |

## Pickers

Pickers open for `/model`, `/mode`, `/effort`, `/output-style`, `/config`, `/resume`, `/rewind`, `/login`, `/logout`, `/accounts` and `/model <role>`.

| Key | Action |
|---|---|
| `↑` / `↓` | Move (wraps around). |
| `Enter` | Pick. |
| `Esc` | Close without picking. |

## API key field

Shown after picking a provider that takes an API key in `/login`.

| Key | Action |
|---|---|
| Typing or pasting | Enter the key (shown as dots). |
| `Backspace` | Delete a character. |
| `Enter` | Connect. |
| `Esc` | Cancel. |

## Questions and permission requests

| Key | Permission request | Question | Plan approval |
|---|---|---|---|
| `y` | Yes | | |
| `a` | Yes, and don't ask again (when offered) | | |
| `n` | No | | |
| `1`–`9` | Pick that choice | Pick that choice (toggle it in a multiple choice) | Pick that choice |
| `↑` / `↓` | Move | Move | Move |
| `Space` | | Toggle the choice in a multiple choice | |
| `Enter` | Confirm | Confirm | Confirm |
| `Esc` | Deny | Skip | Keep planning |
| `PgUp` / `PgDn` | Scroll the conversation | Scroll the conversation | Scroll the conversation |

After **No, and tell the agent what to do instead**, type your note: `Enter` sends it (an empty note just says no), `Backspace` deletes, `Esc` says no without a note.

## Side question (/btw)

| Key | Action |
|---|---|
| `Esc`, `Enter`, `Space` | Close the answer. |

## Diff browser (/diffs)

| Key | Action |
|---|---|
| `↑` / `↓` | Previous / next file. |
| `k` / `j` | Scroll one line up / down. |
| `PgUp` | Scroll a page up. |
| `PgDn`, `Space` | Scroll a page down. |
| `n` / `p` | Next / previous hunk. |
| `Tab` | Next scope: uncommitted → unstaged → staged → branch. |
| `r` | Reload. |
| `Esc`, `q` | Close. |

## Sessions list

Opened with `←` on an empty prompt, or with `/fleet` (`/sessions`).

| Key | Action |
|---|---|
| `↑` / `↓` | Move (wraps around). |
| `Enter` | Switch to the chosen session, or continue the chosen earlier conversation; the one you leave keeps working. |
| `n` | Start a new session; the current one keeps working. |
| `Esc` | Back to the chat. |

## Vim mode

`/vim` turns vim keys on or off; the choice is remembered. In vim mode the prompt starts in insert mode, where keys behave as usual. `Esc` switches to normal mode (so with vim keys on, interrupting the agent with `Esc` takes a second `Esc`; `Ctrl+C` always interrupts).

Normal mode keys:

| Key | Action |
|---|---|
| `h` / `l`, `←` / `→` | Left / right. `Backspace` also moves left. |
| `w` | To the start of the next word. |
| `b` | To the start of the previous word. |
| `0`, `^` | Start of the line. |
| `$` | End of the line. |
| `j` / `k` | Line down / up. |
| `x` | Delete the character under the cursor. |
| `D` | Delete to the end of the line. |
| `dd` | Clear the prompt. |
| `dw` | Delete the next word. |
| `i` | Insert before the cursor. |
| `a` | Insert after the cursor. |
| `I` | Insert at the start of the line. |
| `A` | Insert at the end of the line. |
| `o` | Open a new line below and insert. |

Other letters do nothing in normal mode. `Enter`, `↑`/`↓` and the `Ctrl` keys work as in insert mode.

## Mouse

In full-screen mode (not `--inline`):

| Action | Effect |
|---|---|
| Wheel | Scroll 3 lines. |
| Drag | Select text; releasing copies it to the clipboard. |
