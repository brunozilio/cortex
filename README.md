# Cortex

A coding agent you talk to in the terminal: a chat (CLI/TUI) with agents,
commands, skills, memory, plugins and MCP servers (HTTP and stdio).

## Installing

From source:

```sh
cargo install --path crates/cli
```

Building needs a Rust toolchain (1.97 or newer) and a C compiler. Bun is
needed only to build the web page (`/trace`, `/console`, `/docs`); without
it the build only warns and the page is not embedded in the binary. Nothing
else is needed to run it. Some tools are used when present:

- `git`: `/diff` and worktrees.
- `pdftotext`: reading PDFs.
- `ripgrep`: optional; `Grep` works without it.
- Language servers (rust-analyzer, clangd…): the `LSP` tool and
  diagnostics after edits.
- `node`/`npx`: only for MCP servers that need them.

## Using it

```sh
cortex login                             # API key or subscription; lists the providers
cortex logout                            # forget a provider
cortex                                   # chat in the current project
cortex -p "explain src/main.rs"          # one answer, printed
cortex models                            # what the connected providers offer
cortex mcp                               # MCP servers: list, add, remove, approve, deny, login, logout
cortex plugin                            # plugins: list, install, update, remove
cortex config                            # read or change the settings: list, get, set, unset
cortex doctor                            # check this installation in this folder
cortex eval                              # run .cortex/evals/<name>/case.json and score them
```

In the chat: Enter sends, Alt+Enter (or Ctrl+J) adds a line, Esc
interrupts, Shift+Tab cycles ask → edits → plan → bypass, Ctrl+V attaches
the clipboard's image (or drop an image file on the terminal; it shows as
`[Image #1]`), Ctrl+O unfolds reasoning, outputs and diffs. `/help` lists
the commands (`/model`, `/mode`, `/effort`, `/compact`, `/diff`, `/memory`,
`/skills`, `/agents`, `/clear`, `/cost`, `/exit`); any other `/name` runs a
skill or custom command. Replies render as Markdown with highlighted code;
`CORTEX_REDUCED_MOTION=1` turns the animations off. Colours follow the
terminal's background (from `COLORFGBG`, or `CORTEX_THEME=light|dark`) and
use true colour where `COLORTERM` says so.

Project settings, skills, agents and rules live in `.cortex/`, project
instructions in `CORTEX.md` or `.cortex/CORTEX.md`, and your own in
`~/.cortex/`. Plugins live in `~/.cortex/plugins/` (`cortex plugin
install <git-url>`, `list`, `update`, `remove`); `/reload` in the chat
lists what was found.

## Environment variables

- `CORTEX_LOG`: log filter for the log file (`logs/cortex.log` in the data folder); the default is `warn`.
- `CORTEX_THEME`: `light` or `dark`; overrides the background detected from `COLORFGBG`.
- `CORTEX_REDUCED_MOTION`: any value other than empty or `0` turns the animations off.
- `CORTEX_CONSOLE_DIR`: a folder holding an already built web page (with an `index.html`) to serve instead of the embedded one.
- `CORTEX_DISABLE_AUTO_MEMORY`: `1`, `true` or `yes` turns off the persistent memory of custom agents.

Also read: `SHELL`, `VISUAL`, `EDITOR`, `HOME`, `COLORFGBG`, `COLORTERM`
and `XDG_CONFIG_HOME`.

## Docs

- [docs/vision.md](docs/vision.md): what it is for, scope and non-goals (draft).
- [docs/GLOSSARY.md](docs/GLOSSARY.md): the project's vocabulary.
- [docs/architecture.md](docs/architecture.md): crates, request path, data on disk, threading.
- [docs/decisions/](docs/decisions/): the decision records.
- [docs/threat-model.md](docs/threat-model.md): assets, actors, trust boundaries, known gaps.

## Where data lives

`~/Library/Application Support/Cortex` on macOS, `~/.config/cortex` on
Linux: sessions, memory and settings. Keys and tokens are sealed in
`secrets.json` there (its key in `secrets.key`, both readable only by
you).

## Layout

| Crate | What it is |
|---|---|
| `domain` | Domain types and rules: requests, engine events, memory. Pure. |
| `agent-engine` | The agent loop, tools, permissions, hooks, sub-agents, skill/agent discovery, MCP, sessions, compaction, wires and one folder per provider. |
| `protocol` | `ClientCommand` / `ServerEvent` in an `Envelope`. |
| `backend` | Command handler: runs, memory, settings, providers, secrets. |
| `llm-providers` | What providers share: the `Provider` trait, credentials, OAuth, sign-in, catalogs, declared providers and model-family vendor folders. Pure. |
| `sdk` | The CLI's internal client to the backend. |
| `cli` | The `cortex` binary: terminal chat (ratatui), `-p`, `login`, `models`, `mcp`, `config`, `doctor`. |
| `brand` | The product's name and command, from `brand.toml`. |
| `language-servers` | Runs language servers for the `LSP` tool and edit diagnostics: client, pool, first-use installs. |
| `code-map` | Experimental tree-sitter index of a repository's functions, types and callers, ranked against a prompt. |
| `platform` | Per-OS IO: child processes and their trees, private files, data folders. |
| `content` | Prompts, built-in skills and agents, embedded at build time. |
| `script` | The QuickJS runtime for JavaScript tools, skills and providers. |
| `console` | The live trace page behind `/trace` and `/console`: serves the browser app on localhost (built from `web/` into the binary when Bun is present, else `web/dist`) with the report, the sessions and their updates as server-sent events. |

## Developing

Read [CLAUDE.md](CLAUDE.md): working rules, how the crates depend on each
other, and how to run Cargo.

```sh
export CARGO_TARGET_DIR="$PWD/target/agents/<task>"
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -q -p xtask -- lint
```
