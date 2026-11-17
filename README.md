# Cortex

A coding agent you talk to in the terminal: a chat (CLI/TUI) with agents,
commands, skills, memory, plugins and MCP servers (HTTP and stdio).

## Installing

From source:

```sh
cargo run -p xtask -- web          # builds the web page into web/dist; needs Bun
cargo install --path crates/cli    # embeds web/dist as it is at that moment
```

Building needs a Rust toolchain (1.97 or newer) and a C compiler. Cargo
never runs Bun: the web page (`/trace`, `/console`, `/docs`) is built by the
first command, which needs [Bun](https://bun.sh) and fails without it. Skip
it and everything else still builds and works; the binary then carries no
page, and `/trace` opens one that says how to build it. Nothing else is
needed to run it. Some tools are used when present:

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

The list of sessions and each project's memory are in `app.db` there, a
SQLite database (with `app.db-wal` and `app.db-shm` beside it while the
program runs), so several running instances share them without losing
each other's changes. Conversations stay as `.jsonl` files and settings
as JSON.

Upgrading from a version that kept them as JSON: the first start imports
`users/local/sessions.json` and `memory/local/<project>.json` into
`app.db` and renames each file with `.imported`; nothing is deleted.
Downgrading afterwards: the older binary finds only the `.imported`
files and shows an empty session list and empty memory until you quit it
and rename them back (drop `.imported`). What was saved after the import
is only in `app.db`, which the older binary does not read.

## Layout

| Crate | What it is |
|---|---|
| `domain` | Domain types and rules: requests, engine events, memory. Pure. |
| `agent-engine` | The agent loop, tools, permissions, hooks, sub-agents, skill/agent discovery, MCP, sessions, compaction and usage metering. It reaches models through `llm-providers`. |
| `app` | The application API, in process: the CLI's client (sessions, runs), `ClientCommand` / `ServerEvent`, and the command handler behind them: runs, memory, settings, providers, secrets. |
| `llm-providers` | Talking to a model: the `Provider` trait, one folder per provider, the model client that sends requests over HTTP and reads the streams, the wires, the transcript types, credentials, OAuth, sign-in, catalogs, declared providers and model-family vendor folders. The subscription providers are a Cargo feature, `subscription-providers`, on by default. |
| `cli` | The `cortex` binary: terminal chat (ratatui), `-p`, `login`, `models`, `mcp`, `config`, `doctor`. |
| `brand` | The product's name and command, from `brand.toml`. |
| `language-servers` | Runs language servers for the `LSP` tool and edit diagnostics: client, pool, first-use installs. |
| `code-map` | Experimental tree-sitter index of a repository's functions, types and callers, ranked against a prompt. |
| `platform` | Per-OS IO: child processes and their trees, private files, stored documents (lock, atomic replace, corrupt set aside), data folders. |
| `content` | Prompts, built-in skills and agents, embedded at build time. |
| `script` | The QuickJS runtime for JavaScript tools, skills and providers. |
| `console` | The live trace page behind `/trace` and `/console`: serves the browser app on localhost (whatever `cargo run -p xtask -- web` last built into `web/dist`, embedded in the binary at compile time) with the report, the sessions and their updates as server-sent events. |

## Developing

Read [CLAUDE.md](CLAUDE.md): working rules, how the crates depend on each
other, and how to run Cargo.

```sh
export CARGO_TARGET_DIR="$PWD/target/agents/<task>"
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -q -p xtask -- lint
```

None of these needs Bun or a built page. The web page in `web/` is built
and checked by its own steps ([web/README.md](web/README.md) has the
details):

```sh
cargo run -p xtask -- web             # bun install --frozen-lockfile + bun run build, into web/dist
cargo run -p xtask -- check-release   # before a release: fails if web/dist is missing or older than its sources
cargo run -p xtask -- web-fixtures    # after changing a trace type: rewrites web/test/fixtures from the Rust types
(cd web && bun test)                  # the page's tests, including the fixtures against its types
```

`cargo build` embeds `web/dist` when it exists and nothing when it does
not; it never starts Bun or touches the network for the page.
