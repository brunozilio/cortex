# The web page

The browser app behind `/trace`, `/console` and `/docs`: Bun, Vite, React and
Tailwind. The `console` crate serves it on localhost and feeds it JSON; this
folder knows nothing of Rust beyond the shapes in `src/types.ts`.

## Building

Cargo builds the page: `crates/page/build.rs` runs `bun install
--frozen-lockfile` and `bun run build` when a source under `web/` is newer
than `web/dist`, so `cargo build` and `cargo install --locked --path
crates/cli` always embed a current page. Without Bun, or with
`SKIP_WEB_BUILD=1`, the build warns and embeds what `web/dist` holds; with no
`web/dist/index.html` the binary carries no page, and the page it serves
instead says how to build it. To build the page alone:

```sh
cargo run -p xtask -- web          # bun install --frozen-lockfile, then bun run build, into web/dist
```

Before a release, after `xtask web`:

```sh
cargo run -p xtask -- check-release
```

It fails when `web/dist/index.html` is missing or older than the newest file
under `web/src`, `web/docs`, `web/public`, `web/index.html`, the package
files or the build configuration, so a release cannot carry a stale page. The normal gate
(tests, clippy, `xtask lint`) needs neither Bun nor a built page.

## Working on it

```sh
cd web
bun install
bun run dev        # Vite on its own port; /api is proxied to CONSOLE_URL (default http://127.0.0.1:3000)
bun test           # the tests in web/test
bun run build      # type-check (tsc -b) and bundle
```

Without a console behind it, `bun run dev` shows the sample data in
`src/sample`. To serve a build from disk instead of the embedded one, point
the `<COMMAND>_CONSOLE_DIR` environment variable (the product's command in
capitals) at a folder holding an `index.html`.

## The contract with Rust

The page's types in `src/types.ts` are written by hand. They mirror the JSON
the Rust side serializes: `domain::trace` (`Report`, `SessionRuns`, `Insights`,
runs and spans) and the session cards of the terminal's board. Three pieces
keep the two in step:

1. `web/test/fixtures/*.json` are produced by Rust, from the real types:
   `xtask/tests/web_fixtures/` builds a report with two sessions, a followed
   session update and a session list, covering every span kind, signal, run
   status, finding check and session status.
2. `cargo test -p xtask` (part of the workspace tests) serializes them again
   and fails when they differ from the committed files. A change to the Rust
   types therefore fails the gate until the fixtures are written again.
3. `web/test/contract.test.ts` checks every fixture against the runtime
   guards in `src/lib/contract.ts` (`isReport`, `isSessionRuns`,
   `isSessionCards`, …). The same guards run in the page: `loadReport`, the
   live feed and the session list refuse a payload of the wrong shape and say
   which field is wrong instead of drawing it.

So when a Rust type changes:

```sh
cargo run -p xtask -- web-fixtures   # writes web/test/fixtures again
cd web && bun test                   # fails where the guards no longer agree
```

then bring `src/types.ts` and `src/lib/contract.ts` up to date together
(`bun run build` fails when the guard's lists of names and the types
disagree) and commit the fixtures with the change.

What the fixtures cannot see: the session cards are built with `json!` in the
terminal's board (`crates/cli/src/tui/features/trace/board.rs`) and the
`insights` field is added to a followed session in
`crates/app/src/features/trace/mod.rs`; the fixture builder repeats those two
steps by hand, so a renamed field there has to be renamed in
`xtask/tests/web_fixtures/main.rs` too.

## The live feed

The page reads `/api/report` once, then follows `/api/events` (server-sent
events): a plain message is one session's latest runs, `report` says the
report changed and `resync` says the console dropped updates for this page
because it was reading too slowly. On `report`, on `resync` and after a lost
connection comes back, the page fetches `/api/report` and `/api/latest` (each
session's latest update) again. While it is disconnected or catching up, the
header says the data may be out of date or partial. The reducer is
`src/lib/feed.ts`; `src/lib/useLiveFeed.ts` only wires it to an `EventSource`.
