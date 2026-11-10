# Superfik Lite

Internal. Superfik's agent engine without the desktop app: a terminal chat
(CLI/TUI) for talking to a coding agent, and an SDK for building on top of it
(agents, commands, skills, memory, in-process MCP tools, MCP over HTTP and
stdio).

## Using it

```sh
cargo install --path crates/superfik-cli   # installs `superfik`
superfik login                             # API key, or Claude/ChatGPT subscription
superfik                                   # chat in the current project
superfik -p "explain src/main.rs"          # one answer, printed
superfik models                            # what the connected providers offer
```

Every terminal shares one background daemon (local models, embeddings
and memory load once); it leaves after 30 idle minutes. `superfik daemon
status|stop` manages it; `SUPERFIK_NO_DAEMON=1` runs everything in the
terminal's own process. Data lives in `~/Library/Application
Support/Superfik Lite` (macOS) or `~/.config/superfik-lite`, apart from the
desktop Superfik.

In the chat: Enter sends, Alt+Enter (or Ctrl+J) adds a line, Esc
interrupts, Shift+Tab cycles ask → edits → plan → bypass, `/help` lists the
commands (`/model`, `/mode`, `/effort`, `/compact`, `/memory`, `/skills`,
`/agents`, `/new`, `/cost`, `/exit`); any other `/name` runs a skill or
custom command.

## Building on it (SDK)

```rust
use superfik_sdk::{Agent, Command, McpConnection, McpServer, Superfik};
use serde_json::json;

let superfik = Superfik::builder()
    .agent(Agent::new("reviewer", "Reviews a diff for bugs.").tools(["Read", "Grep"]))
    .command(Command::new("audit", "Audits a path.", "Audit $ARGUMENTS for security issues."))
    .mcp(McpServer::new("math").tool("add", "Adds a and b.", json!({"type": "object"}), |input| {
        Ok(json!(input["a"].as_f64().unwrap_or(0.0) + input["b"].as_f64().unwrap_or(0.0)))
    }))
    .connect(McpConnection::http("docs", "https://example.com/mcp").header("Authorization", "Bearer …"))
    .build()?;
let model = superfik.models()?.into_iter().next().ok_or(superfik_sdk::Error::NoModel)?;
let session = superfik.session(".", model);
for event in session.send("/audit src")? { /* EngineEvent: text, tools, asks… */ }
superfik.memory(".").search("how do we deploy")?;
```

`Skill::new`, `McpConnection::stdio`, `Run::answer`/`cancel`,
`connect_provider`/`sign_in` and `memory(..).remember/forget` cover the
rest; see `crates/superfik-sdk/src/lib.rs`.

## Layout

| Crate | What it is |
|---|---|
| `superfik-core` | Domain types: requests, turns, engine events, memory, plugins, `Principal`. Pure. |
| `superfik-engine` | `AgentEngine` trait and registry. |
| `superfik-native` | The agent loop, tools, permissions, hooks, sub-agents, MCP, sessions, compaction, provider APIs. |
| `superfik-gateway` | Local MLX runtime and stream utilities. |
| `superfik-protocol` | `ClientCommand` / `ServerEvent` in an `Envelope`. |
| `superfik-backend` | Command handler: runs, memory, sessions, settings, plugins, providers. |
| `superfik-workspace` | git and `gh` behind traits. |
| `superfik-plugins` | Plugin manifests, skill/agent discovery (Claude Code layout), MCP config. |
| `superfik-providers` | LLM providers (Anthropic, OpenAI, Claude and ChatGPT subscriptions): credentials, sign-in, request and catalog shapes. Pure. |
| `superfik-sdk` | The SDK: sessions and runs, agents, skills, commands, memory, in-process MCP tools, MCP over HTTP/stdio. |
| `superfik-cli` | The `superfik` binary: terminal chat (ratatui), `-p`, `login`, `models`. |
| `superfik-oauth`, `-keychain`, `-process`, `-private-file` | Small IO helpers. |

Architecture: [docs/architecture.md](docs/architecture.md). Decisions:
[docs/decisions.md](docs/decisions.md).

## Developing

```sh
export CARGO_TARGET_DIR="$PWD/target/agents/<task>"   # see CLAUDE.md
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -q -p xtask -- lint
```
