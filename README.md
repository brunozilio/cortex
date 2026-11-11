# Cortex

A coding agent you talk to in the terminal: a chat (CLI/TUI) with agents,
commands, skills, memory, plugins and MCP servers (HTTP and stdio).

## Installing

From source:

```sh
cargo install --path crates/cli
```

Nothing else is required. Some tools are used when present:

- `git`: `/diff` and worktrees.
- `pdftotext`: reading PDFs.
- `ripgrep`: optional; `Grep` works without it.
- Language servers (rust-analyzer, clangd…): the `LSP` tool and
  diagnostics after edits.
- `node`/`npx`: only for MCP servers that need them.

## Using it

```sh
cortex login                             # API key, or Claude/ChatGPT/Copilot/Kiro subscription
cortex                                   # chat in the current project
cortex -p "explain src/main.rs"          # one answer, printed
cortex models                            # what the connected providers offer
cortex mcp                               # MCP servers: list, add, remove, approve
cortex config                            # read or change the settings
cortex doctor                            # check this installation in this folder
```

In the chat: Enter sends, Alt+Enter (or Ctrl+J) adds a line, Esc
interrupts, Shift+Tab cycles ask → edits → plan → bypass, Ctrl+V attaches
the clipboard's image (or drop an image file on the terminal; it shows as
`[Image #1]`), Ctrl+O unfolds reasoning, outputs and diffs. `/help` lists
the commands (`/model`, `/mode`, `/effort`, `/compact`, `/diff`, `/memory`,
`/skills`, `/agents`, `/clear`, `/cost`, `/exit`); any other `/name` runs a
skill or custom command. Replies render as Markdown with highlighted code;
`CORTEX_REDUCED_MOTION=1` turns the animations off.

Project settings, skills, agents and rules live in `.cortex/`, project
instructions in `CORTEX.md` or `.cortex/CORTEX.md`, and your own in
`~/.cortex/`. Plugins and MCP servers: [docs/plugins.md](docs/plugins.md).

## Where data lives

`~/Library/Application Support/Cortex` on macOS, `~/.config/cortex` on
Linux: sessions, memory and settings. Keys and tokens are sealed in
`secrets.json` there (its key in `secrets.key`, both readable only by
you).

## Layout

| Crate | What it is |
|---|---|
| `domain` | Domain types and rules: requests, engine events, memory, `Principal`. Pure. |
| `agent-engine` | The agent loop, tools, permissions, hooks, sub-agents, skill/agent discovery, MCP, sessions, compaction, provider APIs. |
| `protocol` | `ClientCommand` / `ServerEvent` in an `Envelope`. |
| `backend` | Command handler: runs, memory, settings, providers, secrets. |
| `llm-providers` | LLM providers (Anthropic, OpenAI, Claude, ChatGPT, GitHub Copilot and Kiro subscriptions): credentials, sign-in, request and catalog shapes. Pure. |
| `sdk` | The CLI's internal client to the backend. |
| `cli` | The `cortex` binary: terminal chat (ratatui), `-p`, `login`, `models`, `mcp`, `config`, `doctor`. |
| `brand` | The product's name and command, from `brand.toml`. |
| `subprocess`, `private-file` | Small IO helpers. |

## Developing

Read [CLAUDE.md](CLAUDE.md) (working rules, including how to run Cargo)
and [docs/architecture.md](docs/architecture.md).

```sh
export CARGO_TARGET_DIR="$PWD/target/agents/<task>"
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -q -p xtask -- lint
```
