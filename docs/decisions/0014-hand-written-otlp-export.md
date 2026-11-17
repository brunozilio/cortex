# 0014 The trace is exported to OpenTelemetry by a small hand-written OTLP/HTTP JSON exporter

Status: Accepted, 2026-11-17

## Context

Users who already run an observability stack want the app's runs beside their other traces. The trace already holds everything such a stack shows: each run is a tree of spans (model calls with tokens, cost and model, tool calls with names, sub-agents, waits) with start and end times, built by `domain::trace::runs`.

The usual way to export is the `opentelemetry` SDK with `opentelemetry-otlp`. That brings the SDK, its exporter crates, protobuf code generation (`prost`, `tonic` for gRPC) and their own async runtime expectations, for a feature that is off for almost everyone. The app's spans are also not live `tracing` spans: they are rebuilt from the trace file after a run ends, so the SDK's span lifecycle, sampling and context propagation would go unused.

OTLP over HTTP accepts a JSON body (`/v1/traces`, the protobuf message written as JSON with ids in hex and 64-bit numbers as strings), and every collector that matters accepts it.

## Decision

- `domain::trace::otlp` maps finished runs to one OTLP `ExportTraceServiceRequest` as JSON: pure, tested on its own. Trace and span ids are derived from the session id, the run index and the span id, so the same run always has the same ids. Attributes follow the GenAI semantic conventions where one exists (`gen_ai.usage.input_tokens`, `gen_ai.tool.name`...) and use the `<command>.` prefix otherwise. Prompt, answer, tool input and tool output text is never exported.
- `crates/app/src/features/trace/export.rs` holds the `Exporter`: a bounded queue fed by the handler's run sink at each terminal event (`try_send`, never blocking), one worker thread that reads the session's trace, batches what finishes within two seconds, and posts it with the existing plain HTTP client. Each run is sent once; the first run seen of a session is its latest one. A failure drops that request and is logged once at `warn`, later ones at `debug`.
- It is opt-in through `<PREFIX>_OTLP_ENDPOINT`, with `<PREFIX>_OTLP_HEADERS` for keys. No settings key: a collector is a property of the machine, not of a project, and an environment variable keeps a project's settings from sending a user's runs elsewhere.

## Costs

- No new dependency, but the OTLP JSON shape is written by hand and must be kept right by tests rather than by the SDK's types.
- No gRPC, no protobuf, no metrics or logs signals; only traces over HTTP.
- Runs are exported after they end, not while they run, and a run that never ends (the process is killed) is never exported.
- A collector that is down loses those runs: there is no retry or disk buffer.
