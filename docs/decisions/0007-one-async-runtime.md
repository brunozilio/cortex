# 0007 One async runtime in the engine with structured cancellation

Status: Accepted, 2026-11-16; stages 8.1 and 8.2 done, 8.3 done except background sub-agents and the parallel reading batch, 2026-11-17 (see the end)

## Context

The engine is synchronous code on operating-system threads: one thread per run, scoped threads for background agents, batches of reading tools, scripts and hooks. Async exists only at the edges and is consumed by blocking code:

- Model HTTP runs on a global two-worker Tokio runtime; the synchronous side blocks on it and wakes every 25 ms to check a stop flag (`crates/llm-providers/src/model/cancellable_http.rs`).
- The console has a second, independent Tokio runtime (`crates/console/src/server.rs`).
- Script calls are served by a loop with the same 25 ms timeout.

Cancellation is a shared `AtomicBool` that each layer polls. The audit ties several findings to this shape: a hook's timeout sets the run's own stop flag and cancels the whole run; a model stream has a 900-second total deadline instead of an idle deadline; bytes may be lost when a chunk splits a multi-byte character across a poll; cancelling one MCP call kills the server; retries ignore `Retry-After`. The binary also carries three channel libraries and three HTTP stacks.

## Decision

The engine owns one Tokio runtime and uses structured cancellation.

In scope, made async with a `CancellationToken`:

- `Http` and `ModelApi`, including streaming, an idle deadline on the stream, `Retry-After` and jitter;
- the agent loop: a run is a task, and waiting for an answer selects on the token;
- MCP transports, `command` and `http` hooks, web fetch, background sub-agents, the batch of parallel reads;
- one script-host bridge instead of two.

Out of scope, kept synchronous and run on blocking threads: file tools, grep, the QuickJS runtime and shells. They gain nothing from async.

The engine's start call stays synchronous so the TUI, which is a synchronous loop reading a channel, does not change.

This is phase 8 of the plan, after the loop is a state machine.

## Consequences

- Cancellation is scoped: cancelling a hook, a tool or one MCP call cancels that and nothing wider.
- The 25 ms polling loops and the wake-up latency they add are removed.
- Timeouts can be idle timeouts, and retries can honour the server.

What gets worse:

- The codebase becomes mixed. Async code calls blocking code through blocking threads and blocking code is called back from scripts; getting the boundary wrong (a blocking call on a runtime worker) stalls every run in the process.
- `async` spreads through traits (`ModelApi`, `Http`, MCP `Transport`) and all their test doubles; every scripted-model test changes.
- Stack traces and panics inside tasks are harder to read than a thread's, and a panic no longer maps to "one run, one thread".
- The synchronous start call hides a runtime behind it, so callers can still block on the engine from a place where blocking is wrong.
- The console keeps its own runtime until the server stacks are unified in phase 9.4.

## Status by stage

Updated 2026-11-17.

### 8.1 Model HTTP async: done

- `ModelApi::turn` returns a boxed `Send` future and takes a `CancellationToken`; `Http` gained `post_streaming`, async with the token. Synchronous callers go through `BlockingTurn::turn_blocking`, which waits on the engine runtime. Test doubles that only implement `Http::post` still work, since `post_streaming` falls back to it.
- The stream body (`model/body.rs`) is read with async IO; every wait races the token and the idle deadline. The 25 ms polling loops of the model path are gone: in the stream reader, in the request's wait for headers, in the retry sleep and in the per-account turn lock (now an async mutex). An idle deadline (300 s, `<PREFIX>_STREAM_IDLE_SECONDS`) replaces the 900 s total deadline and fails as the typed `ModelError::Stalled`. A line split across chunks is decoded only once whole.
- Retries keep their policy (four attempts, only when nothing was shown, doubling from the wire's backoff) and add jitter (half to all of the wait) and `Retry-After` (`retry-after-ms`, seconds or a date; more than 60 s is not waited for).
- The run's cancellation is `platform::Stop`: a token plus the flag the code not yet moved reads, set together. It dereferences to the flag, so the many functions that still take `&AtomicBool` accept it unchanged; they are moved one by one.

### 8.2 One runtime, a run is a task: done

- One multi-threaded runtime per process (`platform::runtime`), built on first use with two to four named workers and at most 256 named blocking threads; `NativeEngine` takes its handle when built, and model HTTP uses the same one. Building it per engine would have split the HTTP connection pool across runtimes.
- `AgentEngine::start` stays synchronous and spawns the run as a task; the agent loop is still synchronous code and runs on a blocking thread of the runtime, so a panic is the task's error and becomes the run's failure. Live sessions and their MCP pools are untouched.
- A permission question or an `AskUserQuestion` waits on a oneshot answer selected against the run's token (`control::Awaited`), so cancelling ends the wait at once.

### 8.3 The rest of the edges async: done but for two

Done:

- The `http` hook is async and selects on the run's token, the hook's timeout and its own child token; a prompt or agent hook's worker waits the same way. A hook's timeout ends the hook only, never the run.
- The MCP stdio call waits on a oneshot answer selected against the run's `Stop` and its deadline instead of polling every 25 ms. The MCP `Transport` takes a `Stop`.
- One host bridge for scripts (`script_bridge::hosted`) replaces the two loops that polled every 25 ms; the script's end is a message, so the wait ends with it.
- A running tool's progress watcher wakes when the tool ends instead of finishing a 150 ms sleep, which held every tool call up to that long, cancelled ones included.
- The MCP HTTP and SSE transports are async: a request is posted with `Http::post_cancellable` and its answer read with the body's async `read_line`, both under the run's token; an SSE request waits on a oneshot answer selected against the token, its deadline and the stream going down. `Http::get_stream` is async and takes a token. Each server's stream listener is a task of the engine runtime, not a thread, and ends when the server is closed (the session's pool closes it when the session ends or its entry changes) or dropped; its reconnection pause is a sleep raced with that end. No 25 ms poll is left in MCP.
- A channel server bridged through a domain `McpEndpoint` is handed the run's own `Stop`: `McpEndpoint` has `Any` as a supertrait, so the in-process transport recognises the bridge and calls it with the `Stop` instead of the flag. `platform::stop::following`, which polled a flag every 25 ms, is gone.
- `command` hooks run through `platform::bounded::run_cancellable`, which races the child's exit, the hook's timeout and the run's token, and kills the child's group the moment the token is cancelled. The exit itself is still noticed by a poll of 1 to 20 ms; a cancel or a timeout no longer waits for it.

- The OAuth refresh of a model call is cancellable again. `Transport::config_under` takes the call's token and `oauth::refresh` is async: it posts with `Http::post_cancellable` and reads the answer with `Body::text`, both under the token. The cached configuration sits behind an async mutex, held across the refresh as the blocking one was, so two calls never refresh the same refresh token. A turn, a provider web search and the check before a turn all pass their token; only the model catalog, which has none, refreshes with a token nothing cancels.
- Web fetch, web search and script HTTP are async under the run's token. `Http` lost `one_hop` and `streamed` as blocking calls and `post_stopped` altogether; it has `one_hop` (the client that follows no redirect and resolves only public addresses) and `request_cancellable` (the ordinary client), both async with a token. `reach::fetch` is async; the guard is unchanged: the name is refused, then its resolved addresses, before every hop, and a redirect is followed one hop at a time. A body is read with `Body::capped`, which stops at the cap and races the token, instead of `read_capped`, which woke every 25 ms. A tool, which is still synchronous, waits on all of it with `web::under`, which races the whole fetch or search with the token, so a request that does not watch the token itself still cannot hold a cancelled run. `Searcher::search` is async with a token; the provider search takes the account's turn with `take_unless` instead of blocking on it. A script's `ctx.fetch` gets the run's token when the script is a tool or a hook; a skill script, a script MCP server and a declared provider have no run, so theirs is a token nothing cancels, as before (the request still ends at 15 s).
- With no caller left, the blocking 25 ms paths are deleted: `cancellable_http::{post, send, unstoppable, one_hop}`, `read_line_stopped`, `Response::text_stopped`, `http::Stopped` and `body::LEGACY_POLL`. Synchronous code that still reads a body through `BufRead` (the model catalog, sign-in, a package lookup) waits on the next chunk with `block_on` instead of a 25 ms timeout in a loop; those bodies are already whole when they are read.

Not done:

- Background sub-agents and the batch of parallel reading tools are still scoped threads inside the run's blocking thread, cancelled with the run (their controls are the run's children, the readers share its `Stop`) and joined before it ends; neither polls. Tokio tasks must be `'static`, and both borrow the run: `Agent<'a>` holds `&'a dyn ModelApi`, `&'a [Box<dyn Tool>]`, `&'a Workspace`, `&'a Control`, `&'a (dyn Fn(EngineEvent) + Sync)` and `&'a [FoundAgent]`, and a reader also borrows the run's `ThisRun` (tool index, repeats, watches, the background list). Making the closures `'static` means `Agent` and `ThisRun` owning those behind `Arc`s, which changes the thirty places that build an `Agent` (eleven test files among them) and every function that takes `&Agent`, for no change in what the user sees: the threads already end on cancel and are joined. It was left out of this stage to keep the change reviewable; an unsafe scoped spawn is not taken.
- The agent loop itself is synchronous; making `Tool::run` async is what would let it become a task proper, and with it the two items above.

What still polls, and why:

- The exit of a child process, noticed by a poll of 1 to 20 ms (`platform::child`): waiting without reaping (`waitid` with `WNOWAIT`) has no async form; a cancel or a timeout does not wait for it.
- The script interrupt handler: QuickJS asks the host at intervals whether to stop; it reads the run's flag, which is what the flag view of `Stop` still exists for.
- Not a poll, but not cancellable either: name resolution before each hop of a fetch (`Http::addresses`, the SSRF guard's check of resolved addresses) is a blocking call on the tool's thread, so a cancel that lands during a lookup waits for it to end; the system resolver's own timeout bounds it. Moving it to `tokio::net::lookup_host` would change the guard's code, which this stage kept as it was.

Measured with `crates/agent-engine/tests/cancel_latency.rs` on an 18-core Mac (debug build, so four runtime workers), cancel to the run's terminal event: a stalled MCP stdio call 45 ms before, under 1 ms after; a run waiting for an answer under 1 ms before and after (dropping the sender already woke it); a stalled model stream under 2 ms before and after. Threads in a run with three MCP servers and a background sub-agent: 10 before, 14 after; the four runtime workers now exist from the first run instead of from the first model request.

The second part of 8.3, measured the same way, three runs each: a stalled MCP call over HTTP 3 to 6 ms before, under 1 ms after; a command hook still running 12 to 18 ms before, under 1 ms after, its process gone; a run whose background sub-agent waits on its model under 2 ms (unchanged). In the unit tests, measured from the cancel: an SSE request waiting for its answer and an SSE server naming its endpoint end in under 1 ms (they polled every 25 ms before); closing a listened HTTP or SSE server lets its stream go at once (before, closing an HTTP server did not end its listener until the last handle on it was dropped). Threads in a run with three MCP stdio servers and a background sub-agent: 14, as before.

The last part of 8.3, measured from the cancel in the unit tests, three runs each: an OAuth refresh of a model call against a server that never answers, 0.1 to 0.2 ms (before, it ran into the request's 15 s timeout and ended as a network error, not a cancel); a web fetch stalled before its headers or in its body, a web search stalled in the account's search and a script's `ctx.fetch` stalled, owner's or project's, all under 0.4 ms (before, the fetch and the script's request were not cancellable at all and the body read woke every 25 ms); `one_hop` and `request_cancellable` against a silent server, 0.1 to 12 ms.

What it cost, beyond the above: every `ModelApi` implementation and test double changed shape; the flag view of `Stop` keeps a second way to read cancellation alive until the last `&AtomicBool` is gone; a synchronous wait inside an async one is served on a fresh thread, which is correct but costs a thread for that wait. The second part of 8.3 changed `Http::get_stream` to an async call and every MCP test double to a body of chunks instead of a reader that blocked a worker for 25 ms at a time; `McpEndpoint` requiring `Any` means an endpoint must own its data. The last part made `Searcher::search` and `Http::one_hop` async, so their test doubles changed shape; a synchronous tool waiting on the network goes through `block_on` once per call; `StandaloneHost` carries a token next to its flag.
