# Architecture

Superfik Lite is a Cargo workspace of small crates with one-way
dependencies. Business rules never depend on IO. Decisions are in
`docs/decisions.md` (cited as `ADR NNNN`), vocabulary in `GLOSSARY.md`.

```
(cli, sdk — next) ──► superfik-backend ──► superfik-native ──► (provider APIs, MLX)
                           │                    ├──► superfik-engine
                           │                    ├──► superfik-plugins
                           │                    └──► superfik-gateway (MLX)
                           ├──► superfik-workspace ─► (git, gh)
                           └──► superfik-protocol ─► superfik-core   (everything speaks core)
```

| Crate | Responsibility |
|---|---|
| `superfik-core` | Domain model: requests, turns, context, agents, engine events, ids, `Principal`, memory rules, usage and cost. Pure. |
| `superfik-engine` | `AgentEngine` trait, `EngineRegistry`, a `fake` engine for tests. |
| `superfik-native` | Superfik's own engine (ADR 0015): agent loop, tools, permissions, hooks, sub-agents, background tasks, MCP client, sessions, compaction, side calls, safety check; provider APIs behind `ModelApi`. |
| `superfik-gateway` | Local MLX runtime; SSE and Chat Completions conversion. |
| `superfik-protocol` | Serialisable `ClientCommand` / `ServerEvent` in an `Envelope` carrying the `Principal`; the `Backend` trait. |
| `superfik-backend` | `LocalBackend` (worker threads) and `CommandHandler`: runs, memory (+ its MCP tool), sessions, settings, plugins, model providers. |
| `superfik-workspace` | Git inspection, branch switching, worktrees; repository slug and current PR through `gh`. |
| `superfik-plugins` | Plugin manifests and packages, skill and agent discovery in Claude Code's layout, MCP config, provider contracts (pure). |
| `superfik-oauth` | Browser sign-in: PKCE and the loopback redirect. |
| `superfik-keychain` | Secrets in the system store. |
| `superfik-process` | Child processes started the same everywhere; `Tree` ends a process with its children. |
| `superfik-private-file` | Files only the current user can read. |

## Commands and events

A client sends `Envelope<ClientCommand>` to a `Backend` and polls
`ServerEvent`s, woken by a `Wake` callback. `LocalBackend` runs everything
in process on three worker threads (run control, workspace queries,
memory), so a slow scan never delays a cancel. Every command carries a
`Principal` and passes an `Authorizer` (`LocalAuthorizer` accepts only the
local owner); this is the seam for a future remote server.

## Engines

`AgentEngine::start` takes a `PromptRequest` and emits `EngineEvent`s;
exactly one terminal event (`Completed`, `Failed`, `Cancelled`) ends a run.
Recognised problems arrive first as `Diagnosed(EngineProblem)`.

The native engine (ADR 0015) runs the loop in process behind `ModelApi`
(`anthropic::Messages`, `openai::Responses`, `chat::Chat` for local
servers). `ListModels` asks every connected provider account's catalog and
lists downloaded MLX models. A run uses its model's account key
(`RunCredentials::model`, never serialized).

- **Built-in agents**: `Explore`, `Plan`, `general-purpose`, `fork`,
  `superfik`. Project or user agents of the same name replace them.
- **Prompt**: planning mode, output styles, tool descriptions carrying the
  working rules; project instructions and rules (`docs/native-engine.md`).
- **Compaction** keeps only the nine-section `<summary>`.
- **Side calls** (`chores/`): `Title`, `Suggest`, `Recap`, `Side`,
  `HeadsUp`, `Rename`.
- **Safety check** (`guard/`): in Bypass, every editing or executing action
  is graded by the run's model before it runs.
- **Built-in skills** (`crates/superfik-native/skills/`, embedded by
  `build.rs`).

Local models (`superfik_gateway::mlx`) live in
`<local data>/models/mlx/<org>/<name>/` on Apple silicon; one
`mlx_lm.server` runs at a time and stops when idle.

## Memory

Agents remember a project between sessions. Memories
(`superfik_core::memory`) are episodic, semantic or procedural, with tags
and author. `MemoryStore` (`FileMemoryStore`, one JSON per project). A
fresh session gets recalled memories plus instructions to write
`{remember, forget, revise}` to a temp file, applied on `Completed`.
Search is BM25 plus embedding similarity (fastembed/ONNX,
`SUPERFIK_MEMORY_MODEL=minilm|mpnet|off`), fused by reciprocal rank; the
same function serves recall and the `memory_search` MCP tool (loopback, a
bearer token per run).

## Plugins and MCP

A plugin (`superfik-plugin.json`) brings skills, agents and MCP servers
(`stdio` or `http`, with `{{secret.<key>}}` templates); "install from
folder" takes any. Connections are signed-in accounts of a plugin; secrets
stay in the backend's `SecretStore`, and a run's MCP config is written
`0600` and removed on the terminal event. The four AI providers are still
catalog plugins today; ADR 0018 moves them out.

## Sessions

Sessions survive restarts: `SaveSession` / `LoadSessions`
(`FileSessionStore`, one JSON per session). The engine keeps its own
transcript (`FileSessions`) for resuming.
