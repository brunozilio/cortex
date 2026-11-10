# Superfik Lite — working agreement

Internal Rust workspace: Superfik's agent engine, driven from a terminal
chat (CLI/TUI) and an SDK (ADR 0018). No GUI.

## Before changing code
- Follow the Cargo isolation rules below before running any Cargo command.
- Read @docs/architecture.md.
- Run `cargo nextest run --workspace` (or `cargo test --workspace`) and `cargo clippy --workspace --all-targets -- -D warnings`.
- `/verify` runs tests, clippy and `cargo run -p xtask -- lint`; run it before committing. The `arch-guard` agent reviews a diff against these rules.

## Cargo isolation — keep the user's `cargo run` unblocked
- The default `target/` build directory is reserved for the user's terminal.
  Agents must set `CARGO_TARGET_DIR` explicitly for every build, test, check,
  clippy, nextest, snapshot and xtask invocation, including commands in skills,
  scripts and background jobs. Never launch these against the default target.
- Use an absolute directory per independent task or agent, for example
  `CARGO_TARGET_DIR="$PWD/target/agents/<task-id>" cargo test --workspace`
  (replace `<task-id>` with a unique, stable task name). Reuse that directory
  throughout the task to keep incremental builds. Concurrent agents must use
  different directories; run commands sharing one directory sequentially.
- Pass this rule and the assigned target directory to every delegated agent.
  Ensure subprocesses inherit it; scripts must not override it with `target/`.
- If Cargo reports a lock wait, inspect the holder and correct the target
  directory instead of launching more waiting commands. Never delete lock
  files or kill another session's processes. Stop obsolete jobs you started.
- Do not run `cargo clean` against the default target or delete the user's
  build artifacts. Separate targets isolate build locks, but Cargo's shared
  package cache can still briefly require a lock during dependency downloads.

## Rules
- Domain rules live in `superfik-core` (pure, no IO), never in the CLI's
  rendering or the SDK's glue.
- Files over ~200–300 lines or functions mixing concerns get split.
- Every new behaviour gets a test; IO sits behind traits so logic is testable.
- Every command carries a `Principal`; keep the backend seam intact for the
  future remote, multi-user server.
- No `unwrap()` outside tests.
- Edit files with the Edit tool, not `python3`/`sed -i` rewrites.
- Skills: `tdd` for new behaviour and fixes, `diagnosing-bugs` for anything broken or slow.

## Workflow
Work directly: no specs or tickets unless asked. Plan in the conversation,
implement, verify. Never commit to `main`
(any branch name will do).
