# Superfik Lite

Internal. Superfik's agent engine without the desktop app: a terminal chat
(CLI/TUI) for talking to a coding agent, and an SDK for building on top of it
(agents, commands, skills, memory, in-process MCP tools, MCP over HTTP and
stdio).

Status: cleanup done; the CLI and SDK crates come next.

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
