---
title: Plugins
description: Install, write and share plugins that bring skills, commands, agents, hooks, MCP servers and language servers to {{NAME}} in one folder.
---

A **plugin** is a folder that bundles extensions for {{NAME}}: skills, custom commands, agents, hooks, MCP servers and language servers. Install one with a git URL and everything it contains is available in every project, under the plugin's name.

## Quick start

Install a plugin from git:

```bash
{{COMMAND}} plugin install https://github.com/acme/{{COMMAND}}-acme-tools.git
```

```text
Installed {{COMMAND}}-acme-tools. A new chat or /reload picks it up.
```

In a running chat, `/reload` shows what was found:

```text
Reloaded ~/{{DIR}} and the project:
  plugins  acme
  skills   14
  agents   5
  MCP      3 ready
```

Then use what it brings: `/acme:triage`, the `acme:reviewer` agent, or its MCP tools.

## Managing plugins

Plugins live in `~/{{DIR}}/plugins/`, one folder each.

| Command | What it does |
|---|---|
| `{{COMMAND}} plugin` or `{{COMMAND}} plugin list` | Lists the installed plugins (folder name and description) |
| `{{COMMAND}} plugin install <git-url-or-path> [--name <folder>]` | Clones it into `~/{{DIR}}/plugins/<folder>` (a shallow clone) |
| `{{COMMAND}} plugin update <folder>` | Pulls its latest commits (`git pull --ff-only`) |
| `{{COMMAND}} plugin remove <folder>` | Deletes its folder |

- The folder's name comes from the source (`…/acme-tools.git` gives `acme-tools`) unless you pass `--name`. It may use letters, digits, `-`, `_` and `.`, and may not start with `.`.
- Installing over an existing folder is refused: update or remove it first.
- `update` works only on a plugin that was cloned with git.
- These commands need `git` on your `PATH`.

You can also copy or symlink a folder into `~/{{DIR}}/plugins/` by hand: every folder there is a plugin.

Changes are picked up by the next prompt, a new chat, or `/reload`.

> [!WARNING]
> A plugin's hooks and MCP servers run programs on your computer, and its MCP servers start without asking for approval. Install plugins only from sources you trust, and read their `hooks/hooks.json` and `.mcp.json`.

## Plugin layout

```text
acme-tools/
├── {{PLUGIN_DIR}}/
│   └── plugin.json          the manifest
├── skills/
│   └── triage/SKILL.md      → /acme:triage
├── commands/
│   └── hello.md             → /acme:hello
├── agents/
│   └── reviewer.md          → acme:reviewer
├── hooks/
│   └── hooks.json           hooks
├── .mcp.json                MCP servers
├── .lsp.json                language servers
└── bin/                     anything else the plugin needs
```

Every part is optional.

### The manifest

`{{PLUGIN_DIR}}/plugin.json`:

```json
{
  "name": "acme",
  "description": "ACME's triage skills, reviewer agent and Jira server"
}
```

| Field | Meaning |
|---|---|
| `name` | The plugin's name, used as the namespace of everything it brings. Defaults to the folder's name. |
| `description` | Shown by `{{COMMAND}} plugin list` |
| `mcpServers` | MCP servers, used when the plugin has no `.mcp.json` |
| `lspServers` | Language servers, used when the plugin has no `.lsp.json` |

When two plugins have the same name, only the first is used.

## What a plugin can contribute

| Part | Where | Named |
|---|---|---|
| Skills | `skills/<name>/SKILL.md` (or `SKILL.js`) | `<plugin>:<name>` |
| Custom commands | `commands/**/*.md` (or `.js`) | `<plugin>:<path>`, such as `acme:db:migrate` |
| Agents | `agents/*.md` | `<plugin>:<name>` |
| Hooks | `hooks/hooks.json` | — |
| MCP servers | `.mcp.json`, or `mcpServers` in the manifest | `plugin_<plugin>_<server>` |
| Language servers | `.lsp.json`, or `lspServers` in the manifest | — |

Skills, commands and agents follow the same formats as your own (see [Skills](./skills) and [Agents](./agents)). Your own and the project's take precedence over a plugin's when names clash, but the namespace keeps that from happening by accident.

### Hooks

`hooks/hooks.json` holds the same events as `"hooks"` in settings, either at the top level or under a `"hooks"` key:

```json
{
  "description": "ACME checks",
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "^Bash$",
        "hooks": [{ "type": "command", "command": "${{{ENV}}_PLUGIN_ROOT}/bin/check.sh" }]
      }
    ]
  }
}
```

A broken `hooks.json` (invalid JSON, an unknown event…) is left out with a warning in the log; the rest of the plugin still works. See [Hooks](./hooks).

### MCP servers

```json
{
  "mcpServers": {
    "jira": {
      "command": "${{{ENV}}_PLUGIN_ROOT}/bin/jira-mcp",
      "args": ["--root", "${{{ENV}}_PLUGIN_ROOT}"]
    }
  }
}
```

The server above is named `plugin_acme_jira` (characters other than letters, digits and `-` become `_`), so its tools are `mcp__plugin_acme_jira__<tool>`. A plugin server named `{{COMMAND}}_…` is ignored. Plugin servers are `ready` as soon as the plugin is installed. Unlike your own `mcp.json`, a plugin's file does not read other environment variables: only `${{{ENV}}_PLUGIN_ROOT}` is filled in. See [MCP servers](./mcp).

### Language servers

`.lsp.json` maps a server name to its configuration, the same shape as `lspServers` in settings:

```json
{
  "zls": {
    "command": "zls",
    "extensionToLanguage": { ".zig": "zig" }
  }
}
```

A broken set is left out with a warning. See [Code intelligence](./code-intelligence).

### `${{{ENV}}_PLUGIN_ROOT}`

Write `${{{ENV}}_PLUGIN_ROOT}` anywhere in `hooks/hooks.json`, `.mcp.json`, `.lsp.json` or the manifest's `mcpServers` and `lspServers`: it is replaced by the plugin's folder before use, so a plugin works wherever it is installed.

## Writing a plugin

1. Make a folder with `{{PLUGIN_DIR}}/plugin.json` and a `name`.
2. Add the parts you need.
3. Link it into `~/{{DIR}}/plugins/` to try it:

   ```bash
   ln -s "$PWD" ~/{{DIR}}/plugins/acme-tools
   ```

4. `/reload` in a chat, then check `/skills`, `/agents`, `/mcp` and `/hooks`.
5. Push it to a git repository; others install it with `{{COMMAND}} plugin install <url>`.

## See also

- [Skills](./skills), [Agents](./agents), [Hooks](./hooks), [MCP servers](./mcp).
- [Files and directories](./files-and-directories).
