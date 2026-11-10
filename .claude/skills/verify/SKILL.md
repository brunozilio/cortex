---
name: verify
description: Run the full Superfik verification loop (tests, clippy, repo lint). Use before committing or when asked to verify/check a change.
---

Set `CARGO_TARGET_DIR` to this task's directory (CLAUDE.md), then run in
order, stop at the first failure and fix it:

1. `cargo nextest run --workspace` then `cargo test --doc --workspace` (nextest skips doctests)
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo run -q -p xtask -- lint` (new findings must be fixed, not baselined, unless the user agrees)

Report pass/fail per step in one line each; show full output only for failures.
