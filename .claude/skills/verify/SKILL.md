---
name: verify
description: Run the full Superfik verification loop (tests, clippy, repo lint, visual snapshot). Use before committing or when asked to verify/check a change.
---

Run in order, stop at the first failure and fix it:

1. `cargo nextest run --workspace` then `cargo test --doc --workspace` (nextest skips doctests)
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo run -q -p xtask -- lint` (new findings must be fixed, not baselined, unless the user agrees)
4. `cargo deny check` when dependencies changed.
   When `cargo-xwin` is installed (`~/.cargo/bin`): `cargo xwin clippy --target x86_64-pc-windows-msvc --workspace --all-targets -- -D warnings` (ADR 0013; a cold build takes over ten minutes, so run it in the background).
5. If UI changed: `cargo run -p superfik-app -- --snapshot /tmp/s.png --scale 2`, then Read `/tmp/s.png` and check it against `docs/design-system.md`.

Report pass/fail per step in one line each; show full output only for failures.
