# Cortex

A coding agent you talk to in the terminal: a chat (CLI/TUI) with agents,
commands, skills, memory, plugins and MCP servers (HTTP and stdio).

## Installing

From source:

```sh
cargo run -p xtask -- web          # builds the web page into web/dist; needs Bun
cargo install --path crates/cli    # embeds web/dist as it is at that moment
```

Building needs a Rust toolchain (1.97 or newer) and a C compiler. The web
page (`/trace`, `/console`, `/docs`) is built with [Bun](https://bun.sh):
`cargo build` and `cargo install` rebuild it themselves whenever a file under
`web/` is newer than `web/dist`. Without Bun, or with `SKIP_WEB_BUILD=1`, they
warn and embed the page as it last was; with no page at all `/trace` opens
one that says how to build it. Nothing else is
needed to run it. Some tools are used when present:

- `git`: `/diff` and worktrees.
- `pdftotext`: reading PDFs.
- `ripgrep`: optional; `Grep` works without it.
- Language servers (rust-analyzer, clangd…): the `LSP` tool and
  diagnostics after edits.
- `node`/`npx`: only for MCP servers that need them.

## Using it

```sh
cortex login                             # API key or subscription; lists the providers
cortex logout                            # forget a provider
cortex                                   # chat in the current project
cortex -p "explain src/main.rs"          # one answer, printed
cortex models                            # what the connected providers offer
cortex mcp                               # MCP servers: list, add, remove, approve, deny, login, logout
cortex plugin                            # plugins: list, install, update, remove
cortex config                            # read or change the settings: list, get, set, unset
cortex doctor                            # check this installation in this folder
cortex eval                              # run .cortex/evals/<name>/case.json and score them
cortex remote                            # remote control from a phone: status, up, relay, pair, devices, revoke
```

In the chat: Enter sends, Alt+Enter (or Ctrl+J) adds a line, Esc
interrupts, Shift+Tab cycles ask → edits → plan → bypass, Ctrl+V attaches
the clipboard's image (or drop an image file on the terminal; it shows as
`[Image #1]`), Ctrl+O unfolds reasoning, outputs and diffs. `/help` lists
the commands (`/model`, `/mode`, `/effort`, `/compact`, `/diff`, `/memory`,
`/skills`, `/agents`, `/clear`, `/cost`, `/exit`); any other `/name` runs a
skill or custom command. Replies render as Markdown with highlighted code;
`CORTEX_REDUCED_MOTION=1` turns the animations off. Colours follow the
terminal's background (from `COLORFGBG`, or `CORTEX_THEME=light|dark`) and
use true colour where `COLORTERM` says so.

Project settings, skills, agents and rules live in `.cortex/`, project
instructions in `CORTEX.md` or `.cortex/CORTEX.md`, and your own in
`~/.cortex/`. Plugins live in `~/.cortex/plugins/` (`cortex plugin
install <git-url>`, `list`, `update`, `remove`); `/reload` in the chat
lists what was found.

## Environment variables

- `CORTEX_LOG`: log filter for the log file (`logs/cortex.log` in the data folder); the default is `warn`.
- `CORTEX_THEME`: `light` or `dark`; overrides the background detected from `COLORFGBG`.
- `CORTEX_REDUCED_MOTION`: any value other than empty or `0` turns the animations off.
- `CORTEX_CONSOLE_DIR`: a folder holding an already built web page (with an `index.html`) to serve instead of the embedded one.
- `CORTEX_SECRETS`: `keychain` (any case) moves the key of `secrets.json` from `secrets.key` into the system keychain; unset, it stays in the file (a rebuilt binary has a new signature, and macOS would ask for the keychain password at every start).
- `CORTEX_STREAM_IDLE_SECONDS`: how many seconds a model's stream may stay silent before the request is given up; the default is 300.
- `CORTEX_OTLP_ENDPOINT`: an OpenTelemetry collector (`http://localhost:4318`) each finished run is sent to as OTLP/HTTP JSON; unset, nothing is exported.
- `CORTEX_OTLP_HEADERS`: `name=value` pairs, separated by commas, added to each export request.

Set for others: `CORTEX_PROJECT_DIR` (the project folder, for command
hooks) and `${CORTEX_PLUGIN_ROOT}` (replaced with a plugin's folder in its
MCP servers, language servers and hooks).

Also read: `SHELL`, `VISUAL`, `EDITOR`, `HOME`, `COLORFGBG`, `COLORTERM`
and `XDG_CONFIG_HOME`.

## Docs

- [docs/vision.md](docs/vision.md): what it is for, scope and non-goals.
- [docs/GLOSSARY.md](docs/GLOSSARY.md): the project's vocabulary.
- [docs/architecture.md](docs/architecture.md): crates, request path, data on disk, threading.
- [docs/decisions/](docs/decisions/): the decision records.
- [docs/feature-flags.md](docs/feature-flags.md): every feature flag, what it gates and what to do with it.
- [docs/threat-model.md](docs/threat-model.md): assets, actors, trust boundaries, known gaps.

## Where data lives

`~/Library/Application Support/Cortex` on macOS, `~/.config/cortex` on
Linux: sessions, memory and settings. Keys and tokens are sealed in
`secrets.json` there (readable only by you); its key is in `secrets.key`
beside it (also readable only by you), or in the system keychain if you
set `CORTEX_SECRETS=keychain`.

The list of sessions and each project's memory are in `app.db` there, a
SQLite database (with `app.db-wal` and `app.db-shm` beside it while the
program runs), so several running instances share them without losing
each other's changes. Conversations stay as `.jsonl` files and settings
as JSON.

Upgrading from a version that kept them as JSON: the first start imports
`users/local/sessions.json` and `memory/local/<project>.json` into
`app.db` and renames each file with `.imported`; nothing is deleted.
Downgrading afterwards: the older binary finds only the `.imported`
files and shows an empty session list and empty memory until you quit it
and rename them back (drop `.imported`). What was saved after the import
is only in `app.db`, which the older binary does not read.

## Layout

| Crate | What it is |
|---|---|
| `domain` | Domain types and rules: requests, engine events, memory. Pure. |
| `agent-engine` | The agent loop, tools, permissions, hooks, sub-agents, skill/agent discovery, MCP, sessions, compaction and usage metering. It reaches models through `llm-providers`. |
| `app` | The application API, in process: the CLI's client (sessions, runs), `ClientCommand` / `ServerEvent`, and the command handler behind them: runs, memory, settings, providers, secrets. |
| `llm-providers` | Talking to a model: the `Provider` trait, one folder per provider, the model client that sends requests over HTTP and reads the streams, the wires, the transcript types, credentials, OAuth, sign-in, catalogs, declared providers and model-family vendor folders. The subscription providers are a Cargo feature, `subscription-providers`, on by default. |
| `cli` | The `cortex` binary: terminal chat (ratatui), `-p`, `login`, `logout`, `models`, `mcp`, `plugin`, `remote`, `config`, `doctor`, `eval`. |
| `brand` | The product's name and command, and one module of constants per section of `config.toml` (`brand::cloudflare`), from `config.toml`. |
| `language-servers` | Runs language servers for the `LSP` tool and edit diagnostics: client, pool, first-use installs. |
| `code-map` | Experimental tree-sitter index of a repository's functions, types and callers, ranked against a prompt. |
| `platform` | Per-OS IO: child processes and their trees, private files, stored documents (lock, atomic replace, corrupt set aside), data folders, the system keychain; the process's one async runtime and the run's `Stop` (cancellation token plus flag). |
| `content` | Prompts, built-in skills and agents, embedded at build time. |
| `script` | The QuickJS runtime for JavaScript tools, skills and providers. |
| `console` | The live trace page behind `/trace` and `/console`: serves the browser app on localhost (the page `page` embeds) with the report, the sessions and their updates as server-sent events. |
| `page` | The built web page: its `build.rs` embeds whatever `cargo run -p xtask -- web` last built into `web/dist` (nothing when it is absent), served by `console` on the machine and by `relay` to phones. |
| `tunnel` | The remote control's wire: relay frames, machine and device keys, pairing links, the Noise handshake, and the requests a phone sends through it. |
| `relay` | The remote control's self-hosted server: serves the page as an installable web app and forwards encrypted frames between a phone and its machine, without reading them; also as a Cloudflare Worker (`worker/relay.js`) that `remote up` publishes. |

## Developing

Read [CLAUDE.md](CLAUDE.md): working rules, how the crates depend on each
other, and how to run Cargo.

```sh
export CARGO_TARGET_DIR="$PWD/target/agents/<task>"
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -q -p xtask -- lint
```

None of these needs Bun or a built page. The web page in `web/` is built
and checked by its own steps ([web/README.md](web/README.md) has the
details):

```sh
cargo run -p xtask -- web             # bun install --frozen-lockfile + bun run build, into web/dist
cargo run -p xtask -- check-release   # before a release: fails if web/dist is missing or older than its sources
cargo run -p xtask -- web-fixtures    # after changing a trace type: rewrites web/test/fixtures from the Rust types
(cd web && bun test)                  # the page's tests, including the fixtures against its types
```

`cargo build` embeds `web/dist` when it exists and nothing when it does
not; it never starts Bun or touches the network for the page.

## Fuzzing

`fuzz/` holds [cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html)
targets (libFuzzer) for the parsers that read outside input. It is a
workspace of its own, left out of the main one, so the gate never builds
it. Each target fails on a panic and, where one holds, on a broken
property:

| Target | Reads | Property beyond "no panic" |
|---|---|---|
| shell_parse | any shell line through the parser and the permission rules | a line allowed under a `Bash(rm:*)` deny has no part that runs `rm` |
| shell_deny | generated lines with `rm` as a real command behind separators, groups, wrappers and spellings | never reaches Allow while `Bash(rm:*)` is denied |
| wildcards | rule and path wildcards | command and path matching agree with a reference matcher; a word could share a file with itself |
| web_page | a fetched page as HTML, feed, JSON or text | |
| code_map | Rust, TypeScript, TSX, JavaScript, Python and Go sources | symbols and calls fall within the file's lines |
| session_log | a saved conversation's bytes | the whole lines before a cut last line are kept |
| sse_stream | provider streams: lines, server-sent events, the three wires and the Kiro event stream | the lines read do not depend on how the stream arrives in pieces; caps hold |
| mcp_messages | an MCP server's answers: `initialize`, `tools/list`, tool results | listed tool names are prefixed and unique |
| config_schema | `settings.json` and `mcp.json` | a file without problems is kept as it is; checking twice changes nothing more |
| tunnel_frame | relay frames, a phone's requests and answers, pairing links | what reads back encodes back to the same bytes |
| tunnel_handshake | a phone's first handshake message, sealed messages, machine proofs | only a known or pairing device is let in; nothing forged opens or proves |

It needs a nightly toolchain (CI pins `nightly-2026-11-01`) and
`cargo install cargo-fuzz --locked`. To run one target for a minute:

```sh
cd fuzz
cargo +nightly-2026-11-01 fuzz run --sanitizer none shell_deny -- -max_total_time=60
```

`fuzz/corpus/<target>` holds small seeds taken from the tests' fixtures;
pass a scratch folder first (`fuzz run shell_deny /tmp/work corpus/shell_deny`)
to keep what the run finds out of the repository. A crash is written to
`fuzz/artifacts/<target>/`; replay it with `fuzz run <target> <file>`, then
add it as a unit test in the crate that owns the parser before fixing it.
The `fuzz` job in CI runs every target for 30 seconds on each push to
`main`, for 10 minutes every night, and for as long as asked when started
by hand; it uploads what crashed. The sanitizer is off: the targets are
safe Rust, and the C parsers behind them (tree-sitter, QuickJS) are built
by `cc` with the system compiler, whose AddressSanitizer runtime does not
link with the toolchain's on macOS.

## Supply chain

`deny.toml` configures [cargo-deny](https://embarkstudios.github.io/cargo-deny/);
the `deny` job in CI runs it on every push and pull request.

```sh
cargo install --locked cargo-deny
cargo deny --locked check
```

It checks four things: advisories from the RustSec database (a yanked
version fails), licenses (only the ones listed in `deny.toml`, which are
the ones the dependency tree uses today; the workspace's own crates are
private and skipped), bans (a `*` version requirement fails, two versions
of one crate only warn) and sources (crates.io only, no git dependency).
An advisory is ignored only by an entry in `deny.toml` with its id and the
reason. A new dependency under a licence not yet listed fails the job
until the licence is added to `allow` on purpose.

## Releasing

`.github/workflows/release.yml` runs when a `v*` tag is pushed. For each of
`aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`
and `aarch64-unknown-linux-gnu` it builds the web page
(`cargo run -p xtask -- web`), checks it (`cargo run -p xtask --
check-release`), builds the binary with `--release --locked`, and packs it
with this README into `<command>-<tag>-<target>.tar.gz` beside a
`.sha256`. The archives are then attached to a draft GitHub release;
nothing is public until you publish the draft. Windows is not released.

1. Set `version` under `[workspace.package]` in `Cargo.toml`, run
   `cargo metadata --format-version 1 > /dev/null` so `Cargo.lock` follows,
   and commit both.
2. Wait for CI on `main` to pass.
3. `git tag v<version>` and `git push origin v<version>`. The workflow
   fails at once when the tag is not `v` followed by the workspace version.
4. Read the draft under the repository's Releases, check an archive against
   its `.sha256` (`shasum -a 256 -c <file>.sha256`), and publish it.
