# Decisions

Decisions that shape Superfik Lite. Code cites them as `ADR NNNN`; numbers
of decisions dropped with the desktop app are not reused.

## 0015 — Superfik is the engine

No agent CLI runs underneath. The agent loop is ours (`superfik-native`): it calls the provider's API itself, runs its tools as
Rust functions, and emits `EngineEvent`s, so clients are engine-agnostic.
Providers sit behind `ModelApi`, tested against captured streams; local
models run on MLX as Chat Completions. The system prompt, tools, sessions
and compaction are ours; the prompt loads `SUPERFIK.md` and names Superfik.

## 0016 — API keys and subscriptions are separate providers

OpenAI API, ChatGPT Subscription, Claude Subscription, Anthropic API,
GitHub Copilot Subscription and Kiro Subscription are independently connected providers, each with its own credentials and
model catalog. Pure contracts live in `superfik-providers`; the native
engine does the IO. The backend handles PKCE and loopback callbacks
and keeps credentials in its `SecretStore`, saving rotated refresh tokens;
runs carry a redacted handle a client cannot supply. Subscription adapters
apply their own preludes; the tools, loop and project instructions stay
Superfik's.

## 0018 — Lite: a CLI and an SDK

Superfik Lite drops the desktop canvas, the web client, the team server and
everything only they used (browser cards, voice, sound, haptics, notes,
routines, channels, vaults, review, visuals, updates, i18n). What remains is
the engine, the backend that drives it, memory, sessions, skills, agents
and MCP. Two crates come on top: a terminal chat (CLI/TUI) and an SDK for
building agents, commands, skills, memory access and in-process MCP tools,
and for connecting MCP servers over HTTP and stdio.

- The AI providers are LLM providers (`superfik-providers`,
  `ModelProviders`). Skills, agents and MCP come from Superfik's folders
  (ADR 0021), its plugins (ADR 0019) and the SDK host. No web search or
  fetch tool.
- One engine, ours. No registry of engines, no tool worker process, no
  git/workspace inspection, no memory optimizer, no session import.
- A separate product from the desktop Superfik: its own data folder,
  nothing shared, nothing migrated. Secrets sit in a sealed file, never
  the system keychain.
- The `Principal`/`Envelope`/`Authorizer` seam stays so a remote backend
  can come back without reshaping the SDK.
- Published as source (`brunozilio/superfik-lite`), with no license,
  release pipeline or packaging: install it with `cargo install`.

## 0019 — A platform: Superfik plugins

Superfik is a general platform; a company's integrations and workflows
(Jira, GitHub, Sonar, Veracode, Datadog, its own skills) live in plugins
kept in their own repositories, never in this workspace. This amends
ADR 0018's "no plugin system of our own". The plugin format is Superfik's:
`.superfik-plugin/plugin.json`, `skills/`, `agents/`, `commands/`,
`hooks/hooks.json`, `.mcp.json`, with `${SUPERFIK_PLUGIN_ROOT}` for the
plugin's folder. Claude Code's plugins and installs are not read. There is
no installer or marketplace: a person puts a plugin's folder in
`~/.superfik/plugins/` (a clone they `git pull`), and `/reload` shows what
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

## 0021 — Only Superfik's folders

Superfik reads only its own files: `SUPERFIK.md` (and `.superfik/SUPERFIK.md`,
`SUPERFIK.local.md`, `~/.superfik/SUPERFIK.md`), and skills, commands,
agents, rules, output styles, settings and MCP servers in the project's
`.superfik/` and `~/.superfik/`. The project's MCP servers are in
`.superfik/mcp.json`, approved with `enabledMcpServers` /
`disabledMcpServers`. Nothing in `.claude/`, `~/.claude`, `CLAUDE.md` or a
root `.mcp.json` is read; `AGENTS.md`, which no single tool owns, still is.
Hooks get `SUPERFIK_PROJECT_DIR`.
