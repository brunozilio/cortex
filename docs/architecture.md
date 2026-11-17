# Architecture

How the pieces fit today, 2026-11-16, before the restructuring plan. The "Target" section at the end says where each part is going. The product is written as "the app", its binary as `<command>`; terms in use are defined in `GLOSSARY.md`.

## Crates

Twelve crates. Dependencies are one-way. These are the real non-dev edges; `xtask lint` checks them against the manifests.

```
cli              -> app, console, content, platform, domain, brand
app              -> agent-engine, llm-providers, code-map, platform, domain, brand
agent-engine     -> llm-providers, language-servers, script, content, platform, domain, brand
llm-providers    -> platform, domain, brand
domain           -> brand
language-servers -> platform, domain, brand
content          -> script, brand
console          -> platform, brand
code-map, platform, script, brand -> no workspace crate
```

The main line is `cli -> app -> agent-engine -> llm-providers -> domain`.

| Crate | Role |
|---|---|
| `cli` | The binary: argument parsing, the terminal chat (TUI), print mode, subcommands |
| `app` | The application API, in process. `client/`: the terminal's client (sessions, runs, calls to the handler). `api/`: `ClientCommand`, `ServerEvent`, `Envelope`, the typed feature requests and replies. `handler/`: command handler, run table, outbox. `features/`: the feature folders. Beside them: provider accounts, secrets, MCP sign-in, session catalog, settings |
| `agent-engine` | The agent loop, tools, permissions, hooks, sub-agents, MCP, sessions, compaction, usage metering, loading declared providers. It re-exports `llm-providers`' `model`, `providers` and `transcript` under its own name |
| `llm-providers` | Talking to a model: the `Provider` trait, one folder per provider, the model client (HTTP, streaming, retries), wires, the transcript types, credentials, OAuth, catalogs, declared providers, vendors. The five subscription providers are behind the Cargo feature `subscription-providers`, on by default |
| `domain` | Types and rules with no IO: requests, events, permission rules, command classification, transcript, memory, trace |
| `language-servers` | Language-server client, pool and first-use installs |
| `content` | Embeds `content/` (prompts, tool descriptions, hooks, skills) and renders it |
| `script` | The QuickJS runtime |
| `console` | The local HTTP server behind the web page |
| `code-map` | Tree-sitter index of a repository, ranked against a prompt |
| `platform` | Per-OS IO: child processes, private files, file locks, data folders, sandbox |
| `brand` | The product's name and command, from `brand.toml` |

`web/` is the browser app the console serves (trace, console, documentation, terminal). `content/` is JavaScript and Markdown, not a crate. `xtask/` holds the repository lint.

`cli` reaches `agent-engine`, and the handler-side types of `app`, only in tests. Four crates (agent-engine, app, cli, llm-providers) register their vendors, providers, wires, builtin tools and features in written `mod` lines and explicit lists, each pinned by a test; no `build.rs` scans a folder.

## Request path: from a keypress to the model and back

Everything below is one process. The seam between terminal and engine is channels and threads, not a network.

1. The TUI loop (`crates/cli/src/tui/event_loop.rs`) drains the terminal events that are waiting each frame and passes each key to `keys::key` (`crates/cli/src/tui/keys.rs`). Enter calls `App::submit` (`crates/cli/src/tui/submit.rs`).
2. `submit` handles slash commands, `!` shell lines, `#` memory notes and steering locally; a prompt goes to `send_as` (`crates/cli/src/tui/turn_flow.rs`), which expands pastes and calls `Session::send_with_images` in `app`'s client (`crates/app/src/client/session.rs`). The first prompt of a session (the git-offer check) and a prompt with `@file` mentions are prepared on a worker first; the turn is busy meanwhile and Esc or Ctrl+C takes the prompt back into the input. Starting a run only enqueues a command and registers a route, so it never waits for the backend.
3. The client fills a `PromptRequest` from the session's state, registers a channel for the new `RunId`, wraps a `ClientCommand::SubmitPrompt` in an `Envelope` with a request id and sends it to the backend (`crates/app/src/client/dispatch.rs`).
4. The backend queues the command on one of two worker threads, `backend-control` or `backend-queries` (`crates/app/src/handler/local.rs`). A handler that panics answers that one request with an error and the worker goes on; a worker that dies anyway marks the backend dead, which fails every waiting call and run at once. `CommandHandler::handle` (`crates/app/src/handler/mod.rs`), for a prompt: loads the provider keys, replaces the request's credentials, lets each feature folder prepare the request (memory recall, code map, channels) and calls `AgentEngine::start`.
5. The engine (`crates/agent-engine/src/engine/mod.rs`) creates a `Workspace`, a `Control` and a `Job`, and runs the job on a new thread, `agent-engine-run`, inside a panic guard. It returns a handle for cancel, answer, mode change and steer.
6. `Job::run` (`crates/agent-engine/src/engine/job.rs`) is the list of a run's phases, each in `crates/agent-engine/src/engine/phases/`: `load_session` loads or creates the session, `brief` loads the Brief, `equip` assembles the tools (built-in, LSP, skills, sub-agents, MCP, script tools), `apply_policy` and `prepare_runtime` apply policy, model roles and the session worktree, `open` builds the system prompt, runs the `SessionStart` and `UserPromptSubmit` hooks and opens a checkpoint, `drive` runs the loop and `close` saves. A phase that ends the run returns a `Stop` (`engine/stop.rs`), turned into the run's last events in one place.
7. `Agent::turns` (`crates/agent-engine/src/agent/turn/mod.rs`) runs the loop as a state machine over a `TurnState`: ask the model, take its reply, run its tools, finish. Asking the model (`agent/turn/model_call.rs`, `agent/turn/upkeep.rs`) measures context and compacts if needed, runs the `ModelRequest` hook and calls `ModelApi::turn`.
8. The model client (`crates/llm-providers/src/model/`) asks the provider for base URL, headers and body shape, encodes the request with the model's wire, and sends it over HTTP. The stream is read line by line as server-sent events; each delta is passed back through a callback.
9. Tool calls in the reply go through `Agent::call` (`crates/agent-engine/src/agent/dispatch/mod.rs`): `PreToolUse` hook, permission decision (`crates/agent-engine/src/agent/permission/`), the tool itself, `PostToolUse` hook, redaction, persistence of large output. A permission question is emitted as an `Asked` event and the run thread blocks until the handle's `answer` is called.
10. Every event leaves through the `RunGate` (`crates/agent-engine/src/boundary/gate.rs`), which feeds the trace recorder and the backend's sink. The sink updates the session catalog and pushes a `ServerEvent::Run` into the outbox (`crates/app/src/handler/outbox.rs`).
11. The `sdk-dispatch` thread drains the outbox and routes each run event to that run's channel. A thread in the TUI iterates the run and forwards each event as a message to the UI thread.
12. `App::event` (`crates/cli/src/tui/turn_flow.rs`) handles the events that change UI state and hands the rest to `Transcript::apply` in `domain`. The next frame redraws.

Calls that are not runs (list models, settings, a feature) use the same path and come back as a `ServerEvent::Reply` carrying the id of the request it answers.

What the application itself has to tell the user (keys cannot be kept, `app.db` could not be opened) travels as a harness notice: the `for_user` field of the reply to `ClientCommand::TakeNotices`, which the TUI polls every second and shows in the transcript, and `--print` writes to the error stream. It is said once per process and never sent to the model. The `for_agent` field of the same reply is the inbox, which is taken only when the session is idle and is sent to the model as a turn.

## The local web page

`/trace`, `/console` and `/docs` start the console server on first use: loopback, an ephemeral port, a random token in the URL exchanged for a cookie. The TUI pushes the trace report, the session list and each session's runs into the console's `Live` hub; the page reads them over HTTP and server-sent events. The terminal view opens a WebSocket to a pseudo-terminal running the app's own binary with `--resume`. The console knows none of the payload formats: it carries JSON strings, and the TypeScript types are written by hand.

## Where data lives

The data folder is `~/Library/Application Support/<Name>` on macOS and `$XDG_CONFIG_HOME/<command>` or `~/.config/<command>` on Linux. `local` in the paths below is the owner directory (see `decisions/0008-single-user-local-product.md`).

| Path | Content | Written by |
|---|---|---|
| `secrets.key`, `secrets.json` | Vault key and encrypted entries | `app` secrets |
| `providers/local/providers.json` | Accounts per provider | `app` providers |
| `users/local/settings.json` | UI state: model, mode, prompt history, last session per folder | TUI saver through `app` |
| `app.db`, `app.db-wal`, `app.db-shm` | SQLite database: the session catalog, each project's memory and its undo point | `app` catalog; `app` memory feature |
| `users/local/sessions.json.imported`, `memory/local/<project>.json.imported` | What an earlier version kept as JSON, renamed after its one import into `app.db`; never read again | renamed by `app` |
| `code-map/<project>.json` | Code-map cache | `code-map` |
| `native-sessions/<hash>.jsonl` | Append-only log of one session's conversation: `message`, `truncated`, `restarted`. A file written before the split also holds trace records | `agent-engine` sessions |
| `native-sessions/<hash>.trace.jsonl`, `<hash>.trace.1.jsonl` | Append-only log of the session's trace: `prompted`, `shown`, `opened`, `event`, `called`. Past 64 MiB it is renamed to `.trace.1.jsonl` (one earlier part is kept) and a new one starts | `agent-engine` sessions |
| `native-sessions/` siblings | Per session: `checkpoints-<hash>.json`, `snapshot-<hash>.json`, `worktree-<hash>.json`, and `<hash>.lock` while a run holds the session | `agent-engine` sessions |
| `lsp/<name>/` | Downloaded language servers | `language-servers` |
| `logs/` | The log file, rotated at 5 MiB | `cli` |
| `~/.<command>/` | User settings, installed copy of the embedded content, plugins, `mcp.json`, hooks, rules | user; `content`; `app` |
| `<project>/.<command>/` | `settings.json`, `settings.local.json`, `mcp.json`, rules, hooks, tools, memory, evals, worktrees | user; engine ("always allow" rules) |
| Temporary directory | Per-run MCP configuration, memory edits, large tool results | `app`; `agent-engine` |

Every JSON file above except the two session logs, the code-map cache and the imported files is a `platform::store::Document` (`crates/platform/src/store/mod.rs`): a change takes a cross-process lock (`.<name>.lock` beside the file, waited for at most ten seconds), reads, applies itself and replaces the file whole through a temporary file that is synced first; a change that changes nothing writes nothing. A file the program owns is the user's alone (0600), and one that does not parse is renamed to `<name>.corrupt-<epoch seconds>` instead of being written over. A file its author edits by hand (`settings.json`, `settings.local.json`, `mcp.json`, the instructions file, team memory) keeps the mode it had, is reported and left where it is when it does not parse, and its lock file is removed once released so nothing is left in a repository. UI state is changed by `ClientCommand::ChangeSettings`, a list of sets, removes and latest-of-a-list applied inside one such change, so two running instances keep each other's keys.

The session catalog and the project memory are rows in `app.db` (`crates/app/src/db/`, `decisions/0006-one-persistence-pattern.md`), one database per data folder: WAL mode, `synchronous=NORMAL`, foreign keys on, a ten-second busy timeout, the user's alone (0600). `-wal` and `-shm` exist while a program has it open. A change is one `BEGIN IMMEDIATE` transaction that loads the catalog or the project's book, applies the `domain` rule and writes only the rows that differ, so running instances never lose each other's changes and a reader is never blocked by a writer. A writer kept waiting past the timeout gets an error naming the file, not a hang. The schema carries a version; migrations are forward-only and run in a transaction at open; a database from a newer version is refused untouched. When the database cannot be opened, for that or any other reason, the program keeps sessions and memory in the process only and tells the user once, with the reason (`crates/app/src/data_root.rs`, `crates/app/src/degraded.rs`). Each project's memory has a revision, bumped by every change, which the in-process copy of a book is checked against before it is used. At the first open, `users/local/sessions.json` and `memory/local/*.json` from an earlier version are imported in one transaction each and renamed with `.imported`; an older binary started afterwards therefore sees an empty session list and empty memory until those files are renamed back.

Not persisted, lost on exit: schedules, background shells, open MCP channels, the inbox, the account rotation counter, the model catalog cache.

A session's file name is a hash of the owner tag, the canonical project path and the session id. Its conversation and its trace are two append-only files (`crates/agent-engine/src/sessions/`), and every line written since the split carries a `seq` number that grows across both.

- Resuming reads only `<hash>.jsonl`, backwards to the last restart marker, and replays from there. It never opens the trace, so a trace that is missing or does not parse cannot stop a conversation from resuming; a trace line that does not parse inside an old mixed file is skipped with a warning, and a last line cut mid-write is dropped. A conversation line that does not parse still fails the load.
- The conversation is saved at every transition of the turn (`agent/turn/mod.rs`, through `Agent::run_keeping`): before each model call (so the prompt is on disk before the first one), after each reply is recorded, after each batch of tool results, and once more in `close`, which normally finds nothing left to write. Each save is one `write` of the new lines followed by `sync_data`. Sub-agents' conversations are not saved.
- A save appends only what is new. The store remembers, per session, how many messages it has persisted, a digest of them and the file's length; when the file still has that length and the digest still matches the start of the conversation, the rest is appended without reading the file. Otherwise (another process appended, a fresh process, an earlier message changed) the file is read once and the difference written: new messages, `truncated` for a rewind, `truncated` marked `amended` when an already kept message changed (a reminder joining the last tool results, for example), `restarted` and the whole conversation after a compaction.
- The trace is written through one buffered file per run, opened at the first entry, flushed at each transition and at the terminal event, never synced. A reader in the same process flushes it first.
- Readers of the trace (`FileSessions::entries`, then `sessions::records`) read `<hash>.jsonl`, `<hash>.trace.1.jsonl` and `<hash>.trace.jsonl` and merge them by `seq`. A line without a number (written before the split, or by an older binary since) takes the number of the line before it in its file, so an old mixed file reads in the order it was written and comes before everything numbered.
- A dangling tool call left by a process that died is closed as interrupted when the session is loaded (`sessions/repair.rs`).

Housekeeping: `<hash>.lock` is removed when the run lets go of the session (unlinked while the lock is still held, and a taker checks that the file it locked is still the one at that path). A session's checkpoints keep the files of its newest 50 prompts and at most 64 MiB of them; older checkpoints stay in the list, so indices do not move, but lose their files and can no longer be rewound to. A checkpoint is added in one `Document::update`. Folders under `<temporary directory>/<command>-tool-results/` not written to for seven days are removed when the engine is first built in a process, without following links; a pointer to a transcript saved before a compaction is rewritten when the session is loaded and that file is gone.

## Threading model

Operating-system threads, synchronous code, channels. There is no async runtime in the terminal client, the client library or the backend.

- UI: one thread runs the TUI loop and wakes at least every 80 ms. Work off the UI thread is one new thread per task, reporting back with a message. A saver thread persists UI state; a watchdog thread checks the loop is not stuck.
- Client: one `sdk-dispatch` thread per client, woken by the backend or every 250 ms.
- Backend: two worker threads, each handling its queue in series and each command inside a panic guard; extra threads for provider sign-in, model catalog discovery and MCP sign-in.
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
| `sdk`, `protocol` and `backend` merged into one typed `app` crate with correlation ids (done) | `decisions/0005-typed-application-api.md` |
| One document helper, SQLite for catalog and memory, transcript split from trace and saved per transition | `decisions/0006-one-persistence-pattern.md` |
| One Tokio runtime in the engine with structured cancellation | `decisions/0007-one-async-runtime.md` |
| The owner concept is removed; on-disk names stay as they are | `decisions/0008-single-user-local-product.md` |
| Function-size lint instead of a file-length limit; `impl` blocks regrouped | `decisions/0009-function-size-lint.md` |
| Rationale lives in `docs/` | `decisions/0010-comments-stay-out-of-code.md` |

Provider folders, the model client, wires and the transcript types have moved from `agent-engine` to `llm-providers` (done). Also planned, without a record of their own: `domain`'s 67 top-level modules are grouped into eight areas.
