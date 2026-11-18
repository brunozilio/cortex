---
title: Environment variables
description: The environment variables {{NAME}} reads, the standard ones it honours, and the ones it sets for the programs it starts.
---

Most of {{NAME}} is configured in [settings files](./settings). Environment variables cover what has to be known before those files are read (logging, colours, where data lives) and a few switches that are handy in scripts and CI.

```bash
{{ENV}}_LOG=debug {{COMMAND}}
{{ENV}}_THEME=light {{ENV}}_REDUCED_MOTION=1 {{COMMAND}}
{{ENV}}_STREAM_IDLE_SECONDS=600 {{COMMAND}} -p "summarise the open TODOs"
```

## {{NAME}} variables

| Variable | Values | Default | Purpose |
|---|---|---|---|
| `{{ENV}}_LOG` | A log filter: `error`, `warn`, `info`, `debug`, `trace`, or per module (`agent_engine=debug,warn`) | `warn` | How much goes to the log file `logs/{{COMMAND}}.log` in the [local data folder](./files-and-directories#data-folder). An empty or invalid filter falls back to `warn`. |
| `{{ENV}}_THEME` | `light`, `dark` (any case) | detected | Forces the chat's light or dark colours instead of detecting the terminal's background. |
| `{{ENV}}_REDUCED_MOTION` | Any non-empty value other than `0` | unset | Turns the chat's animations off. |
| `{{ENV}}_CONSOLE_DIR` | A folder containing `index.html` | unset | Serves the [trace console](./trace-console) page from this folder instead of the copy built into the binary. |
| `{{ENV}}_STREAM_IDLE_SECONDS` | A whole number of seconds above zero | `300` | How long a model's answer may stay silent mid-stream before the request is given up and treated as failed. Anything else falls back to the default. |
| `{{ENV}}_OTLP_ENDPOINT` | An `http://` or `https://` URL, such as `http://localhost:4318` | unset: nothing is exported | Sends each finished run to this OpenTelemetry collector as OTLP/HTTP JSON, at `<endpoint>/v1/traces`. See [Export to OpenTelemetry](./trace-console#export-to-opentelemetry). |
| `{{ENV}}_OTLP_HEADERS` | `name=value` pairs separated by commas | unset | Headers added to each export request, such as an API key. Read only when `{{ENV}}_OTLP_ENDPOINT` is set. |
| `{{ENV}}_TEAM_NAME` | A team's name | set by the lead | The team a teammate belongs to. A lead sets it on each teammate it starts; see [Agent teams](./agents#agent-teams). |
| `{{ENV}}_AGENT_NAME` | A teammate's name | set by the lead | The teammate's own name in its team, set with `{{ENV}}_TEAM_NAME`. |
| `{{ENV}}_TEAMMATE_COMMAND` | A program path | unset: the running `{{COMMAND}}` | The program a lead starts teammates with, as `<program> … teammate`. |
| `{{ENV}}_SECRETS` | `keychain` (any case) | unset: the key stays in `secrets.key` | With `keychain`, the key that opens your saved API keys and tokens moves from `secrets.key` into the system keychain. Unset (or any other value), it stays in the file and the keychain is never written; a rebuilt or reinstalled binary has a new signature, and macOS would ask for the keychain password at every start. If an earlier version moved the key into the keychain, it is taken back into the file once (macOS asks once); if the keychain does not answer then, a new key is made and keys saved before must be entered again. With `keychain`, a keychain that stops answering is an error, not a new key; set `{{ENV}}_SECRETS=file` to leave it. See [Files and directories](./files-and-directories#data-folder). |

## Remote control variables

| Variable | Read by | Purpose |
|---|---|---|
| `CLOUDFLARE_API_TOKEN` | `{{COMMAND}} remote up` | An API token with the permission **Account · Workers Scripts · Edit**, used instead of signing in to Cloudflare in the browser (for a machine without one). See [Remote control](./remote-control). |
| `RELAY_LISTEN` | the `relay` program | The address and port it listens on, such as `127.0.0.1:8080`; `relay --listen` does the same. The default is `0.0.0.0:8080`. See [Run your own relay](./remote-control#run-your-own-relay). |

## Standard variables it honours

| Variable | Used for |
|---|---|
| `HOME` | Your home folder: `~/{{DIR}}` (settings, skills, agents, plugins…), `~/` in rules and paths, and the `~` shown for the project in the welcome screen. On Windows the user profile folder is used. |
| `XDG_CONFIG_HOME` | Linux: the [data folder](./files-and-directories#data-folder) is `$XDG_CONFIG_HOME/{{COMMAND}}`; `~/.config/{{COMMAND}}` when unset or empty. |
| `APPDATA` | Windows: the data folder is `%APPDATA%\{{NAME}}`; `~\AppData\Roaming\{{NAME}}` when unset. |
| `LOCALAPPDATA` | Windows: the local data folder (sessions, logs, keys) is `%LOCALAPPDATA%\{{NAME}}`; `~\AppData\Local\{{NAME}}` when unset. Also where Git Bash is looked for (`Programs\Git\bin\bash.exe`). |
| `ProgramFiles` | Windows: where Git Bash is looked for (`Git\bin\bash.exe`). Without Git Bash, commands run in Windows PowerShell. |
| `SHELL` | macOS and Linux: the shell that runs the agent's commands (`$SHELL -c …`); `/bin/sh` when unset or empty. Hooks and scripts always use `/bin/sh`. |
| `VISUAL`, `EDITOR` | The editor Ctrl+G opens to write the prompt, and the one `/memory edit` opens the project's `{{INSTRUCTIONS}}` in. `VISUAL` first, then `EDITOR`, then `vi`. The value may include arguments (`code --wait`). |
| `COLORFGBG` | Detects a light or dark terminal background when `{{ENV}}_THEME` is not set; dark when it is missing. |
| `COLORTERM` | `truecolor` or `24bit` turns on true colour in the chat. |
| `PATH` | Where language servers (`rust-analyzer`, `gopls`…) and other programs are found. |
| `TMPDIR` | macOS and Linux: the [temporary folder](./files-and-directories#temporary-files) for long tool outputs, the prompt being edited and sandbox write access. |
| `HTTPS_PROXY`, `HTTP_PROXY`, `ALL_PROXY` (or lower case) | Requests to providers and web pages go through this proxy. With one of them set, the agent's web requests leave to the proxy the check that a site is not on this computer or your private network. |

The `${VAR}` and `${VAR:-default}` forms in `mcp.json` files are filled in from the environment; a variable that is unset and has no default is left as written. See [MCP](./mcp).

HTTP [hooks](./hooks) can use `$VAR` or `${VAR}` in their `headers`, but only for the variables listed in the hook's `allowed_env_vars`; others become empty.

> [!NOTE]
> Provider keys and tokens are not read from the environment. Connect providers with `{{COMMAND}} login` (or `/login`); credentials are sealed in the data folder. See [Accounts](./accounts).

## Variables it sets for other programs

| Variable | Set for | Value |
|---|---|---|
| `{{ENV}}_PROJECT_DIR` | Command hooks | The project folder the run works in. |
| `{{ENV}}_PLUGIN_ROOT` | Plugin configuration | Not an environment variable: `${{{ENV}}_PLUGIN_ROOT}` in a plugin's MCP servers, language servers and hooks is replaced with the plugin's folder. See [Plugins](./plugins). |
| `NO_COLOR=1`, `CLICOLOR=0`, `CARGO_TERM_COLOR=never`, `CARGO_TERM_PROGRESS_WHEN=never`, `PY_COLORS=0` | The agent's shell commands | Only with the `experimental-output-cleanup` feature on (the default), so output arrives without colours or progress bars. |
| `CARGO_TARGET_DIR` | Background test runs in Rust projects | `<project>/target/agent-tests`, so they do not block your own builds. Feature `experimental-affected-tests`. |
| `GOBIN` | Installing a Go language server | A staging folder inside the language server cache. |
| `TERM=xterm-256color`, `COLORTERM=truecolor` | The terminal the trace console opens | Fixed values. |
| `GIT_TERMINAL_PROMPT=0`, `GIT_OPTIONAL_LOCKS=0` | Git commands {{NAME}} runs on its own (status, diffs) | So git never asks for a password and never takes a lock your own git commands would wait on. |
| `APP_NOTIFICATION_TITLE`, `APP_NOTIFICATION_MESSAGE` | Windows desktop notifications | The notification's title and text. |
| Variables from `env` | MCP servers (stdio) and language servers | What you put in the server's `env` in `mcp.json` or `lspServers`. |

On Windows with Git Bash, `BASH_ENV` and `ENV` are removed from the environment of the agent's commands, so no startup profile runs before them.

Everything else in your environment is passed on unchanged to the commands, hooks and servers {{NAME}} starts.
