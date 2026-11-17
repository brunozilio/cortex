# Architecture

How the pieces fit today, 2026-11-16, before the restructuring plan. The "Target" section at the end says where each part is going. The product is written as "the app", its binary as `<command>`; terms in use are defined in `GLOSSARY.md`.

## Crates

Fourteen crates. Dependencies are one-way. These are the real non-dev edges; `xtask lint` checks them against the manifests.

```
cli              -> sdk, console, content, platform, domain, brand
sdk              -> backend, protocol, platform, domain, brand
backend          -> agent-engine, llm-providers, code-map, protocol, platform, domain, brand
agent-engine     -> llm-providers, language-servers, script, content, platform, domain, brand
llm-providers    -> domain, brand
protocol         -> domain
domain           -> brand
language-servers -> platform, domain, brand
content          -> script, brand
console          -> platform, brand
code-map, platform, script, brand -> no workspace crate
```

The main line is `cli -> sdk -> backend -> agent-engine -> llm-providers -> domain`.

| Crate | Role |
|---|---|
| `cli` | The binary: argument parsing, the terminal chat (TUI), print mode, subcommands |
| `sdk` | The terminal's client: sessions, runs, calls to the backend |
| `protocol` | `ClientCommand`, `ServerEvent`, `Envelope` |
| `backend` | Command handler, run table, provider accounts, secrets, memory, session catalog, feature folders |
| `agent-engine` | The agent loop, tools, permissions, hooks, sub-agents, MCP, sessions, compaction, model client, wires, one folder per provider |
| `llm-providers` | The `Provider` trait, credentials, OAuth, catalogs, declared providers, vendors |
| `domain` | Types and rules with no IO: requests, events, permission rules, command classification, transcript, memory, trace |
| `language-servers` | Language-server client, pool and first-use installs |
| `content` | Embeds `content/` (prompts, tool descriptions, hooks, skills) and renders it |
| `script` | The QuickJS runtime |
| `console` | The local HTTP server behind the web page |
| `code-map` | Tree-sitter index of a repository, ranked against a prompt |
| `platform` | Per-OS IO: child processes, private files, file locks, data folders, sandbox |
| `brand` | The product's name and command, from `brand.toml` |

`web/` is the browser app the console serves (trace, console, documentation, terminal). `content/` is JavaScript and Markdown, not a crate. `xtask/` holds the repository lint.

`cli` reaches `backend` and `agent-engine` only in tests. Four crates (agent-engine, backend, cli, llm-providers) register their vendors, providers, wires, builtin tools and features in written `mod` lines and explicit lists, each pinned by a test; no `build.rs` scans a folder.

## Request path: from a keypress to the model and back

Everything below is one process. The seam between terminal and engine is channels and threads, not a network.

1. The TUI loop (`crates/cli/src/tui/mod.rs`) reads one terminal event per frame and passes a key to `keys::key`. Enter calls `App::submit`.
2. `submit` handles slash commands, `!` shell lines, `#` memory notes and steering locally; a prompt goes to `send_as`, which expands pastes and `@file` mentions and calls `Session::send_with_images` in `sdk`.
3. `sdk` fills a `PromptRequest` from the session's state, registers a channel for the new `RunId`, wraps a `ClientCommand::SubmitPrompt` in an `Envelope` and sends it to the backend (`crates/sdk/src/client.rs`).
4. `backend` queues the command on one of two worker threads, `backend-control` or `backend-queries` (`crates/backend/src/local.rs`). `CommandHandler::handle` (`crates/backend/src/handler.rs`) checks the envelope, then for a prompt: loads the provider keys, replaces the request's credentials, lets each feature folder prepare the request (memory recall, code map, channels) and calls `AgentEngine::start`.
5. The engine (`crates/agent-engine/src/engine/mod.rs`) creates a `Workspace`, a `Control` and a `Job`, and runs the job on a new thread, `agent-engine-run`, inside a panic guard. It returns a handle for cancel, answer, mode change and steer.
6. `Job::run` (`engine/job.rs`) loads or creates the session, loads the Brief, assembles the tools (built-in, LSP, skills, sub-agents, MCP, script tools), applies policy and model roles. `converse` (`engine/converse.rs`) builds the system prompt, runs the `SessionStart` and `UserPromptSubmit` hooks, opens a checkpoint and starts the loop.
7. `Agent::run_loop` (`agent/mod.rs`), per turn: measure context and compact if needed, run the `ModelRequest` hook, call `ModelApi::turn`.
8. The model client (`model/`) asks the provider for base URL, headers and body shape, encodes the request with the model's wire, and sends it over HTTP. The stream is read line by line as server-sent events; each delta is passed back through a callback.
9. Tool calls in the reply go through `Agent::call` (`agent/dispatch.rs`): `PreToolUse` hook, permission decision (`agent/permission.rs`), the tool itself, `PostToolUse` hook, redaction, persistence of large output. A permission question is emitted as an `Asked` event and the run thread blocks until the handle's `answer` is called.
10. Every event leaves through the `RunGate` (`boundary/gate.rs`), which feeds the trace recorder and the backend's sink. The sink updates the session catalog and pushes a `ServerEvent::Run` into the outbox (`crates/backend/src/outbox.rs`).
11. The `sdk-dispatch` thread drains the outbox and routes each run event to that run's channel. A thread in the TUI iterates the run and forwards each event as a message to the UI thread.
12. `App::event` (`crates/cli/src/tui/runs.rs`) handles the events that change UI state and hands the rest to `Transcript::apply` in `domain`. The next frame redraws.

Calls that are not runs (list models, settings, a feature) use the same path but come back on one shared reply channel, matched by event type.

## The local web page

`/trace`, `/console` and `/docs` start the console server on first use: loopback, an ephemeral port, a random token in the URL exchanged for a cookie. The TUI pushes the trace report, the session list and each session's runs into the console's `Live` hub; the page reads them over HTTP and server-sent events. The terminal view opens a WebSocket to a pseudo-terminal running the app's own binary with `--resume`. The console knows none of the payload formats: it carries JSON strings, and the TypeScript types are written by hand.

## Where data lives

The data folder is `~/Library/Application Support/<Name>` on macOS and `$XDG_CONFIG_HOME/<command>` or `~/.config/<command>` on Linux. `local` in the paths below is the owner directory (see `decisions/0008-single-user-local-product.md`).

| Path | Content | Written by |
|---|---|---|
| `secrets.key`, `secrets.json` | Vault key and encrypted entries | `backend` secrets |
| `providers/local/providers.json` | Accounts per provider | `backend` providers |
| `users/local/settings.json` | UI state: model, mode, prompt history, last session per folder | TUI saver through `backend` |
| `users/local/sessions.json` | Session catalog | `backend` catalog |
| `memory/local/<project>.json` | Project memory | `backend` memory feature |
| `code-map/<project>.json` | Code-map cache | `code-map` |
| `native-sessions/<hash>.jsonl` | Append-only log of one session: conversation and trace records together | `agent-engine` sessions |
| `native-sessions/` siblings | Per session: checkpoints, prompt snapshot, worktree, lock | `agent-engine` sessions |
| `lsp/<name>/` | Downloaded language servers | `language-servers` |
| `logs/` | The log file, rotated at 5 MiB | `cli` |
| `~/.<command>/` | User settings, installed copy of the embedded content, plugins, `mcp.json`, hooks, rules | user; `content`; `backend` |
| `<project>/.<command>/` | `settings.json`, `settings.local.json`, `mcp.json`, rules, hooks, tools, memory, evals, worktrees | user; engine ("always allow" rules) |
| Temporary directory | Per-run MCP configuration, memory edits, large tool results | `backend`; `agent-engine` |

Not persisted, lost on exit: schedules, background shells, open MCP channels, the inbox, the memory undo point, the account rotation counter, the model catalog cache.

A session's file name is a hash of the owner tag, the canonical project path and the session id. Resuming reads the log backwards to the last restart marker and replays from there. The conversation is saved at the end of a run.

## Threading model

Operating-system threads, synchronous code, channels. There is no async runtime in the terminal client, the client library or the backend.

- UI: one thread runs the TUI loop and wakes at least every 80 ms. Work off the UI thread is one new thread per task, reporting back with a message. A saver thread persists UI state; a watchdog thread checks the loop is not stuck.
- Client: one `sdk-dispatch` thread per client, woken by the backend or every 250 ms.
- Backend: two worker threads, each handling its queue in series; extra threads for provider sign-in, model catalog discovery and MCP sign-in.
- Engine: one thread per run. Scoped threads for background sub-agents (up to 16), batches of parallel reading tools (8 at a time), script calls and some hooks.
- Async islands: model HTTP runs on a global two-worker Tokio runtime that synchronous code blocks on, polling every 25 ms for cancellation; the console server has its own two-worker Tokio runtime.
- Scripts: a new QuickJS runtime per invocation.
- Cancellation: a stop flag on the run's `Control`, checked at the top of the loop, in stream reading, in HTTP sends, in retries, in child-process waits and in the script interrupt handler. Cancelling a parent cancels its children.
- Shared state: about a dozen process-global maps keyed by client number (inbox, peers, shells, schedules, channels, memory). Poisoned mutexes are tolerated through a helper in `platform`.

## Target

The plan runs in eleven phases on `main`. The decisions that change this document:

| Change | Record |
|---|---|
| A session object lives in the engine across runs; MCP servers are pooled | `decisions/0001-long-lived-session.md` |
| `Job::run`, `converse` and `run_loop` become typed phases and a `TurnState` | `decisions/0002-agent-loop-state-machine.md` |
| Tools declare effects in Rust; one policy engine; a real shell parser | `decisions/0003-effects-based-permission-policy.md` |
| An untrusted project folder loads nothing that executes | `decisions/0004-workspace-trust.md` |
| `sdk`, `protocol` and `backend` merge into one typed `app` crate with correlation ids | `decisions/0005-typed-application-api.md` |
| One document helper, SQLite for catalog and memory, transcript split from trace and saved per transition | `decisions/0006-one-persistence-pattern.md` |
| One Tokio runtime in the engine with structured cancellation | `decisions/0007-one-async-runtime.md` |
| The owner concept is removed; on-disk names stay as they are | `decisions/0008-single-user-local-product.md` |
| Function-size lint instead of a file-length limit; `impl` blocks regrouped | `decisions/0009-function-size-lint.md` |
| Rationale lives in `docs/` | `decisions/0010-comments-stay-out-of-code.md` |

Also planned, without a record of their own: provider folders and wires move from `agent-engine` to `llm-providers`; `domain`'s 67 top-level modules are grouped into eight areas.
