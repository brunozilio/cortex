# 0003 One effects-based permission policy

Status: Accepted, 2026-11-16

## Context

Whether a tool call needs the user's consent is decided in several places that do not agree.

- A tool's class (`read`, `edit`, `execute`) is a string in its `content/tools/<Name>.js` file, read by `crates/agent-engine/src/tools/mod.rs`. The permission class of a Rust tool is therefore set by an embedded JavaScript file that a user can override.
- The "outside the project" check exists only for reading tools (`crates/agent-engine/src/agent/permission.rs`). In the default mode `Write` and `Edit` write to any path without asking.
- A Bash command classified as read-only runs without a question in every mode, including plan. The classifier in `crates/domain/src/command_policy/reading.rs` and `crates/domain/src/permission_rules/pattern.rs` works on text: it drops `VAR=value` prefixes and `sudo` before looking, does not filter flags that write or execute, and tests "inside the project" by string.
- The secrets block in `crates/domain/src/command_policy/secrets.rs` is a list of programs and file names that a wildcard or an unlisted program escapes.
- Deny rules match on the same text splitting and can be stepped around with quoting, a wrapper program or a variable.

The mode table, the rules, the hook decision, the forced questions and the bypass classifier are combined in one 116-line function.

## Decision

One policy engine in `domain` decides, from what a call will do.

- Each tool declares in Rust the effects of a given call: read a path, write a path, execute a command, reach a network host, spawn.
- `PolicyEngine::decide` takes the tool, its effects and the context (mode, rules, project boundary for reads and writes, folder trust from `0004-workspace-trust.md`) and returns allow, ask or deny. Paths are resolved, not compared as text.
- Bash commands are parsed with a real shell parser. A command is read-only only if every program in it is on a list of program plus permitted flags. Environment assignments and wrappers are part of what is judged.
- Auto-approval of read-only commands becomes opt-in.
- The `access` field is removed from the 39 `content/tools/*.js` files.
- Rollout: the new engine first runs in shadow mode beside the current verdict and differences are logged; then it replaces it. The audit's bypasses become regression tests, and the parser gets property tests.

Phase 1 ships a stopgap (no auto-approval of "read-only" Bash, an outside-project check for writing tools); this decision is phase 6.2.

## Consequences

- One place answers "may this run", and it is testable without the engine.
- Writes are bounded by the project the same way reads are.
- A content override can no longer change what a tool is allowed to do.

What gets worse:

- More questions by default. Commands that ran silently as read-only now ask until the user opts in or writes a rule.
- Every tool, including MCP and script tools that cannot describe their effects precisely, needs a conservative declaration, which pushes them toward "ask".
- A shell parser is a new dependency or a new body of code, and shell is large: constructs the parser does not understand must be treated as "ask", never as safe.
- The shadow period means two implementations run for a while, and existing allow rules written against the old text matching may stop matching.
