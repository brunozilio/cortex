# 0001 Long-lived session in the engine

Status: Accepted, 2026-11-16

## Context

The engine keeps nothing between prompts. `NativeEngine::start` in `crates/agent-engine/src/engine/mod.rs` creates a new `Workspace`, `Control` and `Job` for every run, and `Job::run` in `crates/agent-engine/src/engine/job.rs` then rebuilds everything:

- the Brief (`crates/agent-engine/src/brief.rs`): plugins, settings, skills, agents, rules, instruction files, with a directory walk and three `git` processes;
- the tool list, including discovery of script tools;
- every MCP connection (`crates/agent-engine/src/mcp/`), so stdio servers are spawned again on each prompt and their `Drop` kills them without waiting;
- every script hook, each in a new QuickJS runtime.

This is visible latency before the first token, and it is the source of leftover MCP processes.

It also makes two hook names wrong. `SessionStart` and `SessionEnd` fire in `crates/agent-engine/src/engine/converse.rs` once per run, so a hook author who expects "once per session" gets "once per prompt".

## Decision

The engine holds a `Session` object that lives as long as the conversation is open in the process.

- The session caches the Brief, the tool list and the parsed hooks, and invalidates a cached item when the modification time of a file it was read from changes.
- MCP servers are pooled per session and stay connected across runs.
- `SessionStart` and `SessionEnd` fire once per session. Two new events, `RunStart` and `RunEnd`, fire once per run and take over what the old pair did in practice.
- The session is the home of the turn state machine (`0002-agent-loop-state-machine.md`) and is loaded and saved through the store defined in `0006-one-persistence-pattern.md`.

This is phase 7.4 of the plan and lands after the loop has been split into typed phases.

## Consequences

- A second prompt in the same session skips discovery and reconnection.
- MCP servers have one owner with a defined shutdown, so child processes are waited for.
- Hook names match what they do.

What gets worse:

- Existing hooks that rely on `SessionStart` or `SessionEnd` firing on every prompt change behaviour and must move to `RunStart` or `RunEnd`. This is a breaking change for users' hook files and has to be called out in the user documentation.
- A cache can be stale. Invalidation by modification time misses a change that keeps the same timestamp and any input that is not a file (environment, git state). `/reload` remains the manual escape.
- The process holds more state and more open connections for longer; an MCP server that leaks now leaks for the whole session instead of one run.
- Per-run isolation is lost: a bug that corrupts session state affects later runs, where today every run starts clean.
