# Repository instructions

Read and follow [CLAUDE.md](CLAUDE.md), including its Cargo isolation rules.

The default `target/` is reserved for the user's terminal. Before running any
Cargo build, test, check, clippy, nextest, snapshot or xtask command, explicitly
set `CARGO_TARGET_DIR` to an absolute, task-specific directory such as
`$PWD/target/agents/<task-id>`. Reuse it for this task and run commands using it
sequentially. Independent agents must use different directories. Apply this
also to scripts, background jobs and delegated work.
