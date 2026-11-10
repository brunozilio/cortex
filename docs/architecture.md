# Architecture

Superfik Lite is a Cargo workspace of small crates with one-way
dependencies. Business rules never depend on IO. Decisions are in
`docs/decisions.md` (cited as `ADR NNNN`), vocabulary in `GLOSSARY.md`.

```
superfik-cli ──► superfik-sdk ──► superfik-backend ──► superfik-native ──► (provider APIs)
                                       │                    ├──► superfik-engine
                                       │                    └──► superfik-providers
                                       ├──► superfik-mlx ─► (mlx_lm.server)
                                       └──► superfik-protocol ─► superfik-core   (everything speaks core)
```

| Crate | Responsibility |
|---|---|
| `superfik-cli` | The `superfik` binary: a full-screen chat (ratatui) over the SDK, `-p` print mode, `login`, `models`. Presentation only. |
| `superfik-sdk` | `Superfik::builder()`: a `LocalBackend` plus the host's `Extensions`; `Session`/`Run` over the protocol; memory, providers, models, settings. |
| `superfik-core` | Domain model: requests, engine events, ids, `Principal`, memory rules, usage and cost. Pure. |
| `superfik-engine` | `AgentEngine` trait (`models`, `start`) and a `fake` engine for tests. |
| `superfik-native` | Superfik's own engine (ADR 0015): agent loop, tools, permissions, hooks, sub-agents, background tasks, MCP client, sessions, compaction, side calls, safety check; skill and agent discovery in Claude Code's layout; provider APIs behind `ModelApi`. |
| `superfik-mlx` | Local MLX models on Apple silicon: which are downloaded, and one `mlx_lm.server` started on demand. |
| `superfik-protocol` | Serialisable `ClientCommand` / `ServerEvent` in an `Envelope` carrying the `Principal`; the `Backend` trait. |
| `superfik-backend` | `LocalBackend` (worker threads), `CommandHandler` and the daemon: runs, memory (+ its MCP tool), settings, model providers, secrets, the host's `Extensions`. |
| `superfik-providers` | LLM provider contracts (ADR 0016): credential config, OAuth shapes, request and catalog shapes. Pure. |
| `superfik-oauth` | Browser sign-in: PKCE and the loopback redirect. |
| `superfik-process` | Child processes started the same everywhere; `Tree` ends a process with its children. |
| `superfik-private-file` | Files only the current user can read. |

## Commands and events

A client sends `Envelope<ClientCommand>` to a `Backend` and polls
`ServerEvent`s, woken by a `Wake` callback. `LocalBackend` runs everything
in process on three worker threads (run control, skill and agent
queries, memory), so a slow scan never delays a cancel. Every command carries a
`Principal` and passes an `Authorizer` (`LocalAuthorizer` accepts only the
local owner); this is the seam for a future remote server.

## Engines

`AgentEngine::start` takes a `PromptRequest` and emits `EngineEvent`s;
exactly one terminal event (`Completed`, `Failed`, `Cancelled`) ends a run.
Recognised problems arrive first as `Diagnosed(EngineProblem)`.

The native engine (ADR 0015) runs the loop in process behind `ModelApi`
(`anthropic::Messages`, `openai::Responses`, `chat::Chat` for local
servers). `ModelProviders` keeps one account per
provider (`ConnectProvider` with an API key or token JSON, `SignInProvider`
for a subscription's browser sign-in). `ListModels` asks every connected
provider's catalog and lists downloaded MLX models. A run uses its
provider's key (`RunCredentials::model`, never serialized).

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

Local models (`superfik_mlx`) live in
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

## Skills, agents and MCP

The engine finds skills, commands and agents where Claude Code keeps them
(the project's `.claude/` and `.superfik/`, `~/.claude`) plus the SDK
host's folders (`Extensions::dirs`).
MCP servers come from the host: HTTP/stdio servers go in a run's private
MCP config (`0600`, removed on the terminal event), in-process ones ride on
the run's credentials.

**Plugins** (ADR 0019) use Claude Code's plugin layout and reach a run
whole (`superfik_native::plugins`): skills, commands and agents as
`<plugin>:<name>`, `hooks/hooks.json` joined to the settings' hooks, and
`.mcp.json` servers as `plugin_<plugin>_<server>`, with
`${CLAUDE_PLUGIN_ROOT}` filled in. They come from the backend
(`Extensions::plugins`, never from a client) and from Claude Code's
installs; a plugin whose hooks or servers do not parse is left out, the
run goes on.

## The SDK and the CLI

The SDK speaks only `ClientCommand`/`ServerEvent` (one dispatcher thread
routes run events to their `Run`, other answers to the waiting call), so it
can move to a remote backend unchanged. What a program adds reaches every
run through the backend's `Extensions`: its agents, skills and commands are
written to `<data>/sdk/<name>/` and offered under their own names
(`Extensions::dirs`); HTTP/stdio MCP servers join the run's private
MCP config; in-process servers (`McpServer`, closures behind
`superfik_core::mcp::McpEndpoint`) ride on `RunCredentials::mcp` and are
called through `LocalTransport`, never over the wire. A typed
`/name args` is expanded by the engine when it names a skill or command.

### The daemon

On Unix every `superfik` talks to one background daemon per computer
(`superfik_backend::daemon`), started on first use (`superfik daemon
serve`, detached) and gone after 30 idle minutes. It loads the services
once (MLX servers, the embedding model, memory books) and gives each
client connection its own `LocalBackend` on clones of them, so a client
gets only its own runs' events and answers, and its runs end when it
disconnects. The wire is newline-delimited JSON over `daemon.sock`
(`0600`, in a `0700` folder): `Envelope<ClientCommand>` in, `ServerEvent`
out. A lock file keeps one daemon per folder; `daemon.json` holds its pid
and build so a newer `superfik` can say the daemon is stale (`superfik
daemon stop`). `SUPERFIK_NO_DAEMON=1` (and Windows) run the backend in
process. SDK programs embed their own backend by default (their
in-process tools cannot cross processes); `Superfik::connect_daemon`
joins the daemon instead.

The CLI is a client of the SDK: `App` (state and effects, slow calls on
threads), `Transcript` (events folded into what is shown), `Input`, `ask`,
`view` (drawing), `diffs` (edits and `git diff`, folded until Ctrl+O),
`attach` (images for the prompt). Its own `/commands` parse in
`superfik_core::command`; model, mode and effort persist in the owner's
settings. `/diff` runs `git` in the client's folder, the one place the CLI
reads the project itself. Attached images travel in
`PromptRequest::images` and open the first user message.

## Secrets

Provider keys and subscription tokens live in one
sealed file (`superfik_backend::system_secrets`): `secrets.json`, each
entry encrypted with XChaCha20-Poly1305 under a key in `secrets.key`
(both `0600`), re-read on every access so two processes never drop each
other's entries. The system keychain is not used: macOS would ask to
allow each new unsigned build of the binary.

## Sessions

The engine keeps each session's transcript (`FileSessions`, one JSON per
owner, folder and session) so `resume` continues it after a restart.
