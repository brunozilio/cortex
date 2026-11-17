# 0005 A typed application API crate replacing sdk, protocol and backend

Status: Accepted, 2026-11-16

## Context

A call from the terminal to the engine crosses three crates: `sdk` (client), `protocol` (`ClientCommand`, `ServerEvent`, `Envelope`) and `backend` (handler). All of it is in one process, over channels and threads. The types are serialisable, but the only place they are serialised is one test.

The audit found the cost without the benefit:

- No correlation id. `crates/sdk/src/client.rs` matches a reply to a call by event type, under one global mutex, with a 120-second timeout. Any stray `Error` event fails whichever call is waiting. `SaveSettings` returns before anything is written and reports nothing on success.
- Half the API is untyped. Thirteen features go through `ClientCommand::Feature { feature: String, input: Value }` (`crates/protocol/src/command.rs`), so their requests, replies and silent defaults are JSON without a schema.
- Long flows are mismatched: sign-in times out in the client at 120 seconds while the backend waits 300 and keeps the port.
- `sdk` is described as the terminal's internal client in the working agreement and as an API to build on in its own manifest, and carries an embedding API (agents, skills, commands and MCP servers defined in code) that nothing uses.

## Decision

One crate, `app`, is the application API. It is in-process only.

- Every request, reply and error carries a correlation id. Success is an explicit reply. Unsolicited events travel on a separate channel from replies.
- Each of the thirteen features gets typed request and reply types, converted one at a time, each with a test that pins today's silent defaults before the change.
- Long flows (sign-in, a run) are cancellable streams, with one timeout owned by one side.
- The order is: fix correlation and typing in place first, then merge `protocol`, `backend` and `sdk` into `crates/app` as a pure move. The client the terminal holds is `app::Client` (it was `Sdk` until 2026-11-17). Session rules that sit in the client today (budget, plan, learning) move to `domain`.
- The embedding API is removed, before the merge; the in-process memory MCP path it shares is kept.

If a remote server becomes a real requirement, a transport is fitted to this API then.

## Consequences

- A reply cannot be attributed to the wrong call, and the global mutex goes away.
- Feature contracts are checked by the compiler.
- One crate boundary to cross instead of three.

What gets worse:

- The serialisable command and event boundary, and the test that exercised it over JSON, are given up. Adding a remote transport later is new design work, not a switch.
- `app` will be a large crate holding the handler, provider accounts, secrets, memory and the client; the compile-time isolation between those is lost unless modules are kept disciplined.
- Typing thirteen features is thirteen behaviour-sensitive changes; any default that was accidental becomes explicit and may differ.
- The crate table, the dependency rules and every import path change in one move commit, which conflicts with any work in flight on those crates.

Follow-up, after the merge: the request a run is started with is typed in four parts by who sets them (`Prompt`, `SessionConfig`, `Policy`, `HostContext`), flattened so its JSON is unchanged, and calls that needed only a folder take a `Folder` or a path instead of a request filled with dummy values. A run's failure crosses each boundary through an explicit conversion (`ModelError` → `RunFailure` → the engine's `Stop` → the run's events → the client's `Error`); see "Failure taxonomy" in `GLOSSARY.md`.
