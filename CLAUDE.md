# Working agreement

Rust workspace: a coding agent driven from a terminal chat (CLI/TUI). The
product's name lives in `brand.toml` (read through the `brand` crate:
`brand::NAME`, `brand::COMMAND`; `cargo run -p xtask -- brand` writes the
files that must spell it out). It is a terminal product; it also serves a
local web page on loopback for the trace, the console, the user docs and a
terminal attached to a session. That page observes and documents; it is not
an editor.

## Crates
Dependencies are one-way; the main line is
`cli ──► app ──► agent-engine ──► llm-providers ──► domain`.
These are all the non-dev edges between workspace crates:

```
cli              ──► app, console, content, platform, domain, brand
app              ──► agent-engine, llm-providers, code-map, platform, domain, brand
agent-engine     ──► llm-providers, language-servers, script, content, platform, domain, brand
llm-providers    ──► platform, domain, brand
domain           ──► brand
language-servers ──► platform, domain, brand
content          ──► script, brand
console          ──► page, tunnel, platform, brand
relay            ──► page, tunnel
code-map, platform, script, brand, page, tunnel ──► no workspace crate
```

`xtask lint` checks crate edges: a manifest edge missing from the table in
`xtask/src/crate_graph.rs`, or a table edge no manifest uses, fails. Change
that table and this diagram in the same commit as the `Cargo.toml`.

`web/` is the page `/trace`, `/console` and `/docs` open (Bun + Vite +
React + Tailwind). Cargo never runs Bun: `cargo run -p xtask -- web` builds
the page into `web/dist`, and `page`'s `build.rs` only embeds what is
there (nothing when it is absent; the gate needs neither Bun nor a page).
`cargo run -p xtask -- check-release` fails on a missing or stale
`web/dist`. The page's types mirror `domain::trace` by hand; the fixtures in
`web/test/fixtures` are written from the Rust types by `cargo run -p xtask
-- web-fixtures` and checked by `cargo test -p xtask` and `bun test`
(`web/README.md`). The CLI's trace feature pushes the report, the
session list and each session's runs into the console's `Live` hub.

Nothing below `cli` knows the CLI; `cli` reaches `agent-engine`, and the
handler side of `app`, only in tests. The README's crate table says what each one
is (`xtask lint` checks it lists every crate).

## Docs
- `docs/vision.md`: the problem, who it is for, scope, non-goals, open decisions.
- `docs/GLOSSARY.md`: the project's vocabulary and where each term lives.
- `docs/architecture.md`: crate map, request path, data on disk, threading.
- `docs/decisions/`: one record per decision (`0001`…`0012`), with context and costs.
- `docs/threat-model.md`: assets, actors, trust boundaries, known gaps.

Files under `docs/` must not spell the product's name, command or env
prefix (`xtask lint` checks); write "the app", `<command>`, `.<command>/`.

## Before changing code
- Follow the Cargo isolation rules below before running any Cargo command.
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
- Delete a task's `target/agents/<task-id>` directory when the task is done.

## Rules
- Domain rules live in `domain` (pure, no IO), never in the CLI's
  rendering or the `app` client glue (`crates/app/src/client/`).
- No comments in code: no `//`, `///`, `//!` or `/* */` in Rust, no `#`
  comments in shell, Python or TOML (a script's `#!` line is fine). The
  one exception: JSDoc type annotations (`@type`, `@param`) in `content/`
  JavaScript, pointing at `content/types.d.ts`. Say it
  with names, types and tests instead; context belongs in the commit
  message. `.claude/hooks/no-comments.sh` rejects an edit that adds
  one and `xtask lint` fails on any.
  Rationale goes to `docs/decisions/`, vocabulary to `docs/GLOSSARY.md`.
- Functions over 100 lines fail clippy (`too_many_lines`); keep a type's
  impl in few cohesive modules; file length is not a goal.
- Every new behaviour gets a test; IO sits behind traits so logic is testable.
- Single-user local product: no `Principal`, members, authorizer or remote
  server; see `docs/decisions/0008-single-user-local-product.md`.
- No `unwrap()` outside tests.
- Edit files with the Edit tool, not `python3`/`sed -i` rewrites.
- Skills: `tdd` for new behaviour and fixes, `diagnosing-bugs` for anything broken or slow.

## Workflow
Work directly: no specs or tickets unless asked. Plan in the conversation,
implement, verify. Work and commit on `main` in this checkout; no feature
branches or worktrees unless asked. Work is sequential by default: parallel
agents are only allowed on non-overlapping files and non-overlapping crate
dependency cones (a crate plus everything that depends on it).
