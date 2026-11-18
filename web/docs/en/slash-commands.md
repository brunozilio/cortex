---
title: Slash commands
description: Complete reference of every slash command in the chat, its arguments and aliases, and how skills and custom commands run as /name.
---

Slash commands control the chat itself: the conversation, the model, the project and the app. Type `/` in the prompt to see them with their aliases and your skills; `↑`/`↓` choose and `Tab` completes.

```text
/help              list every command and the main keys
/help compact      explain one command (or skill)
/model             pick a model
/compact keep the API decisions
```

## How commands are read

- A command is `/` followed by a command-shaped word (letters, digits, `-`, `_`, `:`, `?`), then optional arguments. Names are case-insensitive.
- A slash followed by a path, like `/Users/me/notes.txt what is this?`, is not a command: it is sent as a prompt.
- A command that takes no arguments refuses them: `/clear now` answers `/clear can't take that.` and leaves your text in the prompt.
- An unknown name gets the nearest match: `/modle is not a command or skill. Did you mean /model?`
- Built-in commands come first, then the feature commands below, then [skills and custom commands](#skills-and-custom-commands).

`/help` groups the commands as **Conversation**, **Model and mode**, **Project**, **App** and **Features**, and ends with the keys.

## Quick reference

| Command | Aliases | What it does |
|---|---|---|
| `/help [command]` | `/?` | Show the commands and keys, or one command |
| `/clear` | `/new`, `/reset` | Start a fresh conversation |
| `/resume [words]` | `/continue` | Go back to an earlier conversation of this folder |
| `/fleet` | `/sessions` | Show every session, running or earlier, and switch between them |
| `/fork` | | Go on in a copy of this conversation |
| `/compact [what to keep]` | | Summarize the conversation to free context |
| `/rewind` | `/undo` | Go back to before an earlier prompt: files and conversation |
| `/retry` | | Send the last prompt again, from where it started |
| `/btw <question>` | | Ask a side question without interrupting |
| `/export [path]` | | Write the conversation as Markdown |
| `/copy` | | Copy the agent's last answer |
| `/model [id \| role [model] \| roles \| fallbacks …]` | | Pick the model, per role, or the fallback chain |
| `/mode [mode]` | `/plan` (= `/mode plan`) | Pick ask, edits, plan or bypass |
| `/effort [level \| auto]` | | Set the reasoning effort |
| `/output-style [style]` | | Choose how the agent talks |
| `/diff` | | Show the project's uncommitted changes |
| `/diffs` | | Browse changes full screen |
| `/add-dir [path]` | | Let the agent read a folder like the project |
| `/permissions` | `/allowed-tools` | Show the allow, ask and deny rules |
| `/hooks` | | Show the hooks a run here fires |
| `/todos` | `/tasks` | Show the agent's task list |
| `/context` | | Show what fills the context window |
| `/skills [new <what>]` | `/commands` | List skills and commands, or write one |
| `/agents [new <what>]` | | List agents, or write one |
| `/mcp [approve\|deny\|login\|logout <name>]` | | List or manage MCP servers |
| `/reload` | | Reread plugins, skills, agents and MCP servers |
| `/cost` | `/usage` | Show this session's cost |
| `/status` | | Show the model, mode, folders, session and version |
| `/config` | `/settings` | Change the chat's settings |
| `/statusline [command \| off]` | | Draw the status line with your command |
| `/vim` | | Turn vim keys on or off |
| `/login` | | Connect a provider |
| `/logout` | | Forget a provider's credentials |
| `/accounts [remove \| balance [way]]` | | Switch, forget or balance accounts |
| `/doctor` | | Check that this installation can work here |
| `/terminal-setup` | | How to make `Shift+Enter` add a line |
| `/exit` | `/quit` | Quit |
| `/branch [n]` | | List the prompts to branch from, or start a new conversation from before prompt n |
| `/budget [dollars \| off]` | | Cap what this session spends |
| `/verification [criteria \| off]` | | Judge runs against criteria |
| `/memory [subcommand]` | | List, search and manage the project's memory |
| `/learn keep\|skip` | | Act on the lesson offered after a run |
| `/check` | | Check this branch's changes against memories |
| `/rules [keep <n>]` | | Permission rules your past answers suggest |
| `/loop [interval] <task>` | | Run a task now and again later |
| `/schedules [delete <id>]` | | List or delete scheduled prompts |
| `/bashes [kill <id>]` | | List or stop commands left in the background |
| `/gain` | | How much shorter command output reached the model |
| `/features [name on\|off]` | | List or switch the features you can turn off |
| `/trace` | `/console` | Open the live trace in the browser |
| `/remote [pair\|off]` | | Remote control from a phone: show it, invite a phone, or stop it |
| `/trust [yes\|no]` | | Show whether this folder is trusted, or change it |
| `/docs [topic]` | | Open this documentation in the browser, or search it |
| `/factory-reset` | | Erase everything {{NAME}} keeps on this machine |
| `/<skill> [arguments]` | | Run a skill or custom command |

## Conversation

### /help

`/help` lists every command by group, the feature commands, `/<skill>`, and the main keys. `/help <name>` (with or without the `/`) shows one command or skill and what it does. Alias: `/?`.

### /clear

Clears the conversation and starts a fresh one with the same model: the transcript, queued messages, checkpoints, title and suggestion are dropped. Folders added with `/add-dir` stay. The old conversation stays available in `/resume`. Aliases: `/new`, `/reset`. Takes no arguments.

### /resume

`/resume` opens a picker of earlier conversations in this folder, newest first, each with its title (or `Untitled`) and how long ago it was used (`2h ago`). `/resume <words>` keeps only conversations whose title contains every word, in any case. The current conversation is not offered. Picking one continues it and shows a `Where you left off:` recap. Not available while the agent works. Alias: `/continue`. See [Sessions](./sessions#resume-a-conversation).

### /fleet

Opens the sessions list: this conversation first, then the sessions you left running in the background, then the earlier conversations of this folder. `↑`/`↓` move, `Enter` switches to the chosen one (an earlier conversation is continued; the one you leave keeps working), `n` starts a new session, `Esc` closes. `←` on an empty prompt opens the same list. Alias: `/sessions`. Takes no arguments. See [Keyboard shortcuts](./keyboard-shortcuts#sessions-list).

### /fork

Makes the next message go on in a copy of this conversation, leaving this one as it is. Needs a conversation with at least one message ("Nothing to fork yet: send a message first."). Takes no arguments.

### /compact

`/compact` summarizes the conversation to free context. `/compact <what to keep>` tells the summary what to focus on. Not available while the agent works. See [Sessions](./sessions#compact-the-conversation).

### /rewind

Opens a **Rewind to before…** picker of this conversation's prompts, newest first. Picking one restores the files the agent changed since then, cuts the conversation back to before that prompt, and puts the prompt back in the input to edit. Changes made with Bash are not undone. Not available while the agent works. `Esc` `Esc` on an empty prompt opens the same picker. Alias: `/undo`. See [Sessions](./sessions#rewind).

### /retry

Sends the last prompt again from where it started: it rewinds to before that prompt, then sends it. If no checkpoint is known it just sends the last prompt again. Not available while the agent works ("Esc stops it, then /retry").

### /btw

`/btw <question>` asks a side question without interrupting the agent and without adding it to the conversation. The answer appears in the panel; `Esc`, `Enter` or `Space` closes it. Without a question it explains how to use it.

### /export

`/export` writes the conversation as Markdown to a file in the project folder named after the title (`fix-the-login-page.md`), or `conversation.md` without one. `/export <path>` writes to that path, relative to the project folder. The file has the title as a heading, your prompts as quotes, the answers, a line per tool call (marked `(failed)` when it failed), sub-agents, plans and diffs; notices, reasoning and errors are left out.

### /copy

Copies the agent's last answer to the clipboard with `pbcopy`, `wl-copy` or `xclip`, whichever is found first.

## Model and mode

### /model

| Form | Effect |
|---|---|
| `/model` | Open the model picker (label and id of every model the connected providers offer). |
| `/model <id or name>` | Switch to that model. |
| `/model roles` | List the roles and the model each uses. |
| `/model <role>` | Pick the model for a role, or **Session model** to follow the session's model. |
| `/model <role> <model>` | Set the model for a role. |
| `/model <role> default` | Clear the role (also `none` or `off`). |
| `/model fallbacks` | Show the fallback chain. |
| `/model fallbacks <model> [model…]` | Set the chain tried when a model is busy, signed out or unreachable. |
| `/model fallbacks default` | Clear the chain (also `none` or `off`). |

The roles are `plan` (plan mode and the Plan agent), `explore` (the Explore agent that searches the code), `small` (background chores: titles, effort picks, web reading), `review` (the judge that checks finished work) and `advisor` (the advisor the agent consults). The model, roles and fallbacks are remembered for the next chats. See [Models and providers](./models-and-providers).

### /mode

`/mode` opens a picker; `/mode <mode>` switches at once.

| Mode | Also accepted | Meaning |
|---|---|---|
| `ask` | `default` | Ask before edits and commands |
| `edits` | `acceptEdits`, `accept-edits` | Edit files freely, ask before commands |
| `plan` | | Read and plan only |
| `bypass` | `bypassPermissions`, `bypass-permissions`, `yolo` | Never ask (a safety check still grades risky actions) |

`/plan` is short for `/mode plan`. `Shift+Tab` cycles the modes. The mode is remembered for the next chats unless you gave `--mode` on the command line. See [Permissions](./permissions).

### /effort

`/effort` opens a picker; `/effort <level>` sets it.

| Level | Meaning |
|---|---|
| `low` | Quickest answers, least reasoning |
| `medium` | Balanced |
| `high` | More reasoning for harder work |
| `xhigh` | Extended reasoning |
| `max` | As much reasoning as the model allows |
| `auto` | Each request picks the model and effort it needs; `/effort <level>` goes back to one level |

The effort is remembered for the next chats.

### /output-style

`/output-style` opens a picker of the built-in styles; `/output-style <style>` sets one: `default`, `concise`, `proactive`, `explanatory` or `learning`. It applies from the next message and is remembered. See [Output styles](./output-styles).

## Project

### /diff

Shows the project's uncommitted changes in the conversation, with new untracked files listed as `● New, not yet in git: …`. Says `No uncommitted changes.` when there are none. Needs git.

### /diffs

Opens the full-screen diff browser on uncommitted changes; `Tab` cycles uncommitted, unstaged, staged and branch. See [Chat](./chat#review-changes) for its keys.

### /add-dir

`/add-dir <path>` lets the agent read a folder like the project, for the rest of the chat (it survives `/clear`). The path may be relative to the project or start with `~/`; it must be an existing folder. `/add-dir` alone lists the folders added. The command-line equivalent is `--add-dir`.

### /permissions

Shows the permission rules in effect here (`deny`, `ask`, `allow`; deny wins over ask, ask over allow) and the added folders. Alias: `/allowed-tools`. See [Permissions](./permissions).

### /hooks

Shows the hooks a run here fires: event, matcher (`*` for all) and what runs. See [Hooks](./hooks).

### /todos

Prints the agent's task list (`☒` done, `◼` in progress, `☐` pending). `Ctrl+T` shows or hides the same list in the panel. Alias: `/tasks`.

### /context

Shows a bar of how full the context window is, the tokens used of the window, how many are free before compacting, and the share of each part: system prompt, tools, MCP tools, agents, memory, skills and messages.

### /skills

`/skills` lists the skills and custom commands found here, as `/name  description`. `/skills new <what it does>` asks the agent to write a new skill under `{{DIR}}/skills/<name>/SKILL.md`. Alias: `/commands`. See [Skills](./skills).

### /agents

`/agents` lists the sub-agents available here. `/agents new <what it does>` asks the agent to write a new one to `{{DIR}}/agents/<name>.md`. See [Agents](./agents).

### /mcp

| Form | Effect |
|---|---|
| `/mcp` | List the MCP servers a session here can use and their state. |
| `/mcp approve <name>` | Approve a server waiting for approval. |
| `/mcp deny <name>` | Refuse it. |
| `/mcp login <name>` | Sign in to a server in the browser. |
| `/mcp logout <name>` | Sign out. |

Changes apply from the next message. See [MCP](./mcp).

### /reload

Rereads `~/{{DIR}}` and the project and reports what was found: plugins, how many skills and agents, and MCP servers ready, waiting for approval or off.

## App

### /cost

Shows what this session cost and, per model, the input and output tokens, cache reads (with the hit rate) and writes, web searches and the price. Models without a known price (a subscription bills by its plan) show `not priced`. Alias: `/usage`. See [Budget and cost](./budget-and-cost).

### /status

Shows the version, the model (label and id), mode, effort, folder, session id, added folders and the cost so far.

### /config

Opens the **Settings** picker: Model, Permission mode, Effort, Output style, Vim keys, Status line, Budget, Features and Verification, each with its current value. Picking one opens the matching command. Alias: `/settings`. For the settings files themselves, see [Settings](./settings).

### /statusline

`/statusline <command>` draws the status line with the first line your command prints; the command gets the session's state as JSON on stdin and runs every few seconds. `/statusline off` restores the usual line. `/statusline` alone says which is in use. See [Status line](./statusline).

### /vim

Turns vim keys in the prompt on or off; remembered. See [Keyboard shortcuts](./keyboard-shortcuts#vim-mode).

### /login

Opens the **Connect a provider** picker. API-key providers ask for the key in a masked field; subscriptions sign in with the browser or a code. See [Accounts](./accounts).

### /logout

Opens a picker of the connected providers and forgets the credentials of the one you pick.

### /accounts

| Form | Effect |
|---|---|
| `/accounts` | Pick which account of a provider is used (the current one is marked `in use`); applies from the next message. |
| `/accounts remove` | Pick an account to forget. |
| `/accounts balance` | List the ways accounts can share the work. |
| `/accounts balance <way>` | Set it for the current model's provider: `off` (only the account in use), `failover` (the others when it is busy or signed out) or `round-robin` (each run starts on the next account). |

See [Accounts](./accounts).

### /doctor

Checks that this installation can work in this folder, like `{{COMMAND}} doctor`.

### /terminal-setup

Explains how to get `Shift+Enter` to add a line: `Alt+Enter` or `Ctrl+J` work everywhere; for `Shift+Enter`, use a terminal that reports it (Ghostty, kitty, WezTerm, iTerm2 with "Report modifiers using CSI u") or map it to send `\n`.

### /exit

Quits the chat. Alias: `/quit`.

## Features

These commands come from the chat's feature modules. Some depend on a feature being on; see [Experimental features](./experimental-features).

### /branch

`/branch <n>` starts a new conversation from before prompt number `n` of this one (numbered oldest first), keeping this one: the conversation is cut there and the prompt put back in the input, and the original stays in `/resume`. Not available while the agent works. Needs the `branches` feature (`/features branches on`).

> [!NOTE]
> `/branch` with no argument lists the numbered prompts you can branch from. See [Branches and git](./branches-and-git).

### /budget

| Form | Effect |
|---|---|
| `/budget` | Show what the session spent and its cap. |
| `/budget <dollars>` | Cap it, for example `/budget 5` or `/budget $5`: model calls stop once the session has spent it. |
| `/budget off` | Remove the cap (also `none`). |

The status line shows `$spent/$cap` while a budget is set. See [Budget and cost](./budget-and-cost).

### /verification

| Form | Effect |
|---|---|
| `/verification` | Show the criteria in effect and where they come from (yours, the plan's or the project's). |
| `/verification <criteria>` | Judge each run against these before it finishes; a judge agent runs the checks. |
| `/verification off` | Clear your criteria (a plan's still apply). |

The status line shows `verify (yours)`, `verify (plan)` or `verify (project)`. See [Verification and review](./verification-and-review).

### /memory

| Form | Effect |
|---|---|
| `/memory` | List the project's memories with their ids. |
| `/memory <words>` | Search them, with a score per match. |
| `/memory optimize` | Have the model tidy the memory (also `optimise`); needs a model. |
| `/memory undo` | Put the memory back as it was before the last change. |
| `/memory forgotten` | List forgotten memories (restorable for 30 days). |
| `/memory restore <id>` | Restore a forgotten memory. |
| `/memory share <id>` | Write a memory to a file to commit, so the team's agents recall it too. |
| `/memory edit` | Open the project's {{INSTRUCTIONS}} (or `{{DIR}}/{{INSTRUCTIONS}}`) in your editor. |
| `/memory seed` | Offer things you asked for in two or more past sessions. Needs `experimental-memory-seed`. |
| `/memory seed keep <n> [<n>…]` | Remember the offered items with those numbers. |
| `/memory score` | Show the memory scorecard. |

A prompt starting with `#` saves a note to memory. See [Memory](./memory).

### /learn

After a run where you corrected the agent, it may offer one lesson. `/learn keep` keeps it (as a memory, a permission rule or a line in {{INSTRUCTIONS}}); `/learn skip` drops it. See [Learning](./learning).

### /check

Has the agent check this branch's changes against the memories about the files they touch. Needs git and the `experimental-learning-check` feature.

### /rules

`/rules` lists permission rules your past answers suggest, numbered, with how many times you allowed or denied each. `/rules keep <n>` saves one to `{{DIR}}/settings.local.json`. See [Permissions](./permissions).

### /loop

`/loop [interval] <task>` runs the task now and again later. With an interval (`30s`, `5m`, `2h`, `1d`) the agent schedules it to recur at that interval; without one, the agent comes back to it when it decides, unless the task is done for good. See [Schedules](./schedules).

### /schedules

`/schedules` lists scheduled prompts (id, when, the wait and the prompt). `/schedules delete <id>` removes one. See [Schedules](./schedules).

### /bashes

`/bashes` lists commands left running in the background (id, running or finished, the command). `/bashes kill <id>` stops one.

### /gain

Shows how much shorter command output reached the model this session. Needs the `experimental-output-savings` feature.

### /features

`/features` lists the features you can turn off, each `on` or `off` with what it does (and `(not on this system)` when unavailable). `/features <name> on|off` switches one; it is saved in your settings and new runs follow it. Also reachable from `/config`. See [Experimental features](./experimental-features).

### /trace

Opens the live trace of every session in the browser: runs, spans, cost, waits and checks. Alias: `/console`. See [Trace console](./trace-console).

### /remote

`/remote` shows the relay, this machine's id, the paired phones and whether this chat is connected to the relay. `/remote pair` prints a QR code and a link that pair one phone within ten minutes, and connects this chat to the relay. `/remote off` stops it until the next `/remote pair` or the next start. Once a relay is named and a phone is paired, a new chat connects by itself; only one chat on the machine holds the relay at a time, and the others say so. See [`{{COMMAND}} remote`](./cli-reference#remote).

### /trust

`/trust` shows whether the project folder is trusted and lists what it brings that needs trust: hooks, scripts, MCP servers, declared providers, grants in `settings.local.json`. `/trust yes` trusts the folder and `/trust no` stops trusting it. A change takes effect on the next run. See [Workspace trust](./sandbox-and-safety#workspace-trust).

### /docs

`/docs` opens this documentation in the browser, served by {{NAME}} on this machine. `/docs <topic>` opens it searching for that topic, for example `/docs hooks`. The chat prints the address it opened.

### /factory-reset

Asks before erasing everything {{NAME}} keeps on this machine (sessions, memory, logins, settings, skills and agents in its data folders and `~/{{DIR}}`), then quits and reports what was erased. The program stays installed and project folders are left alone. The safe answer, **No, keep everything**, is the default.

> [!CAUTION]
> A factory reset cannot be undone.

## Skills and custom commands

Any `/name` that is not one of the commands above runs the skill or custom command of that name. The rest of the line becomes its arguments: they replace `$ARGUMENTS` in the skill's text and are also appended as `Arguments: …`.

```text
/review-pr 123
/acme:triage the login bug
```

| Source | Becomes |
|---|---|
| `{{DIR}}/skills/<name>/SKILL.md` (or `SKILL.js`) in the project, its parent folders or `~/{{DIR}}` | `/<name>` (or the `name` in its frontmatter) |
| `{{DIR}}/commands/<name>.md` (or `.js`) | `/<name>`; a file in a subfolder, `commands/git/push.md`, becomes `/git:push` |
| A plugin's skills and commands | `/<plugin>:<name>` |
| An MCP server's prompts | `/mcp__<server>__<prompt>` (see [MCP](./mcp)) |

They appear in the `/` completions and in `/skills`. `/help <name>` shows a skill's description. See [Skills](./skills) and [Plugins](./plugins).
