---
title: Settings
description: Every key {{NAME}} reads from its settings files, where those files live, how they combine, and how to change them.
---

{{NAME}} is configured with JSON files named `settings.json`. You can keep one for yourself, one per project that you check in for your team, and a private one per project that stays on your machine. This page explains where they live, how they combine, and lists every key they can hold.

## Quick start

Create `~/{{DIR}}/settings.json` with the settings you want everywhere:

```json
{
  "outputStyle": "concise",
  "permissions": {
    "allow": ["Bash(cargo test:*)", "Bash(git diff:*)"],
    "deny": ["Read(.env)"]
  }
}
```

Or change one key from the terminal without opening an editor:

```bash
{{COMMAND}} config set outputStyle concise
{{COMMAND}} config set permissions.allow '["Bash(cargo test:*)"]'
{{COMMAND}} config get permissions.allow
```

New runs pick up the change; there is nothing to restart.

## Settings files

| File | Scope | Trusted as yours | Typical use |
|---|---|---|---|
| `~/{{DIR}}/settings.json` | You, in every project | Yes | Your own preferences, models, permissions. `{{COMMAND}} config` and `/features` write here. |
| `~/{{DIR}}/settings.local.json` | You, in every project | Yes | Same as above, if you prefer to keep a second file. |
| `{{DIR}}/settings.json` in the project (or any parent folder) | Everyone who clones the project | No: checked in | Team rules: deny and ask rules, hooks, output style. |
| `{{DIR}}/settings.local.json` in the project | You, in this project | Yes, once you trust the folder | Your private project choices. "Don't ask again" answers, `/rules keep` and `{{COMMAND}} config --project` write here. |
| `~/{{DIR}}/managed-settings.json` | You, in every project | Yes | Read last, after every other file. |

> [!IMPORTANT]
> A project's `{{DIR}}/settings.local.json` counts as yours only in a folder you trust (see [Workspace trust](./sandbox-and-safety#workspace-trust)). Until then it is read like the checked-in `settings.json` and can only restrict, so a rule you saved with "don't ask again" holds for that run only and applies to later ones once the folder is trusted; the chat says so when it saves the rule. Keep the file out of version control: in a trusted folder anything in it can grant permissions.

A `{{DIR}}/` folder above the project (above the folder that holds `.git`, or above the current folder when there is no repository) is read like a checked-in file, whatever its name and whatever you trusted: it can restrict, never grant.

### Which folders are read

For a run in a folder, {{NAME}} reads, in this order:

1. `~/{{DIR}}/settings.json`, then `~/{{DIR}}/settings.local.json`.
2. For every folder from the root of the file system down to the current folder: `{{DIR}}/settings.json`, then `{{DIR}}/settings.local.json`. A folder that is reached twice (your home folder is usually also a parent of the project) is read only once.
3. `~/{{DIR}}/managed-settings.json`.

Missing files are skipped. A file that is not valid JSON stops the run with an error that names the file.

A value of the wrong type, or outside its choices, does not stop the run: that value is left out (in a list, only that item; in `hooks`, that hook), the rest of the file still applies, and `/permissions` and `/hooks` show a notice that names the file, the key's path and what was expected:

```text
/home/me/{{DIR}}/settings.json: permissions.deny[1]: expected string, found 5
/home/me/{{DIR}}/settings.json: hooks.Stop[0].hooks[0].type: expected one of "command", "script", "http", "prompt", "agent", found "shell"
```

A key {{NAME}} does not know is named the same way (`theme: unknown key, ignored`) and otherwise ignored. A value of the right type that {{NAME}} still cannot use, such as a rule it cannot parse, a hook matcher that is not a regex or a model name with spaces, stops the run with an error that names the file and the key, for example `permissions.deny: rule "Bash(rm" misses its )`.

### JSON Schema

Both settings files and `mcp.json` have a published JSON Schema, so an editor can complete keys and underline mistakes as you type. {{NAME}} copies the schemas into your own folder:

| File | Schema |
|---|---|
| `settings.json`, `settings.local.json`, `managed-settings.json` | `~/{{DIR}}/schemas/settings.schema.json` |
| `mcp.json` | `~/{{DIR}}/schemas/mcp.schema.json` |

Point a file at its schema with a `$schema` key. {{NAME}} ignores the key, and it is never reported as unknown. In `~/{{DIR}}/settings.json` a relative path works:

```json
{
  "$schema": "./schemas/settings.schema.json",
  "outputStyle": "concise"
}
```

A project's `{{DIR}}/settings.json` lives elsewhere, so use the full path to your copy (for example `"$schema": "/home/me/{{DIR}}/schemas/settings.schema.json"`) or map the file pattern to the schema in your editor's settings. The schemas match the version of {{NAME}} that wrote them and are refreshed when it updates, unless you edited your copy. In the source, they are `content/schemas/`.

### How values combine

- **Single values** (`outputStyle`, `agent`, `advisorModel`, `judgeModel`, `escalationModel`, `fallbackModel`, `smallModel`, `teammateMode`, `jsonSchema`, `bashOutputLimit`, `enableAllProjectMcpServers`): the last file read wins. In practice the folder nearest to where you work beats its parents, and `managed-settings.json` beats everything. Setting `advisorModel`, `judgeModel`, `smallModel`, `escalationModel`, `fallbackModel`, `agent` or `jsonSchema` to `null` clears a value set by an earlier file.
- **Lists and maps add up**: permission rules, `verification.allow`, `permissions.additionalDirectories`, `hooks`, `lspServers`, `enabledProviders`, `enabledMcpServers`, `disabledMcpServers`, `sandbox.allowWrite`, `sandbox.allowUnixSockets`, `sandbox.allowRead`.
- **Switches** (`features`, `scripts`, `sandbox.network`, `sandbox.required`): each file changes only the names it mentions.

### What a checked-in file cannot do

A project's `{{DIR}}/settings.json` comes from whoever wrote the repository, so it can restrict {{NAME}} but not widen what it may do:

| Key | In a checked-in `settings.json` |
|---|---|
| `permissions.allow` | Ignored. Only your files allow. |
| `permissions.ask`, `permissions.deny` | Honoured. |
| `permissions.additionalDirectories` | Ignored. |
| `verification.allow` | Ignored. |
| `escalationModel` | Ignored. |
| `enabledProviders` | Ignored. |
| `enableAllProjectMcpServers`, `enabledMcpServers`, `disabledMcpServers` | Ignored. |
| `hooks` | Skipped until you trust the folder. See [Hooks](./hooks#project-hooks-and-workspace-trust). |
| `scripts.*` | Can only turn access off. It cannot give the project's own scripts access. |
| `features.*` | Can turn an ordinary feature off, and can turn a security feature (`experimental-sandbox`, `redact-secrets`, `experimental-dependency-guard`) on, never the other way round. |
| `sandbox.network` | Can only turn the network off. |
| `sandbox.allowWrite` | Ignored. |
| `sandbox.allowUnixSockets` | Ignored. |
| `sandbox.allowRead` | Ignored. |
| `sandbox.required` | Can only turn it on. |
| `lspServers` | `command`, `args` and `env` are dropped; a project can only map extensions, root markers or disable a server. |
| Everything else | Honoured as in your own files. |

## Changing settings

### `{{COMMAND}} config`

`{{COMMAND}} config` reads and edits one settings file by dotted key.

| Command | What it does |
|---|---|
| `{{COMMAND}} config list` | Prints the whole file. |
| `{{COMMAND}} config get <key>` | Prints one value, for example `{{COMMAND}} config get sandbox.network`. Fails when the key is not set. |
| `{{COMMAND}} config set <key> <value>` | Sets one value. The value is parsed as JSON (`true`, `3`, `["Bash(ls:*)"]`, `{"network": false}`); anything that is not JSON is kept as text. Missing parent objects are created. |
| `{{COMMAND}} config unset <key>` | Removes one key. |

Without options it edits `~/{{DIR}}/settings.json`. With `--project` it edits `{{DIR}}/settings.local.json` in the current folder (or the folder given with `-C`).

```bash
{{COMMAND}} config set features.experimental-sandbox false
{{COMMAND}} config --project set permissions.deny '["Bash(rm:*)"]'
{{COMMAND}} config unset outputStyle
```

### In the chat

- `/config` (also `/settings`) opens the **Settings** picker: Model, Permission mode, Effort, Output style, Vim keys, Status line, plus Features, Verification and Budget. Picking an entry runs the matching command (`/model`, `/mode`, `/effort`, `/output-style`, `/vim`, `/statusline`, `/features`, `/verification`, `/budget`). These choices are remembered for you, see [Remembered chat preferences](#remembered-chat-preferences).
- `/features <name> on|off` writes `features.<name>` to `~/{{DIR}}/settings.json`. See [Experimental features](./experimental-features).
- Answering **Yes, and don't ask again for …** to a permission request, or `/rules keep <n>`, adds a rule to `{{DIR}}/settings.local.json`. See [Permissions](./permissions).
- `/permissions` shows the rules and folders in effect; `/hooks` shows the hooks.

## Reference

### Top-level keys

| Key | Type | Default | Description |
|---|---|---|---|
| `outputStyle` | string | `"default"` | How the agent talks. A built-in style (`default`, `concise`, `proactive`, `explanatory`, `learning`, any case) or the name of a custom style in `{{DIR}}/output-styles/<name>.md` (letters, digits, `-` and `_`). The style you pick with `/output-style` wins over this key. See [Output styles](./output-styles). |
| `agent` | string or `null` | none | Runs the main conversation as this agent (a name `/agents` lists), with its instructions, tools and model. An unknown name fails the run. See [Agents](./agents). |
| `bashOutputLimit` | integer, 1000 to 1000000 | `30000` | Characters of a Bash command's output the agent sees. Past it, the agent gets the start and the end of the output under a first line such as `[Output cut: 81234 characters in 2010 lines, over the limit of 30000; the first 15000 and the last 15000 are shown. The full output is in /tmp/…/bash-123-4.txt: Read or Grep it for the rest.]`, and the whole output is saved to that file. The chat shows the same line above the folded output. A value outside the range is moved to the nearest end. |
| `advisorModel` | string or `null` | none | Model for the advisor the agent consults. A model id or name, no spaces. A role set with `/model advisor <model>` wins. |
| `judgeModel` | string or `null` | none | Model for the judge that checks finished work against verification criteria. A role set with `/model review <model>` wins. See [Verification and review](./verification-and-review). |
| `smallModel` | string or `null` | the provider's small model | Model for background chores: titles, effort picks, web reading. A role set with `/model small <model>` wins. |
| `teammateMode` | `process`, `tmux`, `iterm2` or `auto` | `process` | Where a lead starts its teammates: in the background, printing to a log (`process`), in a new pane of the tmux window the lead runs in (`tmux`), in a new iTerm2 pane (`iterm2`), or `auto`: tmux inside tmux, else iTerm2 inside iTerm2, else in the background. See [Agent teams](./agents#agent-teams). |
| `escalationModel` | string or `null` | none | Where a call the agent keeps failing is sent when the `experimental-escalation` feature is on (otherwise the Advisor). Read only from your own files. |
| `fallbackModel` | string or list of strings | none | Model, or chain of models, tried when the chosen one is busy, signed out or unreachable. `--fallback-model` and `/model fallbacks` win over it. See [Models and providers](./models-and-providers). |
| `jsonSchema` | JSON Schema object or `null` | none | Gives the agent a `StructuredOutput` tool whose `result` must match this schema. Useful with `-p`; see [Headless](./headless). An invalid schema fails the run. |
| `enabledProviders` | list of strings | `[]` | Ids of providers declared by the project in `{{DIR}}/providers/` that you allow to load. Read only from your own files. |
| `enableAllProjectMcpServers` | boolean | `false` | Starts every MCP server the project's `{{DIR}}/mcp.json` declares without asking. Read only from your own files. |
| `enabledMcpServers` | list of strings | `[]` | Project MCP servers you approved. `{{COMMAND}} mcp approve` and `/mcp approve` write here. Read only from your own files. |
| `disabledMcpServers` | list of strings | `[]` | Project MCP servers you turned off; wins over the two keys above. Read only from your own files. |
| `hooks` | object | `{}` | Commands, prompts or HTTP calls that run on events. Hooks from every file are combined; a project's hooks are skipped until you trust its folder. See [Hooks](./hooks#project-hooks-and-workspace-trust). |
| `lspServers` | object | `{}` | Adds, changes or disables language servers. See [`lspServers`](#lspservers) and [Code intelligence](./code-intelligence). |
| `permissions` | object | `{}` | Allow, ask and deny rules and extra folders. See [`permissions`](#permissions). |
| `verification` | object | `{}` | Rules for the verification judge. See [`verification`](#verification). |
| `scripts` | object | all `true` for your scripts, all `false` for a project's | What JavaScript tools, skills, hooks and providers may do. See [`scripts`](#scripts). |
| `sandbox` | object | network on | Where sandboxed commands may write and whether they reach the network. See [`sandbox`](#sandbox). |
| `features` | object | every feature on | Turns features on or off by name. See [`features`](#features). |

`$schema` names the file's [JSON Schema](#json-schema). Any other key {{NAME}} does not know is ignored with a notice. `trustProjectHooks`, which earlier versions read, no longer does anything: a project's hooks run once you trust its folder, and the chat says so until you remove the key.

> [!NOTE]
> MCP server definitions (`mcpServers`) do not live in `settings.json`: they live in `{{DIR}}/mcp.json` (the project's) and `~/{{DIR}}/mcp.json` (yours). See [MCP](./mcp). The status line command is a chat preference (see [Remembered chat preferences](#remembered-chat-preferences) and [Status line](./statusline)).

### `permissions`

| Key | Type | Default | Description |
|---|---|---|---|
| `permissions.allow` | list of rules | `[]` | Tool calls that run without asking. Read only from your own files. |
| `permissions.ask` | list of rules | `[]` | Tool calls that always ask, even when the mode would allow them. |
| `permissions.deny` | list of rules | `[]` | Tool calls that are refused, in every mode. |
| `permissions.additionalDirectories` | list of folders | `[]` | Folders the agent reads like the project. `~/` starts at your home folder, a relative path at the project. Read only from your own files. |

```json
{
  "permissions": {
    "allow": ["Bash(npm run test:*)", "Edit(src/**)", "WebFetch(domain:docs.rs)", "mcp__jira"],
    "ask": ["Bash(git push:*)"],
    "deny": ["Bash(rm:*)", "Read(*.env)", "mcp__github__delete_repo"],
    "additionalDirectories": ["~/notes", "../shared"]
  }
}
```

Rule syntax and precedence are on the [Permissions](./permissions) page.

### `verification`

| Key | Type | Default | Description |
|---|---|---|---|
| `verification.allow` | list of rules | `[]` | Rules the verification judge may run without asking (for example the test command it uses to check the work). Read only from your own files. |

```json
{ "verification": { "allow": ["Bash(cargo test:*)", "Bash(npm test)"] } }
```

### `scripts`

| Key | Type | Your scripts | A project's scripts | Description |
|---|---|---|---|---|
| `scripts.files` | boolean | `true` | `false` | Scripts may read and write files (`readFile`, `writeFile`). |
| `scripts.processes` | boolean | `true` | `false` | Scripts may start processes (`exec`). |
| `scripts.network` | boolean | `true` | `false` | Scripts may reach the network (`fetch`). |

The default depends on where the script lives:

- **Your scripts** are the ones under `~/{{DIR}}/` and the ones {{NAME}} ships. They have all three kinds of access unless a settings file turns one off.
- **A project's scripts** are the ones under the project's `{{DIR}}/`: its tools, skills, hooks, providers and script MCP servers. They have **none** of the three until one of your own settings files grants it.

Your own files are `~/{{DIR}}/settings.json`, `~/{{DIR}}/settings.local.json`, `~/{{DIR}}/managed-settings.json` and a project's `{{DIR}}/settings.local.json` in a folder you trust. A value set there applies to both groups: `true` grants the access to project scripts as well, `false` takes it from your scripts too. A checked-in `{{DIR}}/settings.json` can only turn access off.

A script that is refused gets an error naming the setting, for example `scripts may not touch files here (settings: scripts.files)`.

These settings are the outer limit. Every script must also list what it uses (`readFile`, `writeFile`, `exec`, `fetch`): a script tool in `spec.capabilities`, a hook or skill script in `export const capabilities = [...]`, a JavaScript MCP server in the `capabilities` of its default export. Each such call is then judged by your permission rules and mode like the matching tool, and where nobody can be asked a call that would ask is refused; see [Capabilities](./sandbox-and-safety#capabilities).

Let the scripts of the project you are working in read files and run commands, but not reach the network. Put this in that project's `{{DIR}}/settings.local.json` so it applies only there:

```json
{ "scripts": { "files": true, "processes": true } }
```

Turn the network off for every script, yours included:

```json
{ "scripts": { "network": false } }
```

> [!NOTE]
> Earlier versions gave a project's scripts all three kinds of access by default. If a project tool or hook now fails with `settings: scripts.…`, grant what it needs in one of your own files as shown above.

### `sandbox`

| Key | Type | Default | Description |
|---|---|---|---|
| `sandbox.network` | boolean | `true` | `false` blocks the network for sandboxed commands. A checked-in file can only turn it off. |
| `sandbox.allowWrite` | list of folders | `[]` | Extra folders sandboxed commands may write to. Absolute paths or `~/…`; relative paths are ignored. Read only from your own files. |
| `sandbox.allowUnixSockets` | list of socket paths | `[]` | macOS: service sockets sandboxed commands may connect to (the Docker, Podman, containerd, BuildKit, Colima, tmux and screen sockets are refused otherwise). Absolute paths or `~/…`. Read only from your own files. |
| `sandbox.allowRead` | list of paths | `[]` | macOS: paths sandboxed commands may read although they are credential stores (`~/.ssh`, `~/.aws`, `~/.kube`, `~/.npmrc`, `~/.gnupg`, …) or the app's data folder, which are unreadable otherwise. A path inside a store, such as `~/.ssh/known_hosts`, opens only that path. Absolute paths or `~/…`. Read only from your own files. See [Sandbox](./sandbox-and-safety#sandbox). |
| `sandbox.required` | boolean | `true` on Linux and macOS, `false` elsewhere | `true` refuses commands, and does not start MCP servers that run as a process, when this system cannot provide the sandbox, instead of running them unconfined. Turn it off in your own settings with `{ "sandbox": { "required": false } }`. A checked-in file can only turn it on. |

```json
{ "sandbox": { "allowWrite": ["~/.m2", "/opt/data"], "network": true } }
```

The sandbox is the `experimental-sandbox` feature (macOS; Linux 5.13 or later). MCP servers that run as a process are sandboxed too, each with its own `sandbox` entry in `mcp.json`; see [MCP servers in the sandbox](./sandbox-and-safety#mcp-servers-in-the-sandbox).

### `lspServers`

A map from server name to its configuration. Using a built-in name (`rust-analyzer`, `typescript`, `pyright`, `gopls`, `clangd`) changes that server; a new name adds one (it needs `command` and `extensionToLanguage`).

| Key | Type | Description |
|---|---|---|
| `lspServers.<name>.command` | string | Program to start. Ignored in a checked-in file. |
| `lspServers.<name>.args` | list of strings | Arguments. Ignored in a checked-in file. |
| `lspServers.<name>.extensionToLanguage` | object | File extension to language id, for example `{".zig": "zig"}`. |
| `lspServers.<name>.rootMarkers` | list of strings | Files that mark the server's project root. |
| `lspServers.<name>.env` | object | Environment variables for the server, added to the built-in ones. Ignored in a checked-in file. |
| `lspServers.<name>.disabled` | boolean | `true` removes the server. |

```json
{
  "lspServers": {
    "zls": { "command": "zls", "extensionToLanguage": { ".zig": "zig" }, "rootMarkers": ["build.zig"] },
    "clangd": { "disabled": true }
  }
}
```

### `features`

Every feature is on until you turn it off. Each key is `features.<name>` with `true` or `false`; a value that is not a boolean is left out with a notice. `/features` lists them with their state, and [Experimental features](./experimental-features) explains each one.

| Key | Default | What it controls |
|---|---|---|
| `features.experimental-sandbox` | `true` | Confine Bash and Monitor commands to the project's folders (macOS; Linux with Landlock). Security feature. |
| `features.redact-secrets` | `true` | Hide tokens, keys and the project's `.env` values in tool output. Security feature. |
| `features.experimental-dependency-guard` | `true` | Ask before installing a package that is missing, brand new or a typo of a popular one. Security feature. |
| `features.destructive-snapshot` | `true` | Keep untracked files before `rm`, `git clean`, `git reset --hard` and the like, for `/rewind`. |
| `features.suggested-rules` | `true` | `/rules` offers permission rules from answers you keep giving. |
| `features.budget` | `true` | `/budget` stops a session's model calls at a spending limit. |
| `features.cache-warning` | `true` | Status line countdown for the prompt cache. |
| `features.experimental-escalation` | `true` | Send a call that keeps failing to the Advisor or `escalationModel`. |
| `features.experimental-learning` | `true` | Offer one lesson to keep after a run where you corrected the agent. |
| `features.team-memory` | `true` | Recall checked-in memories in `{{DIR}}/memory`; `/memory share <id>`. |
| `features.branches` | `true` | `/branch` starts a conversation from before an earlier prompt. |
| `features.git-offer` | `true` | Offer `git init` in a folder that is not a repository. |
| `features.experimental-session-claims` | `true` | Warn when another session in the same folder edited the same file. |
| `features.experimental-pr-follow` | `true` | The `follow-pr` skill watches a pull request. |
| `features.experimental-affected-tests` | `true` | Run the tests covering an edited file in the background. |
| `features.experimental-output-cleanup` | `true` | Bash runs without colours and drops escape codes and repeated lines. |
| `features.experimental-output-filters` | `true` | Bash hides passing tests and progress from common tools, keeping the whole output in a file. |
| `features.experimental-output-savings` | `true` | `/gain` shows how much shorter command output became. |
| `features.experimental-web-main-content` | `true` | WebFetch puts a page's main content first. |
| `features.experimental-web-cleanup` | `true` | WebFetch drops hidden parts and repeated menu links. |
| `features.experimental-web-links` | `true` | WebFetch numbers links and lists each address once. |
| `features.experimental-web-small-pages-direct` | `true` | WebFetch hands pages of 6,000 characters or less to the agent directly. |
| `features.experimental-web-formats` | `true` | WebFetch asks for Markdown and compacts JSON and feeds. |
| `features.experimental-web-empty-page-error` | `true` | WebFetch reports a page with almost no text as an error. |
| `features.auto-review` | `true` | A read-only reviewer checks the changes before a run that changed files ends. |
| `features.experimental-follow-through` | `true` | A small model sends the agent back when it ends a turn promising more work. |
| `features.project-criteria` | `true` | Judge runs against `{{DIR}}/verification.md` when you set no criteria. |
| `features.experimental-memory-anchors` | `true` | Memories name their files and surface when those files are read or edited. |
| `features.experimental-structured-learnings` | `true` | Decisions are kept as decided, ruled out and watch-outs. |
| `features.experimental-learning-check` | `true` | `/check` compares the branch with memories about the files it touches. |
| `features.experimental-blast-radius` | `true` | After an edit, name the files that call the changed symbols. |
| `features.experimental-code-map` | `true` | Index symbols with tree-sitter and point each prompt at matching code. |
| `features.experimental-memory-seed` | `true` | `/memory seed` offers repeated requests as memories. |
| `features.experimental-memory-scorecard` | `true` | One request in 20 runs without recalled memory, for `/memory score`. |
| `features.experimental-load-on-call` | `true` | A tool waiting behind ToolSearch that the agent calls by name is loaded and run, not refused. |
| `features.experimental-code-mode` | `true` | REPL becomes the agent's only tool: it writes JavaScript that calls every other tool as a function. |
| `features.memory-search` | `true` | The `memory_search` tool. |
| `features.schedules` | `true` | `/loop` and `/schedules`. |
| `features.plugins` | `true` | Install, update, list and remove plugins. |
| `features.trace` | `true` | `/trace` and `/console`. |
| `features.channels` | `true` | MCP servers that stay open and send messages to the agent. |
| `features.declared-providers` | `true` | Load the providers declared in `providers/` folders. |

```json
{ "features": { "experimental-code-map": false, "git-offer": false } }
```

### Remembered chat preferences

Choices you make in the chat are not written to `settings.json`. They are remembered per user in the data folder (`users/local/settings.json`, see [Files and directories](./files-and-directories#data-folder)) and read back when the chat starts. You normally change them with commands, not by hand.

| Key | Type | Set by | Description |
|---|---|---|---|
| `model` | string | `/model`, `--model` | The model to start with. Also used by `-p` when `--model` is not given. |
| `mode` | string | `/mode`, Shift+Tab | `ask`, `edits`, `plan` or `bypass` (the other spellings in [Permission modes](./permissions#permission-modes) are read too). `--mode` wins for that start. |
| `effort` | string | `/effort` | `low`, `medium`, `high`, `xhigh`, `max` or `auto`. |
| `outputStyle` | string | `/output-style` | Wins over `outputStyle` in the settings files. |
| `vim` | `"on"` or `"off"` | `/vim` | Vim keys in the prompt. |
| `statusLine` | string | `/statusline` | Command that draws the status line; empty for the usual one. See [Status line](./statusline). |
| `roles` | object | `/model <role> <model>` | Model per role: `plan`, `explore`, `small`, `review`, `advisor`. |
| `fallbacks` | list of strings | `/model fallbacks` | Fallback chain for this user. |
| `history` | object | sending prompts | The last 100 prompts per folder, for the prompt's history. |
| `sessions` | object | conversations | The last conversation per folder, for `-c`. |
| `titles` | object | conversations | The title of the last conversation per folder. |
A saved `mode` or `effort` that is not a known value is ignored with a notice. If the file stops being valid JSON it is moved aside to `settings.json.corrupt-<time>` and the chat starts with the defaults.

## Full example

`~/{{DIR}}/settings.json`:

```json
{
  "outputStyle": "explanatory",
  "advisorModel": "opus",
  "judgeModel": "sonnet",
  "smallModel": "haiku",
  "escalationModel": "opus",
  "fallbackModel": ["sonnet", "gpt-5"],
  "enabledProviders": ["acme-llm"],
  "enableAllProjectMcpServers": false,
  "enabledMcpServers": ["jira"],
  "disabledMcpServers": ["sonar"],
  "permissions": {
    "allow": ["Bash(cargo test:*)", "Bash(git diff:*)", "Edit(src/**)", "WebFetch(domain:docs.rs)"],
    "ask": ["Bash(git push:*)"],
    "deny": ["Bash(rm -rf:*)", "Read(*.env)"],
    "additionalDirectories": ["~/notes"]
  },
  "verification": { "allow": ["Bash(cargo test:*)"] },
  "scripts": { "files": true, "processes": true, "network": false },
  "sandbox": { "network": true, "allowWrite": ["~/.m2"] },
  "features": { "experimental-code-map": false, "git-offer": false },
  "lspServers": {
    "zls": { "command": "zls", "extensionToLanguage": { ".zig": "zig" } }
  },
  "hooks": {
    "PostToolUse": [
      { "matcher": "Edit|Write", "hooks": [{ "type": "command", "command": "cargo fmt" }] }
    ]
  }
}
```

A project's checked-in `{{DIR}}/settings.json`:

```json
{
  "outputStyle": "concise",
  "agent": "reviewer",
  "jsonSchema": null,
  "permissions": {
    "ask": ["Bash(npm publish:*)"],
    "deny": ["Read(secrets/**)"]
  },
  "features": { "experimental-sandbox": true }
}
```
