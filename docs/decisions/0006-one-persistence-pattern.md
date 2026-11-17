# 0006 One persistence pattern

Status: Accepted, 2026-11-16

## Context

Atomic replace with fsync and private permissions is the common write path (`crates/platform/src/private_file.rs`), and a cross-process file lock exists (`crates/platform/src/file_lock.rs`). But each store combines them differently, and several get it wrong:

- The session catalog (`crates/backend/src/catalog.rs`) has only an in-process mutex, and a corrupt file is read as empty and written over.
- UI state (`crates/backend/src/settings.rs`) has no lock; two instances overwrite each other and a failed read makes the TUI's saver rewrite from an empty document.
- MCP tokens, the user's `mcp.json` and `settings.local.json` are read, changed and written with no lock, some of them not atomically.
- Creating the secrets key has no lock, and any read error generates and overwrites a new key.
- The conversation is written only at the end of a run (`crates/agent-engine/src/engine/converse.rs`), so a crash mid-run loses it. One truncated line makes the session's log unreadable (`crates/agent-engine/src/sessions/log.rs`).
- Conversation and trace share one file; saving the conversation rereads the whole file, which grows with the trace, and a corrupt trace line blocks resuming.
- The session list has two sources of truth: the catalog and a key in the UI state.

## Decision

- One helper, `platform::store::Document<T>`: take the cross-process lock, read, change, write atomically, and set a corrupt file aside instead of overwriting it. It is adopted by the catalog, settings, MCP tokens, `mcp.json`, `settings.local.json`, checkpoints and the creation of the secrets key.
- SQLite (WAL mode, bundled) holds the session catalog and project memory, behind the existing traits. The current JSON files are imported once and renamed with an `.imported` suffix. The duplicate session list in the UI state stops being a source of truth.
- The transcript stays an append-only JSONL file, `<hash>.jsonl`, and the trace moves to its own file, `<hash>.trace.jsonl`.
- The transcript is saved at every transition of the turn state machine (`0002-agent-loop-state-machine.md`). Loading tolerates a truncated last line and repairs a tool use left without a result.
- Old files keep loading: a session file that mixes conversation and trace is still readable.
- UI state stays a file, written per key under the lock.
- Secrets go to the operating system's keychain, with today's encrypted file as the fallback.

This is phase 5 of the plan. The audit proposed SQLite for trace and UI state as well; the plan narrowed it to catalog and memory.

## Consequences

- Two instances no longer lose each other's updates.
- Killing the process mid-run keeps the conversation up to the last transition.
- A corrupt file is kept for inspection instead of silently replaced.

What gets worse:

- The SQLite import has no simple way back: an older binary run after the import sees an empty catalog and empty memory.
- SQLite is a new C dependency, which adds build time and one more thing that needs a C compiler.
- Catalog and memory stop being files a user can read or fix with a text editor.
- A write and an fsync per transition costs more IO than one write per run.
- Two storage technologies (documents and a database) remain, plus the JSONL logs; "one pattern" means one helper per kind, not one format.
