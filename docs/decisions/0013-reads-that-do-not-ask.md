# 0013 Reads that do not ask: absolute paths, reads outside, MCP tools marked read-only

Status: Accepted, 2026-11-17

## Context

In accept-edits mode the owner was asked again and again about commands that only read. The sessions showed three causes:

- A read-only command naming an absolute path inside the project (`file /proj/app.exe`, `ls -la /proj/`) was not read-only: the classifier refused every word starting with `/`, since it did not know the project's folders.
- A read-only command reaching outside the project (`ls ~/Library/...`, `strings /opt/x`) was an opaque command: it asked with a `Bash(ls:*)` rule, which, once remembered, allowed `ls` anywhere, and which asked again for every other program reading the same folder. The `Read` tool, reading the same file, asked about the folder instead.
- Every MCP tool counted as acting on a remote server, including searches the server itself declares read-only (`annotations.readOnlyHint`).

## Decision

**The classifier knows what the session may read.** `command_policy::classify` takes the folders the session may read (`Bounds::may_read`: the project's roots and the extra readable folders). An absolute word inside them counts as inside the project. Paths are compared as written, before symlinks are resolved, as relative paths already were. Matching a command against permission rules stays strict: a word starting with `/` never "goes along" with an allowed command.

**A read outside is a read outside, whatever tool does it.** When a Bash command is opaque only because it names paths outside those folders (or `~`, expanded to the user's home at the start of a word), and would otherwise be read-only, the call stands as `ReadsOutside`, like a `Read` of the first such path: it asks with `Reason::Outside`, suggests `Read(//<folder>/**)`, and is judged by `Read` rules as well as `Bash` rules, the stricter of the two winning. Remembering the answer allows reading that folder by `Read` and by commands alike; a `Read` deny rule refuses the command.

**An MCP tool its server marks read-only is a read.** A tool listed with `annotations.readOnlyHint: true` (the boolean, nothing else) declares no effect, as the app's own memory search does. Any other tool still declares `Effect::Remote` and asks. Deny and ask rules naming the tool still apply.

## Consequences

- Reading in the project by any path, and reading outside it once per folder, no longer asks in accept-edits mode. Plan mode asks about reads outside instead of refusing them, as it already did for `Read`.
- The owner's answer to a read outside is reusable across tools.

What gets worse:

- `readOnlyHint` is server metadata, which the threat model counts as untrusted. A server the owner approved can mark a tool read-only and have it run without a question, and a call still sends its arguments to the server: a read-only MCP tool is a way out for data the model has seen. The owner keeps an `ask` or `deny` rule per tool or per server (`mcp__<server>`) for servers they do not trust that far.
- Paths are compared as written: a symlink inside the project pointing outside reads outside without a question, as a relative path through it already did.
