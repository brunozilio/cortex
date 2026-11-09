# Contributing

Thanks for helping. Everyone taking part follows the
[code of conduct](CODE_OF_CONDUCT.md).

Superfik is a native Rust desktop canvas (`winit` + `wgpu`, no web view)
plus a self-hostable server. Read these before changing code:

- [docs/architecture.md](docs/architecture.md): crates, the reducer, the
  backend seam.
- [docs/design-system.md](docs/design-system.md): every colour, size, font
  and duration comes from `superfik-ui/theme`.
- [CLAUDE.md](CLAUDE.md): the working rules, which hold for people as much
  as for agents.
- [GLOSSARY.md](GLOSSARY.md) and [docs/decisions.md](docs/decisions.md):
  the vocabulary and the decisions behind it.
- [docs/platforms.md](docs/platforms.md) and
  [docs/native-engine.md](docs/native-engine.md): building per platform,
  the browser engine, and the agent engine's configuration.

## Set up

You need stable Rust (see `rust-version` in `Cargo.toml`) and, on macOS,
`cmake` and `ninja` (`brew install cmake ninja`) for the browser card's CEF
wrapper. The first build downloads CEF (~340 MB). Per-platform details are in
[docs/platforms.md](docs/platforms.md).

```sh
cargo run -p superfik-app -- /path/to/a/repository
```

On macOS the first run lays out `target/debug/Superfik.app` (CEF only runs
from an app bundle) and continues from there. Packaging: `scripts/bundle-macos.sh`,
`scripts/bundle-linux.sh`, `scripts/bundle-windows.ps1` and
`.github/workflows/scripts/bundle-server.sh`.

## Before opening a pull request

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo run -p xtask -- lint
cargo run -p superfik-app -- --snapshot /tmp/s.png --scale 2   # for UI changes
```

- **Every new behaviour gets a test.** IO sits behind traits so the logic
  can be tested.
- **Domain rules** live in `superfik-core` or the app reducer (`state/`),
  never in views or components. The reducer is pure: IO happens through
  `Effect`s.
- **UI components** are stateless: props in, paint, return hit geometry.
- **No `unwrap()`** outside tests. Split files that grow past ~300 lines.
- **Show UI changes:** attach before and after snapshots to the PR.

## Branches and pull requests

- Never commit to `main`. Branch as `<type>/<slug>` (or `<n>-<slug>` for
  issue `#n`); one change per PR.
- The **PR title** becomes the squash commit and must be a
  [Conventional Commit](https://www.conventionalcommits.org/): `feat:`,
  `fix:`, `perf:`, `refactor:`, `docs:`, `test:`, `ci:` or `chore:`, with an
  optional scope (`fix(browser): …`) and `!` for a breaking change. CI checks
  it, and release notes are built from it.
- Link the issue it fixes (`Fixes #n`). Merge by squash once CI is green.

## Releases and CI

Merged `feat:`/`fix:` titles accumulate in release-please's release PR;
merging it publishes a signed release (the `release` skill).

CI runs every check on every push and pull request. `SUPERFIK_CI_RUNNER`
(a JSON list of labels) moves the jobs to a self-hosted runner. Pull requests from forks always run
on GitHub-hosted runners. Scripts used only by the workflows live in
`.github/workflows/scripts/`.

## Issues

Use the bug or feature template. Security problems never go in an issue:
see [SECURITY.md](SECURITY.md).

## Licence

By contributing, you agree that your contribution is licensed under the
[Apache License 2.0](LICENSE).
