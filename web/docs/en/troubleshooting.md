---
title: Troubleshooting
description: Check your installation with the doctor, find the logs, and fix the problems people run into most.
---

When something does not work, start with the doctor: it checks the pieces {{NAME}} needs in one go and says what to fix. If that is not enough, the log file has the details.

## Run the doctor

From a terminal, in the project folder:

```bash
{{COMMAND}} doctor
```

Or, in the chat:

```text
/doctor
```

```text
All checks passed:
  ✓ version      0.3.1
  ✓ providers    Anthropic API, Grok Subscription
  ✓ models       42 available
  ✓ settings     6 permission rules, 2 hooks
  ✓ trust        /home/ana/rocket is trusted
  ✓ mcp servers  3 configured
  ✓ declared providers none (~/{{DIR}}/providers/*.js adds one)
  ✓ sandbox      on: commands write only inside the project, added folders, temp and caches
  ✓ git          git version 2.47.0
  ✓ ripgrep      ripgrep 14.1.1 (Grep searches with it)
```

When a check fails, the first line counts the problems (`1 problem(s):`) and the check is marked `✗`. `{{COMMAND}} doctor` exits with 0 when every check passes and 1 otherwise, so you can use it in scripts. Use `-C <path>` to check another folder.

### What each check means

| Check | Passes when | When it fails |
|---|---|---|
| `version` | Always; shows the installed version | |
| `providers` | At least one provider is connected; lists them | `none connected: run {{COMMAND}} login`. See [Models and providers](./models-and-providers). |
| `models` | The connected providers offer at least one model; shows how many | `none available: connect a provider`, or the error the provider returned |
| `settings` | Every settings file here reads; shows the permission rules and hooks found | The file with the problem and why, then `(fix the file it names)`. See [Settings](./settings). |
| `trust` | Whether this folder is trusted; when it is not, how many project items were skipped, and that `/trust` (or `--trust` with `--print`) trusts it | Only when the trusted folders cannot be read. See [Workspace trust](./sandbox-and-safety#workspace-trust). |
| `mcp servers` | The MCP configuration reads; shows how many servers | The configuration error. See [MCP](./mcp). |
| `declared providers` | Every file in `~/{{DIR}}/providers/` and `{{DIR}}/providers/` loaded | Each file that did not, and why |
| `sandbox` | Always; says whether commands are confined | |
| `git` | `git` runs; shows its version | `` `git` was not found on PATH `` |
| `ripgrep` | Always; shows `rg`'s version, or that it is not installed | |

The `sandbox` line reads:

| Line | Meaning |
|---|---|
| `on: commands write only inside the project, added folders, temp and caches` | The sandbox confines commands |
| `off in your settings (/features experimental-sandbox on)` | It is available and turned off; the command in the line turns it on |
| `not on this system: commands run unconfined` | This system has no sandbox |
| `not offered` | The sandbox feature is not offered |

See [Sandbox and safety](./sandbox-and-safety).

The `declared providers` line says `none (~/{{DIR}}/providers/*.js adds one)` when there are none and `<n> loaded` when all loaded. A file that did not load fails the check with one of:

- `<file>: the project's provider <id> waits: add it to enabledProviders in your settings`
- `<file>: left out: <why>`, for example a wrong `wire`, a `base` that is not `https`, an id a built-in provider already has, or `the default export is not a provider`

See [Declared providers](./models-and-providers#declared-providers).

> [!NOTE]
> `ripgrep` is optional. When `rg` is on `PATH`, the `Grep` tool uses it to find the files that match, which is much faster on large projects; without it, `Grep` searches with its built-in engine and gives the same results. Either way the check passes, with `not installed (Grep uses its built-in search, slower on large projects)` when it is missing. `git` is needed for `/diff` and worktrees.

## Logs

{{NAME}} writes a log file in its local data folder:

| System | Log file |
|---|---|
| macOS | `~/Library/Application Support/{{NAME}}/logs/{{COMMAND}}.log` |
| Linux | `$XDG_CONFIG_HOME/{{COMMAND}}/logs/{{COMMAND}}.log`, or `~/.config/{{COMMAND}}/logs/{{COMMAND}}.log` |
| Windows | `%LOCALAPPDATA%\{{NAME}}\logs\{{COMMAND}}.log` |

By default only warnings and errors are written. Set `{{ENV}}_LOG` to log more, with a filter such as `info`, `debug` or `app=debug`:

```bash
{{ENV}}_LOG=debug {{COMMAND}} -p "hello"
```

A filter that cannot be read falls back to `warn`. Each line carries the thread's name, and a crash is written to the log with the thread it happened on.

When the file grows past 5 MiB, it is moved to `{{COMMAND}}.log.1` the next time {{NAME}} starts, and a new file begins; the older `.log.1` is replaced.

> [!TIP]
> When you report a problem, attach the end of the log file and the output of `{{COMMAND}} doctor`.

## Common problems

### No model is available

```text
{{COMMAND}}: no model is available: run `{{COMMAND}} login` first
```

No provider is connected, or the connected providers offer no models. Run `{{COMMAND}} login`, then `{{COMMAND}} models` to see what is offered. In the chat, `/model` says `No models yet. Run {{COMMAND}} login to connect a provider.`

### A model name is not found

```text
{{COMMAND}}: no model named sonnet: `{{COMMAND}} models` lists them
```

A model can be named by its full id (`anthropic:claude-sonnet-5-5`), the part after the colon, or its label. Check the spelling with `{{COMMAND}} models`. In the chat the message is `No model named <name>. /model lists them.`

### A browser sign-in does not finish

- If the browser does not open, copy the URL {{NAME}} prints.
- ChatGPT Subscription waits on port 1455, Kiro Subscription on port 49153 and Antigravity Subscription on port 51121. Close whatever else uses that port and try again.
- Kiro accepts Google, GitHub, Builder ID and organization (IAM Identity Center) sign-ins. An organization sign-in fails when Kiro's portal does not say the organization's start URL (it must be `https`) or its AWS region; check the start URL and try again.
- GitHub Copilot refuses classic `ghp_` tokens.

### The agent cannot do anything in print mode

```text
denied: the agent asked for permission (use --mode or --allowedTools)
```

In print mode nobody can answer a permission prompt, so every request is denied. Allow what the run needs with `--allowedTools`, or pick a `--mode`. See [Headless mode](./headless#choose-what-the-agent-may-do).

### The plan is not carried out

```text
plan not carried out: run again with --mode edits to implement it
```

The run was in plan mode. Run it again with `--mode edits`.

### The agent stops after too many steps

```text
The agent stopped after too many steps without finishing.
```

The run used all its turns: 200 by default, or `--max-turns` in print mode. Split the task, or raise `--max-turns`.

### The model call failed or an account is busy

```text
The model call failed; going on with <model>.
Account …aaaa is busy or signed out; going on with …bbbb.
```

These are not errors: a [fallback model](./models-and-providers#fallback-models) or [another account](./accounts#share-the-work-between-accounts) took over. If calls keep failing, check `{{COMMAND}} doctor` and the log.

### The cache went cold

```text
cache cold: next turn rewrites 48k tokens
```

The provider's prompt cache expires after about five minutes without a call, so the next turn pays to write it again. See [Prompt caching](./models-and-providers#prompt-caching).

### A settings file has an error

The `settings` check names the file and the problem, for example an invalid JSON value or `bashOutputLimit must be a whole number of characters`. Fix that file, or inspect it with `{{COMMAND}} config list` (`--project` for the project's). In the chat, a saved `mode` or `effort` that is not known is reported as `The saved effort "..." is not low, medium, high, xhigh, max or auto; using the default.`

### A project's MCP server does not start

A server from a project's `{{DIR}}/mcp.json` waits for your approval. `{{COMMAND}} mcp` lists it as `waiting for approval` and says how to approve it (in a folder you have not trusted it is listed as `skipped: folder not trusted`; trust the folder first):

```bash
{{COMMAND}} mcp approve <name>
```

### `-c` starts a new conversation

```text
{{COMMAND}}: no earlier conversation in this folder; starting a new one
```

`-c` goes on with the last conversation of the current folder. Run it from the same folder, or use `-r` to pick one.

### Shift+Enter does not add a line

```text
/terminal-setup
```

Alt+Enter and Ctrl+J add a line in every terminal. Shift+Enter works in terminals that report it: Ghostty, kitty, WezTerm, and iTerm2 with *Report modifiers using CSI u*. In other terminals, map Shift+Enter to send `\n` in the terminal's key settings. {{NAME}} asks the terminal for these keys when it can. See [Keyboard shortcuts](./keyboard-shortcuts).

### Colours or animations look wrong

- The colours follow the terminal's background, read from `COLORFGBG`. Set `{{ENV}}_THEME=light` or `{{ENV}}_THEME=dark` to choose.
- `{{ENV}}_REDUCED_MOTION=1` turns the animations off.

## Start over

`/factory-reset` erases everything {{NAME}} keeps on this machine and quits:

```text
/factory-reset
```

It asks first, with *No, keep everything* as the first choice; nothing is erased unless you pick *Yes, erase and quit*, which lists the folders it removes:

- the data folder: sessions, memory, logins (keys and tokens) and settings;
- the local data folder with the logs, where it is a different folder (on Windows);
- `~/{{DIR}}`: your settings, skills, agents, plugins and declared providers.

The program stays installed, and project folders (including each project's `{{DIR}}`) are left alone. If a folder cannot be erased, {{NAME}} says which and why.

> [!CAUTION]
> A factory reset cannot be undone. You will need to connect your providers again with `{{COMMAND}} login`.

If nothing is kept yet, it says `{{NAME}} keeps nothing on this machine yet.`

See [Files and directories](./files-and-directories) for what lives in each folder.
