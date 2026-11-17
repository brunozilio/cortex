# 0003 One effects-based permission policy

Status: Implemented, 2026-11-17 (except opt-in auto-approval; see "As implemented")

## Context

Whether a tool call needs the user's consent is decided in several places that do not agree.

- A tool's class (`read`, `edit`, `execute`) is a string in its `content/tools/<Name>.js` file, read by `crates/agent-engine/src/tools/mod.rs`. The permission class of a Rust tool is therefore set by an embedded JavaScript file that a user can override.
- The "outside the project" check exists only for reading tools (`crates/agent-engine/src/agent/permission.rs`). In the default mode `Write` and `Edit` write to any path without asking.
- A Bash command classified as read-only runs without a question in every mode, including plan. The classifier in `crates/domain/src/safety/command_policy/reading.rs` and `crates/domain/src/safety/permission_rules/pattern.rs` works on text: it drops `VAR=value` prefixes and `sudo` before looking, does not filter flags that write or execute, and tests "inside the project" by string.
- The secrets block in `crates/domain/src/safety/command_policy/secrets.rs` is a list of programs and file names that a wildcard or an unlisted program escapes.
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

## As implemented

- `crates/domain/src/safety/permission/`: `Effect` (read a path, read a folder tree, write a path, execute a parsed command, reach a host, spawn work, cancel work, notify, message a session, read an MCP resource, call an MCP server, a script tool's declared class), `PolicyContext`, `PolicyEngine::decide` and `Verdict` (`Allow`, `Ask { reason, rememberable, rule }`, `Deny`). Its first version ports the mode table and the rule precedence of the old decision: a deny rule always wins; in bypass and for anything refused while planning the mode decides; otherwise an ask or allow rule decides; otherwise the mode. A read-only command, a call with no effects and one that only spawns a sub-agent or a watch, notifies, or cancels its own work count as reads. Risky commands, an address heard only from fetched content, leaving the sandbox and a package concern ask every time, with the concern as the verdict's reason.
- Every tool implements `Tool::effects(input, workspace)`; paths are resolved the way the outside check resolves them, keeping both the path as written and where it really leads, and a rule is judged against both. A user script tool in `.<command>/tools/*.js` still declares a coarse `access` (`read`, `edit`, `execute`; a project's script tool always asks); the built-in tools in `content/tools/*.js` no longer declare one, and a test keeps it so.
- The shadow run was dropped before it ran: the port is checked instead by a domain test that replays the old mode table for every effect family, mode and kind of rule, and by the unchanged agent-loop and golden permission tests. The old table is gone (`Access`, `verdict`, `ruled`, `declared_access`, `reaches_outside`, `writes_to`, the Bash special-casing in `agent/permission/command_class.rs`).
- `crates/domain/src/safety/shell/` parses a command line: simple commands, assignments, quoting (single, double, `$'…'`, `$"…"`, backslashes), pipelines, `&& || ; &` and newlines, subshells and groups, `if`, `for`, `select`, `while`, `until`, `case`, `[[ … ]]`, function definitions, redirections including heredocs and here-strings, `$(…)`, backticks and process substitution. Arithmetic (`$((…))` and `((…))`, C-style `for`), `coproc`, substitutions inside `${…}` or arithmetic, nested backticks, an unquoted heredoc whose body substitutes, unbalanced quotes or brackets, and nesting deeper than 48 are opaque. Rules, the read-only list, the secrets block and the risky-command check see every simple command, including those in control structures, functions, substitutions and `find -exec`; `sudo env nice timeout xargs nohup time command builtin doas stdbuf ionice chrt busybox` are unwrapped.

Deliberate changes in behaviour:

- `KillShell` needs no permission only for a background command this agent started; any other id asks.
- A Bash deny rule now also restricts the command `Monitor` runs, and the secrets block applies to it.
- A command inside a substitution, behind `xargs`, run by `find -exec` or inside a function or `case` is denied by a deny rule naming it, where it used to be asked about.
- A quoted pattern (`find . -name '*.rs'`, `grep 'a.*b' file`) and a revision's tilde (`git diff HEAD~1`) no longer stop a command from being read-only.
- A read-only command inside `if`/`for`/`while`, a function or a group is no longer read-only, but each part can now be covered by allow rules.

What gets worse, as implemented:

- Auto-approval of read-only commands is still on by default in every mode; the opt-in is not done.
- An opaque line asks in more cases than the old splitter, which guessed.
- The secrets block and deny rules remain best-effort lists; the sandbox is the barrier.
