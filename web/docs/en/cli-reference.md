---
title: CLI reference
description: Every command, subcommand and option of the {{COMMAND}} program.
---

`{{COMMAND}}` opens the chat in the current folder. With a prompt, the chat starts with it; with `--print`, it answers once and exits; with a subcommand, it does that one thing.

```bash
{{COMMAND}} [OPTIONS] [PROMPT]...
{{COMMAND}} [OPTIONS] <COMMAND>
```

## Common uses

```bash
{{COMMAND}}                                   # chat in the current project
{{COMMAND}} "why does the build fail?"        # chat, starting with this prompt
{{COMMAND}} -p "explain src/main.rs"          # one answer, printed
{{COMMAND}} -c                                # go on with the last conversation here
{{COMMAND}} -r                                # pick an earlier conversation of this folder
{{COMMAND}} -m claude-opus-5-5 --mode plan    # start with this model, in plan mode
{{COMMAND}} -C ../other-project               # work in another folder
{{COMMAND}} login                             # connect a provider
{{COMMAND}} models                            # list the models
{{COMMAND}} doctor                            # check this installation here
```

## Subcommands

| Command | What it does |
|---|---|
| `{{COMMAND}} login` | Connect an LLM provider with an API key or a subscription; it lists the providers to pick from |
| `{{COMMAND}} logout` | Forget a provider's credentials |
| `{{COMMAND}} models` | List the models the connected providers offer |
| `{{COMMAND}} mcp` | The MCP servers a session here can use: list, get, add, remove, approve, deny, login, logout |
| `{{COMMAND}} plugin` | The plugins in `~/{{DIR}}/plugins`: list, install, update, remove |
| `{{COMMAND}} remote` | Remote control from a phone, through your relay: status, relay, pair, devices, revoke |
| `{{COMMAND}} doctor` | Check that this installation can work in the folder |
| `{{COMMAND}} eval` | Run the evals in `{{DIR}}/evals` and score them |
| `{{COMMAND}} teammate` | Work as a teammate of a lead's team, from its mailbox; started by the lead, not by hand (see [Agent teams](./agents#agent-teams)) |
| `{{COMMAND}} config` | Read or change the settings: yours, or this project's with `--project` |
| `{{COMMAND}} help` | Print the help, or a subcommand's help |

The global options (`--cwd`, `--model` and the rest) go before the subcommand: `{{COMMAND}} -C ../app doctor`.

### login

```bash
{{COMMAND}} login
```

Prints the providers, each with how it connects (`paste an API key`, `sign in with the browser`, `sign in with a code`), and asks `Choose 1-<n>:`. A key is typed hidden; a sign-in opens the browser. It ends with `Connected <provider> (<account>).` Connecting a provider that is already connected adds another account. See [Models and providers](./models-and-providers) and [Accounts](./accounts).

### logout

```bash
{{COMMAND}} logout
```

Lists the connected providers and asks `Forget which?`. The provider is forgotten with all of its accounts. With nothing connected, it prints `No provider is connected.`

### models

```bash
{{COMMAND}} models
```

Prints one line per model: its id, padded to 40 characters, then its label.

### mcp

```bash
{{COMMAND}} mcp [COMMAND]
```

Without a command, it lists the servers. Every command prints the server list afterwards: the name, where it comes from (`host`, `project`, `user` or `plugin <name>`), its state (`ready`, `waiting for approval`, `off` or `skipped: folder not trusted`), and its URL or command. See [MCP](./mcp).

| Command | Arguments | What it does |
|---|---|---|
| `mcp list` | | List the servers, where they come from and whether they start |
| `mcp get` | `<NAME>` | Show one server's entry, with its secrets hidden |
| `mcp add` | `<NAME> <TARGET> [ARGS]...` | Add one of your own servers: a URL (HTTP) or a command and its arguments |
| `mcp remove` | `<NAME>` | Remove one of your own servers |
| `mcp approve` | `<NAME>` | Let a server of this project's `{{DIR}}/mcp.json` start |
| `mcp deny` | `<NAME>` | Turn a server of this project's `{{DIR}}/mcp.json` off |
| `mcp login` | `<NAME>` | Sign in to a remote server in the browser (OAuth) |
| `mcp logout` | `<NAME>` | Forget the sign-in to a remote server |

`mcp add` arguments and options:

| Argument or option | Value | Default | Description |
|---|---|---|---|
| `<NAME>` | text | | The server's name: letters, digits, `-` and `_` |
| `<TARGET>` | text | | A URL starting with `https://` or `http://` for an HTTP server; anything else is the command to start |
| `[ARGS]...` | text | | The command's arguments; write `--` before them when one starts with `-` |
| `--force` | | | Replace a server of yours that has this name |
| `-e`, `--env` | `KEY=VALUE` | | An environment variable for a command server (repeatable). `${VAR}` is read when the server starts. |
| `-H`, `--header` | `"Name: value"` | | A header for an HTTP server (repeatable) |
| `--transport` | `http` or `sse` | `http` | `sse` for a server on the older HTTP+SSE transport |

The options may come anywhere before `--`; everything after `--` is the server's own:

```bash
{{COMMAND}} mcp add -H "Authorization: Bearer $TOKEN" docs https://mcp.example.com/mcp
{{COMMAND}} mcp add --transport sse legacy https://old.example.com/sse
{{COMMAND}} mcp add -e API_KEY='${API_KEY}' files -- npx -y @example/files-mcp --root .
{{COMMAND}} mcp get files
{{COMMAND}} mcp approve jira
```

### plugin

```bash
{{COMMAND}} plugin [COMMAND]
```

Without a command, it lists the installed plugins. See [Plugins](./plugins).

| Command | Arguments and options | What it does |
|---|---|---|
| `plugin list` | | List the installed plugins: name and description |
| `plugin install` | `<SOURCE>` `[--name <NAME>]` | Install a plugin from a git URL or a path. `--name` sets the folder's name; otherwise it is taken from the source. |
| `plugin update` | `<NAME>` | Pull a plugin's latest commits |
| `plugin remove` | `<NAME>` | Remove a plugin's folder |

After an install, a new chat or `/reload` picks the plugin up.

```bash
{{COMMAND}} plugin install https://github.com/example/{{COMMAND}}-plugin.git
{{COMMAND}} plugin install ./my-plugin --name mine
{{COMMAND}} plugin update mine
```

### remote

```bash
{{COMMAND}} remote [COMMAND]
```

Follow and drive this machine's sessions from a phone: the same page as `/console` (sessions, runs, the terminal), installed on the phone's home screen. The phone reaches the machine through a relay you run yourself (the `relay` program from this repository, behind HTTPS); the machine connects out to it and opens no port. Everything between the phone and the machine is end-to-end encrypted: the relay forwards bytes it cannot read. Without a command, it shows the relay, this machine's id and the paired devices.

| Command | Arguments | What it does |
|---|---|---|
| `remote status` | | The relay, this machine's id and the paired devices |
| `remote up` | `[--account <ID>]` | Publish the relay as a Worker on your Cloudflare account (workers.dev) and use it |
| `remote logout` | | Forget the Cloudflare sign-in `remote up` keeps |
| `remote relay` | `<https://HOST>` | Name a relay you run elsewhere, which phones reach this machine through |
| `remote pair` | | Print a QR code and a link that pair one phone, within ten minutes, once |
| `remote devices` | | List the paired devices |
| `remote revoke` | `<NAME>` | Forget a device; its open connections close within seconds |

The machine is reachable while a chat is open with remote control on (`/remote`); it starts by itself once a relay is named and a phone is paired. On the phone, open the link, let the page connect, then use **Add to Home Screen**. A paired phone can open the terminal, which runs commands as you: revoke a phone you lose.

```bash
{{COMMAND}} remote relay https://relay.example.com
{{COMMAND}} remote pair
{{COMMAND}} remote revoke "iPhone"
```

#### Without a server: `remote up`

`{{COMMAND}} remote up` publishes the relay as a Worker on your own Cloudflare account, through Cloudflare's API, and makes it this machine's relay. It needs no server, no domain and nothing installed: the relay gets a fixed address, `https://{{COMMAND}}-relay.<your subdomain>.workers.dev`, and keeps running on Cloudflare; nothing stays open on this machine but the chat. It fits in Cloudflare's free plan (a Worker and one Durable Object per machine).

The first time, it opens the browser to sign in to Cloudflare: choose the account and allow it. The sign-in is kept in {{NAME}}'s credentials vault and renewed by itself, so the next `remote up` does not ask again; `{{COMMAND}} remote logout` forgets it. On a machine without a browser, set `CLOUDFLARE_API_TOKEN` to an API token with the permission **Account · Workers Scripts · Edit** instead. If the sign-in reaches more than one account, name one with `--account <id>`. An account that never opened Workers & Pages has no workers.dev subdomain yet: open it once in the dashboard. Run `remote up` again after updating {{NAME}} to publish the new page; paired phones keep working.

Cloudflare carries the traffic and serves the page to the phone, but what passes between the phone and the machine stays end-to-end encrypted.

```bash
{{COMMAND}} remote up
CLOUDFLARE_API_TOKEN=… {{COMMAND}} remote up --account 0123abcd…
```

### doctor

```bash
{{COMMAND}} doctor
```

Runs the installation checks for the folder and exits with 0 when they all pass, 1 otherwise. The `trust` line says whether the folder is trusted and how many project items are skipped while it is not; an untrusted folder is not a failed check. See [Troubleshooting](./troubleshooting#run-the-doctor).

### eval

```bash
{{COMMAND}} eval [OPTIONS] [NAMES]...
```

| Argument or option | Description |
|---|---|
| `[NAMES]...` | Only these cases (all by default) |
| `--json` | Print the scores as JSON |

Each case is `{{DIR}}/evals/<name>/case.json` with an optional `fixture/`. `--model`, given before `eval`, is used for every case. Exits with 0 when every case passed, 1 otherwise. See [Evals](./evals).

### config

```bash
{{COMMAND}} config [--project] <COMMAND>
```

Reads or changes your settings file, `~/{{DIR}}/settings.json`. With `--project`, it works on this project's `{{DIR}}/settings.local.json` instead. `--project` can go before or after the command. See [Settings](./settings).

| Command | Arguments | What it does |
|---|---|---|
| `config list` | | Print every setting, as JSON |
| `config get` | `<KEY>` | Print one setting. The key is a dotted path like `permissions.allow`. |
| `config set` | `<KEY> <VALUE>` | Set one. The value is read as JSON (`true`, `["Bash(ls:*)"]`), or else as text. |
| `config unset` | `<KEY>` | Remove one |

```bash
{{COMMAND}} config set permissions.allow '["Bash(ls:*)"]'
{{COMMAND}} config set outputStyle concise
{{COMMAND}} config get permissions.allow
{{COMMAND}} config --project set permissions.allow '["Bash(cargo test:*)"]'
{{COMMAND}} config unset outputStyle
```

`set` and `unset` create the file and its folder when needed and print `Set <key> in <file>.` or `Removed <key> from <file>.` `get` of a key that is not set fails with `<key> is not set in <file>`.

## Options

### Prompt and output

| Option | Short | Value | Default | Requires | Description |
|---|---|---|---|---|---|
| `[PROMPT]...` | | text | | | What to ask. The words are joined with spaces. It opens the chat with it unless `--print` is given. |
| `--print` | `-p` | | off | | Answer once, print the reply and exit. With `--input-format text` it needs a prompt (`--print needs a prompt`). See [Headless mode](./headless). |
| `--output-format` | | `text`, `json` or `stream-json` | `text` | `--print` | `text` streams the answer; `json` prints one result object; `stream-json` prints every event as a JSON line, then the result |
| `--input-format` | | `text` or `stream-json` | `text` | `--print` | `text` takes the prompt from the command line; `stream-json` reads user messages as JSON lines on standard input and answers them in turn |
| `--max-turns` | | number | 200 | `--print` | The most model turns a run may take |
| `--verification` | | text | | `--print` | Judge the work against these criteria before it ends; the command exits with 1 when it fails them |
| `--trust` | | | off | `--print` | Trust the project folder for this invocation only, so its hooks, scripts, MCP servers and local settings load. Nothing is stored. Without it a `--print` run is untrusted and one line on stderr says what was skipped. In your home folder it is ignored, with a line on stderr saying so. See [Workspace trust](./sandbox-and-safety#workspace-trust). |

### Model and behaviour

| Option | Short | Value | Default | Description |
|---|---|---|---|---|
| `--model` | `-m` | model id or name | the last one used | The model. The full id, the part after the colon, or the label. |
| `--fallback-model` | | model id or name | | The model to go on with when the chosen one is overloaded. It takes precedence over the fallbacks set in the chat and in the settings. |
| `--mode` | | `ask`, `edits`, `plan` or `bypass` | | The permission mode. `default` is also read as `ask`, `acceptEdits` and `accept-edits` as `edits`, `bypassPermissions`, `bypass-permissions` and `yolo` as `bypass`, in any letter case. |
| `--allowedTools` | | rules | | Rules allowed without asking, like `Bash(git diff:*) Edit`. Repeatable. Also `--allowed-tools`. |
| `--disallowedTools` | | rules | | Rules refused, like `Bash(rm:*)`. Repeatable. Also `--disallowed-tools`. |
| `--append-system-prompt` | | text | | Text added at the end of the system prompt |

Rules in `--allowedTools` and `--disallowedTools` are separated by spaces or commas outside parentheses. See [Permissions](./permissions).

### Conversations

| Option | Short | Value | Conflicts with | Description |
|---|---|---|---|---|
| `--continue` | `-c` | | `--resume` | Go on with the last conversation in this folder. If there is none, the chat says `no earlier conversation in this folder; starting a new one`. |
| `--resume` | `-r` | optional id | `--continue` | Go on with an earlier conversation: its id, or, without an id, pick one of this folder in the chat |
| `--fork-session` | | | | With `-c` or `-r`: go on in a copy of that conversation, leaving it as it is |

See [Sessions](./sessions).

### Folders and terminal

| Option | Short | Value | Default | Description |
|---|---|---|---|---|
| `--cwd` | `-C` | path | the current folder | The project folder |
| `--add-dir` | | path | | Let the agent read this folder like the project. Repeatable. Relative paths start at the project folder; `~/` starts at your home folder. The folder must exist. |
| `--inline` | | | off | Chat at the bottom of the terminal, with the conversation in the terminal's own scrollback, instead of full screen |
| `--channels` | | names, comma-separated | | MCP servers to keep up as channels: what they send (a chat's messages, a webhook) reaches the agent as an event. See [Channels](./channels). |

### Help and version

| Option | Short | Description |
|---|---|---|
| `--help` | `-h` | Print the help. `{{COMMAND}} <command> --help` prints a subcommand's help. |
| `--version` | `-V` | Print the version |

## Exit codes

| Code | When |
|---|---|
| `0` | Success |
| `1` | A failure: a failed run, a failed verification or eval, a failed doctor check, or an error such as an unreadable folder. The message is printed on standard error as `{{COMMAND}}: <message>`. |
| `2` | The command line is wrong: an unknown option or value, a missing argument, or an option used without the one it requires. Also when standard input or output is not a terminal and `--print` is not given (`no terminal: use -p for print mode`). |

## Environment variables

| Variable | Effect |
|---|---|
| `{{ENV}}_LOG` | What goes to the log file, as a filter like `warn` or `app=debug`. Default `warn`. See [Troubleshooting](./troubleshooting#logs). |
| `{{ENV}}_THEME` | `light` or `dark`: the chat's colours, instead of following the terminal |
| `{{ENV}}_REDUCED_MOTION` | `1` turns the chat's animations off |

See [Environment variables](./environment-variables) for the full list.
