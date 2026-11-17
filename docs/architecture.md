# Architecture

How the pieces fit today, 2026-11-16, before the restructuring plan. The "Target" section at the end says where each part is going. The product is written as "the app", its binary as `<command>`; terms in use are defined in `GLOSSARY.md`.

## Crates

Twelve crates. Dependencies are one-way. These are the real non-dev edges; `xtask lint` checks them against the manifests.

```
cli              -> app, console, relay, page, content, platform, domain, brand
app              -> agent-engine, llm-providers, code-map, platform, domain, brand
agent-engine     -> llm-providers, language-servers, script, content, platform, domain, brand
llm-providers    -> platform, domain, brand
domain           -> brand
language-servers -> platform, domain, brand
content          -> script, brand
console          -> page, tunnel, platform, brand
relay            -> page, tunnel, brand
code-map, platform, script, brand, page, tunnel -> no workspace crate
```

The main line is `cli -> app -> agent-engine -> llm-providers -> domain`.

| Crate | Role |
|---|---|
| `cli` | The binary: argument parsing, the terminal chat (TUI), print mode, subcommands |
| `app` | The application API, in process. `client/`: the terminal's client (sessions, runs, calls to the handler). `api/`: `ClientCommand`, `ServerEvent`, `Envelope`, the typed feature requests and replies. `handler/`: command handler, run table, outbox. `features/`: the feature folders. Beside them: provider accounts, secrets, MCP sign-in, session catalog, settings |
| `agent-engine` | The agent loop, tools, permissions, hooks, sub-agents, MCP, sessions, compaction, usage metering, loading declared providers. It re-exports `llm-providers`' `model`, `providers` and `transcript` under its own name |
| `llm-providers` | Talking to a model: the `Provider` trait, one folder per provider, the model client (HTTP, streaming, retries), wires, the transcript types, credentials, OAuth, catalogs, declared providers, vendors. The five subscription providers are behind the Cargo feature `subscription-providers`, on by default |
| `domain` | Types and rules with no IO, grouped into areas (below): requests, events, permission rules, command classification, transcript, memory, trace |
| `language-servers` | Language-server client, pool and first-use installs, with the pinned install recipes (`install.rs`) |
| `content` | Embeds `content/` (prompts, tool descriptions, hooks, skills) and renders it |
| `script` | The QuickJS runtime |
| `console` | The local HTTP server behind the web page, and `remote`: the machine's side of remote control (remote book, relay link, forwarding to itself) |
| `page` | The built web page embedded at compile time, served by `console` and `relay` |
| `tunnel` | Remote control's wire: relay frames, keys, pairing links, the Noise handshake and transport, the phone's requests and the machine's answers |
| `relay` | The self-hosted relay binary: serves the page as an installable web app and forwards encrypted frames between phones and their machine, and the same relay as a Cloudflare Worker (`worker/relay.js`), published by `<command> remote up` |
| `code-map` | Tree-sitter index of a repository, ranked against a prompt |
| `platform` | Per-OS IO: child processes, private files, file locks, data folders, sandbox, system keychain |
| `brand` | The product's name and command, from `brand.toml` |

`web/` is the browser app the console serves (trace, console, documentation, terminal). `content/` is JavaScript and Markdown, not a crate. `xtask/` holds the repository lint, the web page's build (`xtask web`), the release check on it and the fixtures that tie the page's types to `domain`'s.

`domain`'s modules sit in area folders under `crates/domain/src/`:

| Area | Modules |
|---|---|
| `identity/` | `ids`, `provider`, `provider_book`, `account_pool`, `credentials`, `legacy` |
| `models/` | `model`, `model_roles`, `auto_pick`, `small_model`, `fallback`, `routing`, `billing`, `cache_window` |
| `run/` | `request`, `events`, `failure`, `engines`, `approval`, `budget`, `verification`, `session_run`, `interception`, `stuck`, `watchdog`, `follow_through`, `review`, `affected`, `changes`, `claims`, `git_offer`, `factory_reset`, `evals`, `image` |
| `safety/` | `permission`, `permission_rules`, `command_policy`, `shell`, `sandbox`, `destructive`, `trust`, `untrusted`, `redact`, `suggested_rules`, `dependencies`, `egress` |
| `session/` | `conversation`, `catalog`, `usage`, `repeats` |
| `knowledge/` | `memory`, `learning` |
| `tools/` | `tool_name`, `hooks`, `lsp`, `mcp`, `mcp_config`, `mcp_oauth`, `code_mode`, `web_reading`, `schedule`, `watch`, `looping`, `inbox`, `output`, `executable`, `skill`, `agents`, `extensions`, `brief_edits` |
| `trace/` | the trace model, an area of its own |

`command` (slash commands, which reach into every area) and `features` (the flags every area is gated by) stay at the top. The crate root re-exports each module under its old name (`domain::events` is `domain::run::events`), so callers did not change with the grouping.

`cli` reaches `agent-engine`, and the handler-side types of `app`, only in tests. Four crates (agent-engine, app, cli, llm-providers) register their vendors, providers, wires, builtin tools and features in written `mod` lines and explicit lists, each pinned by a test; no `build.rs` scans a folder.

## Request path: from a keypress to the model and back

Everything below is one process. The seam between terminal and engine is channels and threads, not a network.

1. The TUI loop (`crates/cli/src/tui/event_loop.rs`) drains the terminal events that are waiting each frame and passes each key to `keys::key` (`crates/cli/src/tui/keys.rs`). Enter calls `App::submit` (`crates/cli/src/tui/submit.rs`).
2. `submit` handles slash commands, `!` shell lines, `#` memory notes and steering locally; a prompt goes to `send_as` (`crates/cli/src/tui/turn_flow.rs`), which expands pastes and calls `Session::send_with_images` in `app`'s client (`crates/app/src/client/session.rs`). The first prompt of a session (the git-offer check) and a prompt with `@file` mentions are prepared on a worker first; the turn is busy meanwhile and Esc or Ctrl+C takes the prompt back into the input. Starting a run only enqueues a command and registers a route, so it never waits for the backend.
3. The client fills a `PromptRequest` from the session's state (its `Prompt`, `SessionConfig` and `Policy`; the `HostContext` part is the backend's), registers a channel for the new `RunId`, wraps a `ClientCommand::SubmitPrompt` in an `Envelope` with a request id and sends it to the backend (`crates/app/src/client/dispatch.rs`).
4. The backend queues the command on one of two worker threads, `backend-control` or `backend-queries` (`crates/app/src/handler/local.rs`). A handler that panics answers that one request with an error and the worker goes on; a worker that dies anyway marks the backend dead, which fails every waiting call and run at once. `CommandHandler::handle` (`crates/app/src/handler/mod.rs`), for a prompt: loads the provider keys, replaces the request's credentials, sets the request's host context (`HostContext`: plugins, home folder, and whether the project folder is trusted, read from the trust store; a value the client sent is dropped), lets each feature folder prepare the request (memory recall, code map, channels) and calls `AgentEngine::start`.
5. The engine (`crates/agent-engine/src/engine/mod.rs`) creates a `Workspace`, a `Control` and a `Job`, and starts the job as a task of the engine runtime (`engine/task.rs`): the task runs the job's synchronous loop on one of the runtime's blocking threads and turns a panic into the run's failure. It returns a handle for cancel, answer, mode change and steer. `start` stays synchronous: it only spawns the task.
6. `Job::run` (`crates/agent-engine/src/engine/job.rs`) is the list of a run's phases, each in `crates/agent-engine/src/engine/phases/`: `load_session` loads or creates the session, takes the session's hold (a second run of a busy session is refused with "answering in another window") and enters its live session (`crates/agent-engine/src/engine/live/`), `brief` takes the Brief and the script tools from the live session's cache, reloading them through the trust gate (`crates/agent-engine/src/trust/`: in an untrusted folder, project hooks, scripts, MCP servers, declared providers and `settings.local.json` grants are skipped) only when a file they are read from changed, `equip` assembles the tools (built-in, LSP, skills, sub-agents, MCP from the live session's pool, script tools), `apply_policy` and `prepare_runtime` apply policy, model roles and the session worktree, `open` builds the system prompt, runs the `SessionStart` hook on the live session's first run, then `RunStart` and `UserPromptSubmit`, and opens a checkpoint, `drive` runs the loop and `close` runs `RunEnd` and saves. A phase that ends the run returns a `Stop` (`engine/stop.rs`), turned into the run's last events in one place.
7. `Agent::turns` (`crates/agent-engine/src/agent/turn/mod.rs`) runs the loop as a state machine over a `TurnState`: ask the model, take its reply, run its tools, finish. Asking the model (`agent/turn/model_call.rs`, `agent/turn/upkeep.rs`) measures context and compacts if needed, runs the `ModelRequest` hook and calls `ModelApi::turn`.
8. The model client (`crates/llm-providers/src/model/`) asks the provider for base URL, headers and body shape, encodes the request with the model's wire, and sends it over HTTP. `ModelApi::turn` and `Http::post_streaming` are async and take the run's cancellation token; the loop, still synchronous, waits on them through `BlockingTurn::turn_blocking`. The stream is read with async IO line by line as server-sent events, each wait racing the token and the idle deadline; each delta is passed back through a callback.
9. Tool calls in the reply go through `Agent::call` (`crates/agent-engine/src/agent/dispatch/mod.rs`): `PreToolUse` hook, permission decision (`PolicyEngine::decide` in `crates/domain/src/safety/permission/` over the effects the tool declares, with hooks and the question around it in `crates/agent-engine/src/agent/permission/`), the tool itself, `PostToolUse` hook, redaction, persistence of large output. A permission question is emitted as an `Asked` event and the run waits until the handle's `answer` is called or the run's token is cancelled, whichever comes first.
10. Every event leaves through the `RunGate` (`crates/agent-engine/src/boundary/gate.rs`), which feeds the trace recorder and the backend's sink. The sink updates the session catalog and pushes a `ServerEvent::Run` into the outbox (`crates/app/src/handler/outbox.rs`).
11. The `sdk-dispatch` thread drains the outbox and routes each run event to that run's channel. A thread in the TUI iterates the run and forwards each event as a message to the UI thread.
12. `App::event` (`crates/cli/src/tui/turn_flow.rs`) handles the events that change UI state and hands the rest to `Transcript::apply` in `domain`. The next frame redraws.

Calls that are not runs (list models, settings, a feature) use the same path and come back as a `ServerEvent::Reply` carrying the id of the request it answers.

What the application itself has to tell the user (keys cannot be kept, `app.db` could not be opened) travels as a harness notice: the `for_user` field of the reply to `ClientCommand::TakeNotices`, which the TUI polls every second and shows in the transcript, and `--print` writes to the error stream. It is said once per process and never sent to the model. The `for_agent` field of the same reply is the inbox, which is taken only when the session is idle and is sent to the model as a turn.

## The local web page

`/trace`, `/console` and `/docs` start the console server on first use: loopback, an ephemeral port, a random token in the URL exchanged for a cookie. The TUI pushes the trace report, the session list and each session's runs into the console's `Live` hub; the page reads them over HTTP and server-sent events. The terminal view opens a WebSocket. For the session this process shows, it watches the mirror: the TUI's backend copies every frame it draws into the console's `Mirror` (`crates/cli/src/tui/mirror.rs`), a page that starts watching gets the size and the whole screen, and what the page types is decoded into key events and handled as if typed in the terminal; the page takes the terminal's size, not the other way round. For any other session it opens a pseudo-terminal running the app's own binary with `--resume`. The console knows none of the payload formats: it carries JSON strings, and the TypeScript types are written by hand.

The page is built outside Cargo. `cargo run -p xtask -- web` runs `bun install --frozen-lockfile` and `bun run build` in `web/` and fails when Bun is missing; `crates/page/build.rs` starts no process and reaches no network, it only embeds the files of `web/dist` when `web/dist/index.html` exists, and Cargo runs it again when that folder changes. At run time the console serves, in order, the folder named by `<COMMAND>_CONSOLE_DIR`, the embedded copy, then a `console` folder in the data folder; no path of the build machine is kept in the binary. With none of them it serves a page that says to run `cargo run -p xtask -- web` and build again. `cargo run -p xtask -- check-release` is the release's check: it fails when `web/dist/index.html` is missing or older than the page's sources. The gate (tests, clippy, lint) needs neither Bun nor a built page.

The hand-written types are held to the Rust ones by fixtures. `xtask/tests/web_fixtures/` builds a report, a followed session update and a session list from `domain`'s own types and compares them with `web/test/fixtures/*.json` (`cargo run -p xtask -- web-fixtures` rewrites them); `web/test/contract.test.ts` checks those files against the runtime guards in `web/src/lib/contract.ts`, which the page also applies to what it receives: a report, a live update or a session list of the wrong shape is refused with the name of the wrong field instead of being drawn. `domain` is a dev-dependency of `xtask` only, so `xtask lint` and the comment hook build without it.

Every `/api/` route needs the cookie, which is named after the port, and a request addressed to any host other than `127.0.0.1:<port>` or `localhost:<port>` is refused. A request body is limited to 64 KiB, at most 32 pages follow `/api/events` at once and at most four terminals are open at once (each answered `429` in JSON beyond that), and `POST /api/watch` takes only what the terminal takes as a session id. The events hub keeps the last 256 updates; a page that reads too slowly and misses some is sent a `resync` event, on which it fetches `/api/report` and `/api/latest` (each session's latest update) again, as it does after its connection was lost and came back. Meanwhile its header says the data is out of date or partial.

### Remote control

The same page reaches a phone through the owner's relay (`decisions/0012-remote-control-through-a-blind-relay.md`). The `relay` binary serves the embedded page with a `<meta name="remote-relay">` added to `index.html`, a web manifest named after the app, and `sw.js`, which caches the page's own files and never `/api/`. On the machine, `console::remote` keeps the remote book and, once a relay is named and a phone is paired or invited, connects out to `wss://<relay>/machine?id=<machine id>`; the TUI's `remote` feature starts it beside the console, from the console's own link. Each phone gets a channel: a Noise handshake against the book, then the phone's requests (`tunnel::Ask`: fetch, events, socket, send, close) are forwarded to the console on loopback with its cookie and answered (`tunnel::Answer`: head, data, end). On the phone, `web/src/remote.ts` runs before the app: on the relay's page it replaces `fetch`, `EventSource` and `WebSocket` with tunnel-backed ones (`web/src/lib/tunnel/`), so the app's code is the same in both places; the device key and the machine's public key are kept in IndexedDB. `xtask/tests/web_fixtures/noise.rs` writes a handshake and transport transcript from the Rust side that `web/test/noise.test.ts` must reproduce byte for byte.

## Where data lives

The data folder is `~/Library/Application Support/<Name>` on macOS and `$XDG_CONFIG_HOME/<command>` or `~/.config/<command>` on Linux. `local` in the paths below is the owner directory (see `decisions/0008-single-user-local-product.md`).

| Path | Content | Written by |
|---|---|---|
| `secrets.json` | The vault's encrypted entries | `app` secrets |
| `secrets.key` | The vault key, as 64 hex digits (the default); the word `keychain` only once `<PREFIX>_SECRETS=keychain` moved it into the system keychain (service: the app's name, account: this file's path) | `app` secrets, through `platform::keychain` |
| `providers/local/providers.json` | Accounts per provider | `app` providers |
| `users/local/settings.json` | UI state: model, mode, prompt history, last session per folder | TUI saver through `app` |
| `users/local/trusted-folders.json` | The project folders the user trusts: a compact list of `{ "path", "trusted_at" }`, keyed by canonical trust root | `app` trust store, on `ClientCommand::Trust` |
| `app.db`, `app.db-wal`, `app.db-shm` | SQLite database: the session catalog, each project's memory and its undo point | `app` catalog; `app` memory feature |
| `users/local/sessions.json.imported`, `memory/local/<project>.json.imported` | What an earlier version kept as JSON, renamed after its one import into `app.db`; never read again | renamed by `app` |
| `code-map/<project>.json` | Code-map cache | `code-map` |
| `native-sessions/<hash>.jsonl` | Append-only log of one session's conversation: `message`, `truncated`, `restarted`. A file written before the split also holds trace records | `agent-engine` sessions |
| `native-sessions/<hash>.trace.jsonl`, `<hash>.trace.1.jsonl` | Append-only log of the session's trace: `prompted`, `shown`, `opened`, `event`, `called`. Past 64 MiB it is renamed to `.trace.1.jsonl` (one earlier part is kept) and a new one starts | `agent-engine` sessions |
| `native-sessions/` siblings | Per session: `checkpoints-<hash>.json`, `snapshot-<hash>.json`, `worktree-<hash>.json`, and `<hash>.lock` while a run holds the session | `agent-engine` sessions |
| `lsp/<name>/` | Downloaded language servers | `language-servers` |
| `remote/remote.json` | Remote control's book, readable only by the user: the machine's private key, the relay's address, the paired devices (name, public key, when) and the pending invitations (one-time code, expiry) | `console::remote`, changed by `<command> remote` and `/remote` |
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

The terminal client, the client library and the backend are operating-system threads, synchronous code and channels. The engine runs on one Tokio runtime (`decisions/0007-one-async-runtime.md`, stages 8.1 and 8.2 done, 8.3 done but for background sub-agents and the parallel reading batch).

- UI: one thread runs the TUI loop and wakes at least every 80 ms. Work off the UI thread is one new thread per task, reporting back with a message. A saver thread persists UI state; a watchdog thread checks the loop is not stuck.
- Client: one `sdk-dispatch` thread per client, woken by the backend or every 250 ms.
- Backend: two worker threads, each handling its queue in series and each command inside a panic guard; extra threads for provider sign-in, model catalog discovery and MCP sign-in.
- Engine runtime (`crates/platform/src/runtime.rs`): one multi-threaded Tokio runtime per process, built on first use, with between two and four workers (`engine-worker`, as many as the machine has cores up to four) and at most 256 blocking threads (`engine-blocking`). `Engine` holds its handle; model HTTP and the engine share it. The console server keeps its own two-worker runtime.
- Runs: a run is a task on the engine runtime; its agent loop is still synchronous and runs on a blocking thread of that runtime (`engine/task.rs`). A panic in it is the task's error and becomes the run's `Failed`. Synchronous code waits on async code with `platform::runtime::block_on`, which drives the future on the waiting thread; a second wait started inside one is served on a short-lived thread instead of deadlocking.
- Inside a run: scoped threads for background sub-agents (up to 16), batches of parallel reading tools (8 at a time), script calls, prompt and agent hooks and the progress watcher of a running tool. Between runs the engine keeps up to eight live sessions (`engine/live/mod.rs`), least recently used first out; a session in the middle of a run is never let go. A live session holds its cache and its MCP pool behind mutexes; the session's hold keeps its runs one at a time. A session pushed out ends on a short-lived `agent-engine-session-end` thread; the sessions still live end when the engine is dropped, on the dropping thread.
- Async: the model client (`ModelApi`, `Http::post_streaming`, the stream body, retries), the HTTP hook and the MCP HTTP and SSE transports (`Http::post_cancellable`, `Http::get_stream`) are async and select on the run's cancellation token; model streams have an idle deadline instead of a total one. Permission questions, every MCP call (a bridged channel's included, which gets the run's own `Stop`), command hooks (`platform::bounded::run_cancellable`, which kills the child the moment the token is cancelled) and prompt or agent hooks wait on the token too, from synchronous code. An MCP HTTP or SSE server's stream listener is a task of the engine runtime, ended when the server is closed or dropped (its session's pool closes it when the session ends).
- Network edges: model calls and their OAuth refresh, MCP over HTTP and SSE, web fetch, web search and the `ctx.fetch` of a script tool or hook are async and race the run's token, request and body alike (`Http::post_cancellable`, `one_hop`, `request_cancellable`, `Body::text`, `Body::capped`); a synchronous tool waits on them with `web::under`, which ends the wait the moment the token is cancelled. No network path polls a flag. A fetch's name resolution (the SSRF guard's address check) is still a blocking call that a cancel waits for.
- Still polling: the exit of a child process, noticed by a poll of 1 to 20 ms (a cancel no longer waits for it); the script interrupt handler, which reads the run's flag.
- Not yet tasks: background sub-agents and the parallel reading batch stay scoped threads of the run's blocking thread, because they borrow the run's `Agent` and `ThisRun`; they are cancelled with the run and joined before it ends.
- Scripts: a new QuickJS runtime per invocation, on its own thread; its host calls come back through one bridge (`tools/script_bridge.rs`).
- Cancellation: the run's `Stop` (`crates/platform/src/stop.rs`), held by its `Control` and `Workspace`: a token that async waits select on and a flag that synchronous code checks, both set at once. Cancelling a parent cancels its children; a hook's timeout cancels a child token of its own, never the run.
- Channels: synchronous code passes messages over `crossbeam-channel` (its `select!` serves the client's run, which waits on its events and on the reply to its start at once), async code over `tokio::sync`. The REPL's JavaScript context (`crates/script/src/session.rs`, `crates/agent-engine/src/agent/repl.rs`) still uses `std::sync::mpsc`.
- HTTP servers: every one is `axum` on Tokio. The console serves the page on its own runtime; the loopback a provider or MCP sign-in redirects the browser to (`crates/app/src/providers/redirect/loopback.rs`) is a task of the engine runtime, handing each request to the waiting sign-in over a channel and answering with what it sends back, and it lets go of its port before its drop returns; the relay is its own binary.
- Shared state: about a dozen process-global maps keyed by client number (inbox, peers, shells, schedules, channels, memory). Poisoned mutexes are tolerated through a helper in `platform`.

## Child processes

Every process the application starts and keeps a handle on is a `platform::child::Child` (`crates/platform/src/child.rs`): Bash shells, monitors and affected-test runs, MCP stdio servers, language servers, and everything that goes through `platform::bounded::run` or `run_cancellable` (hook commands, git helpers, installers, the status line). The web terminal's pseudo-terminal child is the one exception; it kills and waits by itself (`crates/console/src/pty.rs`). A child is started in its own process group (a job object on Windows) so that what it starts ends with it. `terminate(grace)` asks the group to stop with SIGTERM, waits up to the grace, then sends SIGKILL, and always waits for the child, so none is left as a zombie; on Windows there is no polite stop and the job is terminated at once. Dropping a child that is still running sends the polite stop and hands it to one `child-reaper` thread, which kills it after two seconds and waits for it, so a drop never blocks. Whether the child has exited is read without reaping it (`waitid` with `WNOWAIT`), and the group is signalled only while the child is unreaped: its process id cannot have been given to another process. What an exited child left running in its group is killed before the child is reaped, except for Bash shells, whose background processes are meant to stay. A child started with `Stderr::Tail` has the last 64 KiB of its error stream kept, which is what an MCP server or a language server that dies is quoted from. Above that: a language server is sent `shutdown` and `exit` before it is let go and is tried again 30 seconds after a failed start; an MCP stdio server whose call is cancelled or times out is sent `notifications/cancelled` and is ended only if it then says nothing for three seconds (a channel's server is told and never ended for that); an MCP server a run connects lives in its session's pool (`crates/agent-engine/src/mcp/pool.rs`) until the session ends or the next run finds its entry changed, unapproved or its process dead, and is then terminated with the grace and waited for; a finished background shell leaves the list once its output has been read to the end, or after an hour.

## Target

The plan runs in eleven phases on `main`. The decisions that change this document:

| Change | Record |
|---|---|
| A session object lives in the engine across runs; MCP servers are pooled (done) | `decisions/0001-long-lived-session.md` |
| `Job::run`, `converse` and `run_loop` become typed phases and a `TurnState` | `decisions/0002-agent-loop-state-machine.md` |
| Tools declare effects in Rust; one policy engine; a real shell parser (done) | `decisions/0003-effects-based-permission-policy.md` |
| An untrusted project folder loads nothing that executes (done) | `decisions/0004-workspace-trust.md` |
| `sdk`, `protocol` and `backend` merged into one typed `app` crate with correlation ids (done) | `decisions/0005-typed-application-api.md` |
| One document helper, SQLite for catalog and memory, transcript split from trace and saved per transition | `decisions/0006-one-persistence-pattern.md` |
| One Tokio runtime in the engine with structured cancellation (8.1 and 8.2 done, 8.3 in part) | `decisions/0007-one-async-runtime.md` |
| The owner concept is removed; on-disk names stay as they are | `decisions/0008-single-user-local-product.md` |
| Function-size lint instead of a file-length limit; `impl` blocks regrouped | `decisions/0009-function-size-lint.md` |
| Rationale lives in `docs/` | `decisions/0010-comments-stay-out-of-code.md` |

Provider folders, the model client, wires and the transcript types have moved from `agent-engine` to `llm-providers` (done).

Closed sets are enums instead of strings (done): tool names (`ToolName`), hook events and handler kinds (`HookEvent`, `HandlerKind`), permission decisions, trace marks and note titles, and a background start or end is its own run event (`BackgroundStarted`, `BackgroundEnded`) instead of text parsed from a tool's output. `PromptRequest` is four parts (`Prompt`, `SessionConfig`, `Policy`, `HostContext`) with the same JSON, and code that only needs a folder takes a `Folder` or a path. Failures map explicitly from one layer to the next: `ProviderError` → `ModelError` → `RunFailure` → the engine's `Stop` → the run's `Failure` and `Failed` events → the client's `Error`; MCP transports return `McpError`. All of these are in `GLOSSARY.md`. Done without a record of its own: `domain`'s top-level modules are grouped into eight areas (see the crate map).
