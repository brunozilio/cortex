# 0001 Long-lived session in the engine

Status: Implemented, 2026-11-17 (accepted 2026-11-16)

## Context

The engine kept nothing between prompts. `NativeEngine::start` in `crates/agent-engine/src/engine/mod.rs` created a new `Workspace`, `Control` and `Job` for every run, and `Job::run` in `crates/agent-engine/src/engine/job.rs` then rebuilt everything:

- the Brief (`crates/agent-engine/src/brief.rs`): plugins, settings, skills, agents, rules, instruction files, with a directory walk and three `git` processes;
- the tool list, including discovery of script tools;
- every MCP connection (`crates/agent-engine/src/mcp/`), so stdio servers were spawned again on each prompt and their `Drop` killed them without waiting;
- every script hook, each in a new QuickJS runtime.

This was visible latency before the first token, and it was the source of leftover MCP processes.

It also made two hook names wrong. `SessionStart` and `SessionEnd` fired once per run, so a hook author who expected "once per session" got "once per prompt".

## Decision

The engine holds a live session for each session it has run, as long as the conversation stays in the process.

- `Engine` (then `NativeEngine`) owns `LiveSessions` (`crates/agent-engine/src/engine/live/mod.rs`): at most eight `LiveSession`s, least recently used out first. A run enters its session's live session once `load_session` has loaded it (and taken the session's hold); a chore, a rewind or a branch enters none. A live session in the middle of a run is never pushed out; the bound is exceeded until it is idle.
- The session cache (`engine/live/cache.rs`) keeps the Brief, with its settings and hooks, and the script tools. Before each run it takes a fingerprint (`engine/live/sources.rs`) of the files they are read from: path, modification time, size and, for files up to 64 KiB, a hash of the content, plus the run's folder, host extensions and trust. The same fingerprint reuses what was loaded; a different one loads it again at the start of that run, never in the middle of one. More than 50 000 watched files means no caching: the Brief is read at every run, as before. The built-in tools are still built per run; they read nothing.
- MCP servers live in the session's MCP pool (`crates/agent-engine/src/mcp/pool.rs`). Each run computes the servers its settings, trust and approvals allow, exactly as before, and asks the pool for them: an unchanged entry whose process is alive is reused, a new or changed entry is started, and an entry that is gone, changed or no longer approved is terminated (polite stop, two seconds' grace, kill) and waited for. In-process servers (the host's own and script servers) are opened per run: they start no process, and a script server carries the run's own judge.
- A live session ends when it is pushed out (on a short-lived thread) or when the engine is dropped. There is no idle timeout: the bound of eight is what limits what a long terminal session keeps, and an idle timer would need a thread of its own for little gain. Ending runs `SessionEnd`, if `SessionStart` fired, with the hooks and folder of the last run, then closes the pool.
- `SessionStart` fires once per live session, on the first run that reaches the prompt. `RunStart` (same input, context added to the prompt like `SessionStart`) and `RunEnd` fire once per run, and took over what the old pair did.
- Runs of one session stay one at a time through the session's hold, as before: a second `start` on a busy session is refused with "answering in another window". `AgentEngine::start` stays synchronous; all of the above happens on the run's thread.
- The session is still loaded and saved through the store of `0006-one-persistence-pattern.md`; the turn state machine of `0002-agent-loop-state-machine.md` stays per run.

## Consequences

- A second prompt in the same session skips the Brief, its hooks, the git snapshot, the script tools' parsing and every MCP start; what is left per run is the fingerprint walk (the same directory walk the Brief did for nested instruction files, without reading or parsing) and `tools/list` on the kept connections.
- MCP servers have one owner with a defined shutdown, so child processes are waited for.
- Hook names match what they do.

What gets worse:

- Existing hooks that relied on `SessionStart` or `SessionEnd` firing on every prompt changed behaviour and must move to `RunStart` or `RunEnd`. The user documentation (`web/docs/*/hooks.md`) says so. A `Brief` hook now runs only when the Brief is loaded again, not at every prompt.
- A cache can be stale. The fingerprint misses any input that is not a file: environment variables, git state (the git snapshot shown to the model was already the session's first), a change to a file outside the watched folders that a skill or an instruction import reads. A change inside the same second on a file system with coarse timestamps is still seen for files up to 64 KiB through the content hash. `/reload` does not reach the engine; a new session is the escape for what the fingerprint cannot see.
- The process holds more state and more open connections for longer; an MCP server that leaks now leaks for the whole session instead of one run, and up to eight sessions' servers stay running. A session ended when the process exits without dropping the engine (a crash, a kill) leaves its stdio servers to the operating system and never runs `SessionEnd`.
- Per-run isolation is partly lost: a bug that corrupts a live session's cache or pool affects later runs, where every run used to start clean.
