# 0006 One persistence pattern

Status: Accepted, 2026-11-16. The SQLite part (session catalog and project memory) is implemented, 2026-11-17; see "Implemented: the database" below. The transcript part (conversation split from the trace, saved at every transition) is implemented, 2026-11-17; see "Implemented: the transcript" below. The keychain part is implemented, 2026-11-17; see "Implemented: the secrets key in the keychain" below.

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

The audit proposed SQLite for trace and UI state as well; this decision narrows it to catalog and memory.

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

## Implemented: the transcript (2026-11-17)

A session is two append-only files in `native-sessions/` (`crates/agent-engine/src/sessions/`): `<hash>.jsonl` holds the conversation (`message`, `truncated`, `restarted`) and `<hash>.trace.jsonl` holds the trace (`prompted`, `shown`, `opened`, `event`, `called`). The conversation kept its file name, so an older and a newer binary find it in the same place.

- Every line written since the split carries `seq`, a number that only grows within a process and starts from the clock in microseconds, so it also grows from one process to the next; a process that loads a conversation never numbers below what it read. Readers of the trace merge the files by it. A line with no number takes the number of the line before it in its file. That gives three cases, all read for good: an old mixed file alone reads in the order it was written; an old mixed file that grew afterwards keeps its old part first, then the new conversation lines in it and the new trace lines beside it in the order they were written; two new files read as one log.
- Resuming reads `<hash>.jsonl` only. A trace file that is missing, unreadable or does not parse costs the trace and a warning in the log, never the conversation; a trace line that does not parse inside an old mixed file is skipped the same way. A conversation line that does not parse still fails the load, as before.
- The conversation is saved at every transition of the turn state machine (`0002-agent-loop-state-machine.md`): before each model call, after each reply is recorded, after each batch of tool results. The save in `close` remains and normally writes nothing. A run killed between two transitions resumes with everything up to the last one, and a tool call left without its result is closed as interrupted.
- A save is one `write` of the new lines and one `sync_data`. The store keeps, per session, the number of messages persisted, a digest of them and the file's length, and appends only what follows when the file still has that length and the digest still matches; otherwise it reads the file once. The conversation is hashed in memory at each save, the file is not read.
- An already saved message that changes (a reminder joining tool results, a prompt joining the results an interruption left) is written as `truncated` to the first changed message, marked `amended: true`, followed by the new version. An older binary reads it as the truncation it is; the trace does not show it as a rewind.
- The trace goes through one buffered file per run: opened once, flushed at each transition and at the terminal event, never synced. Past 64 MiB it is renamed `<hash>.trace.1.jsonl`, replacing the earlier one, and a new file starts.
- Housekeeping done with it: the session's `.lock` file is removed on release; checkpoints are added in one `Document::update` and capped (the newest 50 and 64 MiB of file content keep their files, older ones keep their place only); tool-result folders in the temporary directory are removed after seven days without a write.

Consequences that are now real:

- One `fsync` per transition, two per turn with tools plus one for the prompt and one for the answer. Measured on the development machine (APFS, release build): about 3.6 ms per synced append and about 0.4 ms to hash a 225 KB conversation, against the single unsynced write per run it replaces. A turn with tools costs about 8 ms more than before.
- Hashing the conversation at every save is work proportional to the conversation, not to what was added; only the bytes written are proportional to what was added.
- Two files per session instead of one, three after a trace rotation. Copying a session by hand means copying all of them; deleting only the trace files is safe.
- A trace older than the last two 64 MiB parts is gone.
- Downgrade: an older binary opening a session written by this one finds the conversation in `<hash>.jsonl` and resumes it (unknown fields are ignored and an amendment is a truncation), but it reads no `<hash>.trace.jsonl`, so its trace view shows that conversation as one saved before traces existed. What it then appends, conversation and trace mixed and unnumbered, is read back by this version in file order.
- An older binary does not expect a session's `.lock` file to disappear: if it opens the file just before this version removes it and locks it just after, both can hold the session at once. The window is a few microseconds at the end of a run.
- The hooks' view of the disk changed: a hook that reads the session files during a run now finds the messages of the turn so far, and trace entries only up to the last transition.
- A checkpoint whose files were dropped cannot be rewound to. An older binary does not know the marker and would rewind to it without restoring files.

## Implemented: the secrets key in the keychain (2026-11-17)

Only the vault's 32-byte key moved to the operating system's keychain (macOS Keychain, the Secret Service over D-Bus on Linux, Windows Credential Manager); the entries stay sealed in `secrets.json` as before (`crates/app/src/secrets/key.rs`, `crates/platform/src/keychain.rs`). The alternative, one keychain item per secret, was set aside: it changes the data format and needs a migration of every entry, macOS asks once per item when a rebuilt binary first reads it, and an MCP server's or a subscription's tokens are rewritten on every refresh, which would be a keychain write each time. With one item, the vault, its locking and its tests are unchanged and the keychain is touched once per process.

- The item's service is the app's name and its account is the full path of `secrets.key`, so two data folders never share a key.
- `secrets.key` becomes a pointer: after the key is in the keychain the file holds the word `keychain` and no key material. The file is rewritten (atomically, under the vault's lock) only after the key was written to the keychain and read back equal; a read-back that fails or differs leaves the file key where it was, and the next start tries again.
- A first start with a reachable keychain makes the key there and writes the pointer. A first start with no reachable keychain (a headless Linux, CI, a container) keeps the key in the file, as before, and moves it the first time a keychain answers. A key found in the keychain with no `secrets.key` beside it is used, not replaced.
- Once the pointer is written, a keychain that cannot be reached, or that no longer holds the item, is an error: no new key is made, since it would leave every sealed entry unreadable. The error names `<PREFIX>_SECRETS=file` as the way out; it reaches the user as the existing "keys cannot be kept" notice, and connecting a provider or signing in is refused until then.
- `<PREFIX>_SECRETS=file` keeps the key in the file and never writes the keychain. If the file is a pointer, the key is taken back from the keychain when it answers; when it does not, a new file key is made, which is the fallback the error offered (keys saved before must be entered again).
- Only the user's real data folder uses the keychain. `DataRoot::under` (`Client::builder().data_dir`, tests) keeps the file key, so no test touches the user's keychain; the test against the real keychain is ignored by default.
- The choice and the creation run under the same cross-process lock as before, so programs starting together end with one key and one keychain write.

Consequences that are now real:

- On Linux the Secret Service is reached through `keyring` 3 with its pure-Rust D-Bus client (`zbus` 4); three of its crates are unmaintained (merged into `zbus` 6) and are listed in `deny.toml` with that reason. No C library is added.
- On macOS the item is trusted for the binary that created it; a rebuilt or updated binary is asked once by the system ("Always Allow").
- Downgrade: an older binary reads the pointer as a key that is not 64 hex digits and refuses to keep keys, without making a new one. Running this version with `<PREFIX>_SECRETS=file` once puts the key back in the file for it.
- A process running as the user can still ask the keychain for the key; the keychain may ask the user first (macOS) or not (an unlocked Secret Service collection).

## Changed 2026-11-17: keychain is opt-in

The vault key now stays in `secrets.key` by default; the keychain is used only with `<PREFIX>_SECRETS=keychain` (any case, trimmed). Reason: a binary rebuilt or reinstalled with cargo gets a new code signature, so macOS asked for the keychain password at every start, which the "Always Allow" consequence above did not prevent. The history above is kept; where it says the keychain is the default, read it as the behaviour with `=keychain`.

- Default: a `secrets.key` holding the pointer word `keychain` (from an earlier version) is taken back into the file once, with one keychain prompt on macOS, and the keychain is never written. If the keychain does not answer then, a new file key is made and keys saved before must be entered again.
- With `=keychain`: the behaviour described above (an existing file key moves after read-back, a pointer is left, a keychain that stops answering refuses a new key and points at `<PREFIX>_SECRETS=file`); any other value means file.
- Cost: by default the key sits beside the vault in a 0600 file, so a copy of the data folder opens the vault, and a same-user process reads the key without a prompt. Choosing the keychain costs a prompt after each rebuild on macOS.
