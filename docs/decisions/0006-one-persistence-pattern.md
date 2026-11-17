# 0006 One persistence pattern

Status: Accepted, 2026-11-16. The SQLite part (session catalog and project memory) is implemented, 2026-11-17; see "Implemented" below.

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

## Implemented: the database (2026-11-17)

The session catalog and the project memory are in one SQLite database per data folder, `app.db`, behind the `SessionCatalog` and `MemoryStore` traits (`crates/app/src/db/`, `crates/app/src/catalog/sqlite.rs`, `crates/app/src/features/memory/store/sqlite.rs`). SQLite is compiled into the binary (`rusqlite` with `bundled`).

- The database is opened in WAL mode with `synchronous=NORMAL`, `foreign_keys=ON` and a ten-second busy timeout. While the program runs, `app.db-wal` and `app.db-shm` sit beside `app.db`; all three are the user's alone (0600).
- The schema has a version (`schema_version`) and a list of forward-only migrations applied in one transaction at open. A database written by a newer version is refused with an error that says so and is not touched; the program then keeps sessions and memory in the process only, and logs it.
- The domain rules did not move. A change is `BEGIN IMMEDIATE`, load the catalog (or the project's book), apply the same `domain` function as before, write the rows that differ, `COMMIT`. A change that changes nothing writes nothing.
- Each project's memory has a revision that every change bumps. The copy of a book a process keeps is used only while its revision is the one in the database, so what another process wrote is read, not what this one remembers reading.
- The point `/memory undo` goes back to is a row in the database: it survives a restart and is ended by an edit made in any process.
- Import, once, at the first open of a data folder: if the database has no sessions and `users/local/sessions.json` exists, its sessions are imported in one transaction and the file is renamed `sessions.json.imported`; if the database has no memory and `memory/local/*.json` exist, every book is imported in one transaction and each file is renamed `<project>.json.imported`. The check and the import are inside one `BEGIN IMMEDIATE` transaction, so two programs starting together import once. Nothing is deleted. A file that does not parse is renamed `<name>.corrupt-<epoch seconds>` by `Document`, as before, and the rest is imported. The older source, `conversations` in the UI state, is still read when the catalog is empty.

Consequences that are now real:

- Building needs a C compiler for one more dependency (the bundled SQLite), and a clean build compiles the SQLite amalgamation once.
- Downgrade: an older binary started after the import finds `sessions.json.imported` and `<project>.json.imported` where it expects `sessions.json` and `<project>.json`, so it shows an empty session list and an empty memory. Nothing is lost: quitting and renaming the files back (dropping `.imported`) gives the older binary the data as it was at the moment of the import. What the newer binary wrote after the import is only in `app.db` and the older binary does not see it. Going forward again, the newer binary does not import a second time, because its database is not empty; it leaves the renamed-back files where they are.
- The catalog and the memory are no longer files to open in a text editor; `sqlite3 app.db` reads them.
- Tests and the in-memory data root use an in-memory SQLite database, so the same SQL runs in both; the in-memory implementations of the two traits remain as test doubles and as the last fallback, and one shared test drives every implementation through the same script.
