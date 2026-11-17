# 0007 One async runtime in the engine with structured cancellation

Status: Accepted, 2026-11-16

## Context

The engine is synchronous code on operating-system threads: one thread per run, scoped threads for background agents, batches of reading tools, scripts and hooks. Async exists only at the edges and is consumed by blocking code:

- Model HTTP runs on a global two-worker Tokio runtime; the synchronous side blocks on it and wakes every 25 ms to check a stop flag (`crates/agent-engine/src/model/cancellable_http.rs`).
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
