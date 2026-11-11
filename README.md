# Superfik Lite

Superfik's agent engine: a terminal chat
(CLI/TUI) for talking to a coding agent, and an SDK for building on top of it
(agents, commands, skills, memory, in-process MCP tools, MCP over HTTP and
stdio).

## Installing

```sh
curl -fsSL https://raw.githubusercontent.com/brunozilio/lite/main/install.sh | bash
```

It installs the latest release in `~/.local/bin` (`SUPERFIK_INSTALL_DIR`
to change it, `SUPERFIK_VERSION` for another release), checking its
SHA-256. On Linux it takes the glibc build when the system's glibc is
2.39 or newer, and the static musl build otherwise (RHEL 8/9, older
Debian/Ubuntu; `SUPERFIK_LIBC=musl|gnu` to choose). The musl build has no
semantic memory search: memory is searched by words. From source:
`cargo install --path crates/cli`.

## Using it

```sh
superfik login                             # API key, or Claude/ChatGPT/Copilot subscription
superfik                                   # chat in the current project
superfik -p "explain src/main.rs"          # one answer, printed
superfik models                            # what the connected providers offer
superfik pull Qwen3-8B-4bit                # a local MLX model (Apple silicon), from mlx-community
superfik pull org/Some-Model-4bit          # any MLX model on Hugging Face (HF_TOKEN for gated ones)
superfik rm Qwen3-8B-4bit                  # delete it again
superfik update                            # replace this binary with the latest release
```

Released binaries (macOS Apple silicon, Linux x86_64/arm64 for glibc and
static musl) are attached to each GitHub release; `superfik update`
downloads this platform's (a musl build stays musl),
checks its SHA-256 and replaces the running binary in place, wherever it
was installed.

Every terminal shares one background daemon (local models, embeddings
and memory load once); it leaves a few seconds after the last terminal
closes. `superfik daemon
status|stop` manages it; `SUPERFIK_NO_DAEMON=1` runs everything in the
terminal's own process. Data lives in `~/Library/Application
Support/Superfik Lite` (macOS) or `~/.config/superfik-lite`. Keys and
tokens are sealed in `secrets.json` there (its key in `secrets.key`, both
readable only by you).

In the chat: Enter sends, Alt+Enter (or Ctrl+J) adds a line, Esc
interrupts, Shift+Tab cycles ask → edits → plan → bypass, Ctrl+V attaches
the clipboard's image (or drop an image file on the terminal; it shows as
`[Image #1]`), Ctrl+O unfolds reasoning, outputs and diffs. `/help` lists
the commands (`/model`, `/mode`, `/effort`, `/compact`, `/diff`, `/memory`,
`/skills`, `/agents`, `/new`, `/cost`, `/exit`); any other `/name` runs a
skill or custom command. Replies render as Markdown with highlighted code;
`SUPERFIK_REDUCED_MOTION=1` turns the animations off.

## Building on it (SDK)

```rust
use sdk::{Agent, Command, McpConnection, McpServer, Superfik};
use serde_json::json;

let superfik = Superfik::builder()
    .agent(Agent::new("reviewer", "Reviews a diff for bugs.").tools(["Read", "Grep"]))
    .command(Command::new("audit", "Audits a path.", "Audit $ARGUMENTS for security issues."))
    .mcp(McpServer::new("math").tool("add", "Adds a and b.", json!({"type": "object"}), |input| {
        Ok(json!(input["a"].as_f64().unwrap_or(0.0) + input["b"].as_f64().unwrap_or(0.0)))
    }))
    .connect(McpConnection::http("docs", "https://example.com/mcp").header("Authorization", "Bearer …"))
    .build()?;
let model = superfik.models()?.into_iter().next().ok_or(sdk::Error::NoModel)?;
let session = superfik.session(".", model);
for event in session.send("/audit src")? { /* EngineEvent: text, tools, asks… */ }
superfik.memory(".").search("how do we deploy")?;
```

`Skill::new`, `McpConnection::stdio`, `Session::send_with_images`, `Run::answer`/`cancel`,
`connect_provider`/`sign_in` and `memory(..).remember/forget` cover the
rest; see `crates/sdk/src/lib.rs`.

## Layout

| Crate | What it is |
|---|---|
| `domain` | Domain types: requests, engine events, memory, `Principal`. Pure. |
| `agent-engine` | The `AgentEngine` trait and the engine behind it: the agent loop, tools, permissions, hooks, sub-agents, skill/agent discovery, MCP, sessions, compaction, provider APIs. |
| `mlx` | Local MLX models (Apple silicon). |
| `protocol` | `ClientCommand` / `ServerEvent` in an `Envelope`. |
| `backend` | Command handler and daemon: runs, memory, settings, providers, secrets. |
| `llm-providers` | LLM providers (Anthropic, OpenAI, Claude, ChatGPT and GitHub Copilot subscriptions): credentials, sign-in, request and catalog shapes. Pure. |
| `sdk` | The SDK: sessions and runs, agents, skills, commands, memory, in-process MCP tools, MCP over HTTP/stdio. |
| `cli` | The `superfik` binary: terminal chat (ratatui), `-p`, `login`, `models`, `pull`/`rm`. |
| `subprocess`, `-private-file` | Small IO helpers. |

Architecture: [docs/architecture.md](docs/architecture.md). Decisions:
[docs/decisions.md](docs/decisions.md).

## Developing

```sh
export CARGO_TARGET_DIR="$PWD/target/agents/<task>"   # see .superfik/SUPERFIK.md
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -q -p xtask -- lint
```
