# Repository instructions

Read and follow [CLAUDE.md](CLAUDE.md), including its Cargo isolation rules.

The default `target/` is reserved for the user's terminal. Before running any
Cargo build, test, check, clippy, nextest, snapshot or xtask command, explicitly
set `CARGO_TARGET_DIR` to an absolute, task-specific directory such as
`$PWD/target/agents/<task-id>`. Reuse it for this task and run commands using it
sequentially. Independent agents must use different directories. Apply this
also to scripts, background jobs and delegated work. Delete that directory when
the task is done.

Work and commit on `main`, sequentially. Parallel agents are only allowed on
non-overlapping files and non-overlapping crate dependency cones.

No comments in code. Rationale goes to `docs/decisions/`, vocabulary to
`docs/GLOSSARY.md`. The product is single-user and local
(`docs/decisions/0008-single-user-local-product.md`).

Docs: `docs/vision.md`, `docs/GLOSSARY.md`, `docs/architecture.md`,
`docs/decisions/`, `docs/threat-model.md`. Files under `docs/` must not spell
the product's name, command or env prefix; write "the app", `<command>`,
`.<command>/`.
