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

OpenAI API, ChatGPT Subscription, Claude Subscription and Anthropic API are
four independently connected providers, each with its own credentials and
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
  `ModelProviders`). There is no plugin system of our own: skills, agents
  and MCP come from Claude Code's folders and from the SDK host. No web
  search or fetch tool.
- One engine, ours. No registry of engines, no tool worker process, no
  git/workspace inspection, no memory optimizer, no session import.
- A separate product from the desktop Superfik: its own data folder,
  nothing shared, nothing migrated. Secrets sit in a sealed file, never
  the system keychain.
- The `Principal`/`Envelope`/`Authorizer` seam stays so a remote backend
  can come back without reshaping the SDK.
- Published as source (`brunozilio/superfik-lite`), with no license,
  release pipeline or packaging: install it with `cargo install`.
