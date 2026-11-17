---
title: Files and directories
description: Every file and folder {{NAME}} reads or writes, in your home folder, in your projects, in its data folder and in the temporary folder.
---

{{NAME}} keeps things in four places:

| Place | What it holds | Who edits it |
|---|---|---|
| `~/{{DIR}}/` | Your settings, instructions, skills, agents, plugins and other extensions, for every project. | You (and a few commands). |
| `{{DIR}}/` in a project | The project's settings, instructions, skills, agents and other extensions. Mostly checked in. | You and your team. |
| The [data folder](#data-folder) | Sessions, memory, remembered choices, credentials, logs, caches. | {{NAME}} only. |
| The [temporary folder](#temporary-files) | Long tool outputs, the prompt being edited and a few working files. | {{NAME}} only. |

## Your folder: `~/{{DIR}}`

| Path | What it is | See |
|---|---|---|
| `settings.json` | Your settings, for every project. Written by `{{COMMAND}} config` and `/features`. | [Settings](./settings) |
| `settings.local.json` | More of your settings, read right after `settings.json`. | [Settings](./settings) |
| `managed-settings.json` | Settings read after every other file. | [Settings](./settings#settings-files) |
| `{{INSTRUCTIONS}}` | Your instructions, added to every conversation. | [Instruction files](./instructions-files) |
| `rules/` | Your rule files (`*.md`). | [Instruction files](./instructions-files) |
| `mcp.json` | Your MCP servers (`mcpServers`). Written by `{{COMMAND}} mcp add` and `remove`. | [MCP](./mcp) |
| `mcp/` | Your MCP servers written as JavaScript. | [MCP](./mcp) |
| `skills/` | Your skills, one folder each with a `SKILL.md`. | [Skills](./skills) |
| `commands/` | Your custom commands (`*.md` or `*.js`), run as `/name`. | [Skills](./skills) |
| `agents/` | Your sub-agents (`<name>.md`). | [Agents](./agents) |
| `agent-memory/` | Memory of agents whose memory scope is `user`. | [Agents](./agents) |
| `hooks/` | Your JavaScript hooks (`*.js`). | [Hooks](./hooks) |
| `tools/` | Your JavaScript tools (`*.js`). | [Skills](./skills) |
| `providers/` | Providers you declare in JavaScript. | [Models and providers](./models-and-providers) |
| `output-styles/` | Your custom output styles (`<name>.md`). | [Output styles](./output-styles) |
| `plugins/` | Installed plugins, one folder each. `{{COMMAND}} plugin install <git-url>` clones here. | [Plugins](./plugins) |
| `prompts/`, `styles/`, `repl/`, `types.d.ts` and built-in files in `agents/`, `skills/`, `hooks/`, `tools/` | Copies of the built-in prompts, styles, agents, skills, hooks and tools, and the types scripts can use. | below |
| `.content.json` | Fingerprints of those copies. | below |

### Built-in content copies

On first use, {{NAME}} copies its built-in agents, hooks, prompts, skills, styles and tools into `~/{{DIR}}` and records each file's fingerprint in `.content.json`. When you edit a copy, your version is used instead of the built-in one, and later updates leave it alone (an unedited copy is updated with each new version). Delete a file to get the built-in one back on the next start.

### A plugin's folder

Each plugin in `~/{{DIR}}/plugins/<name>/` can hold:

| Path | What it is |
|---|---|
| `{{PLUGIN_DIR}}/plugin.json` | The manifest: its `name`, and optionally `mcpServers` and `lspServers`. |
| `.mcp.json` | Its MCP servers (`mcpServers`), instead of the manifest's. |
| `.lsp.json` | Its language servers, instead of the manifest's. |
| `hooks/hooks.json` | Its hooks. |
| `skills/`, `commands/`, `agents/` | Its skills, commands and agents. |

## Project folder: `{{DIR}}/`

Settings, rules, skills, agents, commands, hooks and output styles are looked for in `{{DIR}}/` of the current folder **and every parent folder**, so a monorepo can share them from its root. The nearest folder wins when two define the same thing.

| Path | What it is | Checked in? | See |
|---|---|---|---|
| `{{DIR}}/settings.json` | The project's settings, for the whole team. | Yes | [Settings](./settings) |
| `{{DIR}}/settings.local.json` | Your private settings for this project. "Don't ask again", `/rules keep`, `{{COMMAND}} config --project` and `{{COMMAND}} mcp approve` write here. | No | [Settings](./settings) |
| `{{DIR}}/{{INSTRUCTIONS}}` | The project's instructions (alternative to `{{INSTRUCTIONS}}` at the root). | Yes | [Instruction files](./instructions-files) |
| `{{DIR}}/rules/` | Rule files (`*.md`). | Yes | [Instruction files](./instructions-files) |
| `{{DIR}}/mcp.json` | The project's MCP servers. The nearest one up the folder tree is used; its servers wait for your approval. | Yes | [MCP](./mcp) |
| `{{DIR}}/mcp/` | MCP servers written as JavaScript (current folder only). | Yes | [MCP](./mcp) |
| `{{DIR}}/skills/` | Project skills. | Yes | [Skills](./skills) |
| `{{DIR}}/commands/` | Project commands. | Yes | [Skills](./skills) |
| `{{DIR}}/agents/` | Project sub-agents. | Yes | [Agents](./agents) |
| `{{DIR}}/hooks/` | JavaScript hooks. | Yes | [Hooks](./hooks) |
| `{{DIR}}/tools/` | JavaScript tools (current folder only). | Yes | [Skills](./skills) |
| `{{DIR}}/providers/` | Providers declared by the project (current folder only); each waits until you add its id to `enabledProviders`. | Yes | [Models and providers](./models-and-providers) |
| `{{DIR}}/output-styles/` | Custom output styles. | Yes | [Output styles](./output-styles) |
| `{{DIR}}/verification.md` | Criteria every run is judged against when you set none (up to 64 KB; the nearest one, never the one in `~/{{DIR}}`). | Yes | [Verification and review](./verification-and-review) |
| `{{DIR}}/memory/` | Team memories, shared with `/memory share <id>`. | Yes | [Memory](./memory) |
| `{{DIR}}/evals/<name>/case.json` | Eval cases for `{{COMMAND}} eval`, with an optional `fixture/` folder. | Yes | [Evals](./evals) |
| `{{DIR}}/agent-memory/` | Memory of agents whose scope is `project` (at the repository root). | Your choice | [Agents](./agents) |
| `{{DIR}}/agent-memory-local/` | Memory of agents whose scope is `local` (at the repository root). | No | [Agents](./agents) |
| `{{DIR}}/worktrees/<id>/` | Git worktrees of agents that work in isolation (at the repository root). | No | [Branches and git](./branches-and-git) |

Instruction files outside `{{DIR}}/`, read in every folder from the root of the file system down to the current one:

| File | What it is |
|---|---|
| `{{INSTRUCTIONS}}` | The project's instructions. |
| `AGENTS.md` | Read too, for projects shared with other agents. |
| `{{LOCAL_INSTRUCTIONS}}` | Your private instructions for this folder; keep it out of version control. |

> [!TIP]
> Add `{{DIR}}/settings.local.json`, `{{DIR}}/agent-memory-local/`, `{{DIR}}/worktrees/` and `{{LOCAL_INSTRUCTIONS}}` to your `.gitignore`.

## Data folder

{{NAME}}'s own data lives outside your projects, in a folder that depends on the system:

| System | Data folder | Local data folder |
|---|---|---|
| macOS | `~/Library/Application Support/{{NAME}}` | same folder |
| Linux | `$XDG_CONFIG_HOME/{{COMMAND}}`, or `~/.config/{{COMMAND}}` | same folder |
| Windows | `%APPDATA%\{{NAME}}` | `%LOCALAPPDATA%\{{NAME}}` |

In the **data folder**:

| Path | What it is |
|---|---|
| `users/<user>/settings.json` | Your [remembered chat preferences](./settings#remembered-chat-preferences): model, mode, effort, style, prompt history, last conversation per folder. At most 256 KB; a file that does not parse is moved to `settings.json.corrupt-<time>`. |
| `users/<user>/trusted-folders.json` | The folders you trusted (see [Workspace trust](./sandbox-and-safety#workspace-trust)). |
| `app.db` | A SQLite database: the list of your conversations and your memories for each project. Readable only by you. |
| `app.db-wal`, `app.db-shm` | SQLite's working files for `app.db`. They exist while {{NAME}} is running; leave them beside it. |
| `users/<user>/sessions.json.imported` | The conversation list an earlier version kept, renamed after it was imported into `app.db`. Not read any more. |
| `memory/<user>/<project>.json.imported` | The memories an earlier version kept for a project, renamed after they were imported into `app.db`. Not read any more. |
| `providers/<user>/providers.json` | Your connected providers and their accounts. |
| `code-map/<project>.json` | The code map index of each project (feature `experimental-code-map`). |
| `console/` | An installed copy of the trace console page, if present. |

In the **local data folder**:

| Path | What it is |
|---|---|
| `native-sessions/` | Conversations. For each one: `<session>.jsonl` (the conversation itself, saved after every step, so it survives a crash or a closed terminal), `<session>.trace.jsonl` (what `/trace` shows; when it passes 64 MB it is renamed to `<session>.trace.1.jsonl` and a new one starts, so only the latest part and the one before it are kept), `checkpoints-<session>.json` (for `/rewind`: the files of the 50 most recent prompts, up to 64 MB), `snapshot-<session>.json` (the date and local time zone, such as `America/Sao_Paulo, UTC-03:00`, the git state and top-level files the conversation started with, so its prompt stays the same) and `worktree-<session>.json`. A `<session>.lock` file exists only while the conversation is answering. Deleting the trace files loses the trace, never the conversation. |
| `secrets.json` | API keys and tokens, encrypted. Readable only by you. |
| `secrets.key` | Where the key that opens `secrets.json` is. By default it holds the key itself, as 64 hex digits. Only with `{{ENV}}_SECRETS=keychain` does it hold just the word `keychain`, the key being in your system keychain (macOS Keychain, the Secret Service on Linux, Windows Credential Manager). Readable only by you. Do not delete it: without it, the keys in `secrets.json` cannot be opened. |
| `logs/{{COMMAND}}.log` | The log (level from `{{ENV}}_LOG`). Over 5 MB it is renamed to `{{COMMAND}}.log.1` and a new one starts. |
| `lsp/` | Language servers {{NAME}} installed on first use. |

`<user>` is `local` for you on your own machine, and `<project>` is the project's path with every character that is not a letter or digit replaced by `-`.

Several running copies of {{NAME}} share `app.db` safely: each change is one transaction, so none of them loses what another wrote. To copy or back it up, quit {{NAME}} first, or copy `app.db`, `app.db-wal` and `app.db-shm` together.

If `app.db` cannot be opened (it was written by a newer version, another program holds it, or the file cannot be read), {{NAME}} still starts and says so once: `The session list and memory could not be opened (<reason>); they will not be kept after this session.` The chat shows it as a notice; with `--print` it goes to the error stream. Until the cause is fixed, the conversation list and what is remembered last only until {{NAME}} quits. The file is left as it is. The conversations themselves, in `native-sessions/`, are still saved.

### Coming from an earlier version

Earlier versions kept the conversation list in `users/<user>/sessions.json` and each project's memories in `memory/<user>/<project>.json`. The first time a version with `app.db` starts, it imports those files and renames each one by adding `.imported`. Nothing is deleted. A file that does not parse is renamed with `.corrupt-<time>` instead and the rest is imported.

> [!WARNING]
> Going back to an earlier version after the import: the earlier version looks for `sessions.json` and `<project>.json`, finds only the `.imported` files, and shows an empty conversation list and an empty memory. Your data is not lost. Quit {{NAME}} and rename the files back (remove `.imported` from each name) and the earlier version shows everything as it was at the moment of the import. Conversations listed and memories saved after the import exist only in `app.db`, which the earlier version does not read. The conversations themselves, in `native-sessions/`, are not affected either way.

> [!CAUTION]
> `/factory-reset` erases the data folder, the local data folder and `~/{{DIR}}` (sessions, memory, logins and settings) after asking you, then quits. Projects' `{{DIR}}/` folders are not touched.

## Temporary files

In the system temporary folder (`TMPDIR` on macOS and Linux):

| Path | What it is |
|---|---|
| `{{COMMAND}}-tool-results/<session>/` | Tool outputs too long for the conversation (over 30,000 characters), whole Bash outputs that passed `bashOutputLimit`, and whole command outputs that were filtered, so the agent can read them in parts. A conversation's folder is removed once nothing has been written to it for 7 days. |
| `{{COMMAND}}-prompt-<random>.md` | The prompt while you edit it in your editor (Ctrl+G); readable only by you, removed afterwards. |
| `{{COMMAND}}-claims/<hash>.json` | Which session last edited which file, per project, for the `experimental-session-claims` warning. |
| `{{COMMAND}}-memory/<run>.json` | The memory changes a run asks for, before {{NAME}} applies them. |

## Files {{NAME}} only reads

| File | Why |
|---|---|
| `.env`, `.env.*` at the project root | With `redact-secrets` on, their values are hidden from tool output. See [Sandbox and safety](./sandbox-and-safety#secret-redaction). |
| `.git` | Finding the repository root (instructions, worktrees, agent memory, code map). |
