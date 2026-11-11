# Decisions

Decisions that shape the app. Code cites them as `ADR NNNN`; numbers
of decisions dropped with the desktop app are not reused.

## 0015 — The app is the engine

No agent CLI runs underneath. The agent loop is ours (`agent-engine`): it calls the provider's API itself, runs its tools as
Rust functions, and emits `EngineEvent`s, so clients are engine-agnostic.
Providers sit behind `ModelApi`, tested against captured streams; local
models run on MLX as Chat Completions. The system prompt, tools, sessions
and compaction are ours; the prompt loads `<COMMAND>.md` and names the app.

## 0016 — API keys and subscriptions are separate providers

OpenAI API, ChatGPT Subscription, Claude Subscription, Anthropic API,
GitHub Copilot Subscription and Kiro Subscription are independently connected providers, each with its own credentials and
model catalog. Pure contracts live in `llm-providers`; the native
engine does the IO. The backend handles PKCE and loopback callbacks
and keeps credentials in its `SecretStore`, saving rotated refresh tokens;
runs carry a redacted handle a client cannot supply. Subscription adapters
apply their own preludes; the tools, loop and project instructions stay
The app's.

## 0018 — Lite: a CLI and an SDK

the app drops the desktop canvas, the web client, the team server and
everything only they used (browser cards, voice, sound, haptics, notes,
routines, channels, vaults, review, visuals, updates, i18n). What remains is
the engine, the backend that drives it, memory, sessions, skills, agents
and MCP. Two crates come on top: a terminal chat (CLI/TUI) and an SDK for
building agents, commands, skills, memory access and in-process MCP tools,
and for connecting MCP servers over HTTP and stdio.

- The AI providers are LLM providers (`llm-providers`,
  `ModelProviders`). Skills, agents and MCP come from the app's folders
  (ADR 0021), its plugins (ADR 0019) and the SDK host. No web search or
  fetch tool.
- One engine, ours. No registry of engines, no tool worker process, no
  git/workspace inspection, no session import. (The memory optimizer came
  back in ADR 0022.)
- A separate product from the desktop the app: its own data folder,
  nothing shared, nothing migrated. Secrets sit in a sealed file, never
  the system keychain.
- The `Principal`/`Envelope`/`Authorizer` seam stays so a remote backend
  can come back without reshaping the SDK.
- Published as source (the repository), with no license,
  release pipeline or packaging: install it with `cargo install` (amended
  by ADR 0024).

## 0019 — A platform: the app plugins

The app is a general platform; a company's integrations and workflows
(Jira, GitHub, Sonar, Veracode, Datadog, its own skills) live in plugins
kept in their own repositories, never in this workspace. This amends
ADR 0018's "no plugin system of our own". The plugin format is the app's:
`.<command>-plugin/plugin.json`, `skills/`, `agents/`, `commands/`,
`hooks/hooks.json`, `.mcp.json`, with `${<COMMAND>_PLUGIN_ROOT}` for the
plugin's folder. Claude Code's plugins and installs are not read. There is
no installer or marketplace: a person puts a plugin's folder in
`~/.<command>/plugins/` (a clone they `git pull`), and `/reload` shows what
is there. MCP servers also come from the project and the user.

## 0020 — Verification: a judge agent grades a run against criteria

A run can carry verification criteria: the user's (`/verification`,
`Session::set_verification`, `-p --verification`), else the `Verification`
section of the last plan. Before a top-level run completes, a judge agent
checks the work against them and emits a `Verdict` (passed, score 0–1, one
check per criterion with evidence). The judge gathers its own evidence: it
may run commands (tests, lint, the app through the `verify` and `run`
skills) under the run's permission mode, but has no editing tools, so it
reports and never fixes. A failed verdict sends the agent back with the
reasons, up to three attempts; then the run completes with the last
verdict. A judge that cannot grade fails the check but never the run.
Planning runs and sub-agents are not judged. The command is
`/verification`, not `/verify`: `verify` and `run` stay skills, which the
judge uses.

## 0021 — Only the app's folders

The app reads only its own files: `<COMMAND>.md` (and `.<command>/<COMMAND>.md`,
`<COMMAND>.local.md`, `~/.<command>/<COMMAND>.md`), and skills, commands,
agents, rules, output styles, settings and MCP servers in the project's
`.<command>/` and `~/.<command>/`. The project's MCP servers are in
`.<command>/mcp.json`, approved with `enabledMcpServers` /
`disabledMcpServers`. Nothing in `.claude/`, `~/.claude`, `CLAUDE.md` or a
root `.mcp.json` is read; `AGENTS.md`, which no single tool owns, still is.
Hooks get `<COMMAND>_PROJECT_DIR`.

## 0022 — Memory is reversible and kept without an LLM

Memory loses nothing at once and needs no model to stay tidy. This
amends ADR 0018's "no memory optimizer": the optimizer is back.

- A memory book that cannot be read is renamed aside
  (`.json.corrupt-<secs>`) and never written over; saves are synced before
  the rename. Without an app data folder memory lasts only for the process,
  never in a temp folder.
- Text passes `memory::sanitize` before it is kept: keys, tokens and
  passwords become `[redacted]`.
- Forgetting moves a memory to `forgotten`, restorable for 30 days;
  revising keeps the last five texts. Ids are never reused.
- Recall weighs episodes by retention: half every 30 days, plus
  `0.5·ln(1+uses)` fading from the last use. Being recalled or found by
  `memory_search` is use. Facts and procedures do not fade.
- With an embedding model, a new memory within cosine 0.92 of an older one
  of the same kind revises it instead of adding a copy.
- The optimizer is one agent run over the whole memory (`/memory optimize`,
  `ProjectMemory::optimize`): no recall, no `memory_search`; its edits can
  be undone as one (`UndoMemory`) until the next edit.

Rejected: a markdown wiki or SQLite as the store (a book is small; BM25
plus vectors already serve it), an approximate-nearest-neighbour index
such as FAISS (brute-force cosine over hundreds of vectors takes under a
millisecond, and a second index would drift from the book), and LLM
consolidation running in the background.

## 0023 — Language servers navigate code and check edits

The agent asks language servers instead of only searching text.

- Rules are pure in `domain::lsp`: the servers (a built-in table for
  rust-analyzer, typescript-language-server, pyright, gopls and clangd,
  changed by `lspServers` in each settings file, then a plugin's `.lsp.json`,
  by name and field by field; `disabled` removes one), the server for a
  file (its extension) and its root (the closest marker, never above the
  working folder), the questions and how answers read.
- `agent_engine::lsp`: `LspClient` speaks the protocol to one process;
  `LspPool` keeps one per server and root for the engine's life (in the
  daemon, across runs and clients), starts it on first use, restarts it if
  it ended, ends it after 10 idle minutes and does not retry one that
  failed to start until then.
- The `LSP` tool is offered only when one of the run's servers is
  installed, so it costs no tokens otherwise.
- After `Edit` or `Write`, the file's server gets the new text and the
  result carries its errors and warnings in `<diagnostics>` (waiting up to
  3 s); the diff the user sees does not change. No server, no answer or
  nothing wrong leaves the result as it was.

A server runs the project's code as its language does (build scripts,
proc macros). What runs is the owner's to say: a checked-in project
`settings.json` may disable servers and set their languages and root
markers, but its `command`, `args` and `env` are dropped
(`ServerConfig::without_commands`). Only the user's settings, the
project's `settings.local.json` and installed plugins set them, as with
the approval of a project's MCP servers.

The pool is single-owner: it is keyed by server and root, not by
`Principal`, which fits the local daemon (one owner per computer). A
multi-user server must add the principal to the key, so owners never
share a process or its open documents.

Rejected: an MCP server for LSP (one more process and protocol between the
agent and the server, and no way to add diagnostics to an edit), and a
server per run (rust-analyzer indexing anew on every prompt).

## 0024 — Released binaries and `<command> update`

Amends ADR 0018's "no release pipeline or packaging": release-please tags
`vX.Y.Z` on the repository and the release job attaches
`<command>-X.Y.Z-<target>.tar.gz` and its `.sha256` for macOS (Apple
silicon, signed and notarized) and Linux (x86_64, arm64). `cargo install`
still works.

- `<command> update` asks GitHub for the latest tag, installs it only when
  it is newer than this build, and refuses an archive whose SHA-256 does
  not match the published one. Which release and which archive are pure
  rules in `domain::release`; the download sits behind the CLI's
  `Releases` trait.
- The new binary is written beside the running one and renamed over it,
  so the file is always the whole old or the whole new build.
- It never stops the daemon (other terminals' runs would end); the daemon
  leaves 5 s after its last client, so the next session starts the new
  build.

Rejected: a package manager formula (one more thing to publish) and
stopping the daemon on update.
