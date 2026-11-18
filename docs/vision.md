# Vision

Status: Approved by the owner, 2026-11-17. Written from the 2026-11-16 audit; the owner's decisions of 2026-11-17 on remote control (`decisions/0012-remote-control-through-a-blind-relay.md`) and on experimental features are folded in.

This document names the product as "the app" and its binary as `<command>`; the real name lives in `config.toml`.

## The problem

A developer who uses more than one model provider ends up with more than one terminal agent. Each has its own configuration, permission rules, session history, extension format and habits. Moving a task from one model to another means moving to another tool and losing the conversation, the rules and the context that were built up.

The second problem is trust. A coding agent runs shell commands and edits files with the user's own privileges, on instructions from a model that reads untrusted text (repositories, web pages, tool servers). The user needs to know what the agent may do without asking, and needs that boundary to hold when the input is hostile.

## Who it is for

Individual developers who want one terminal agent across several model providers, working in their own repositories, on their own machine, and following or driving that work from their own phone when they are away from it.

It implies: one person, their own machine and devices, local files, no administrator, no team server.

## What it does that alternatives do not

These are the differentiators the code supports today, each with the part of the code that carries it.

- One agent, several providers. The same chat, tools, permission rules and session history work over API-key providers and sign-in providers, and over providers a user declares in a small JavaScript file (`llm-providers`, `script`).
- Models have roles. A session can use different models for the main loop, planning, exploring, review, a small model for auxiliary work and an advisor, with a fallback chain and several accounts per provider (`llm-providers`, `agent-engine`).
- The agent's behaviour is content, not code. Prompts, tool descriptions, hooks, output styles, built-in agents and skills are JavaScript and Markdown files embedded in the binary, and a user can override any of them (`content`).
- Runs are observable. Every run is recorded and can be inspected in a local web page (runs, spans, model calls, cost), beside the terminal (`domain::trace`, `console`, `page`). The same runs can be exported to the user's own OpenTelemetry collector over OTLP (`decisions/0014-hand-written-otlp-export.md`).
- Runs can be measured. `<command> eval` runs a folder of eval cases against a model and scores them, so a change to a prompt, a tool or a model can be judged on cases rather than by feel.
- Remote control without a hosted service. The owner follows and drives their own sessions, terminal included, from a paired phone, through a relay they host themselves that forwards end-to-end encrypted frames it cannot read (`tunnel`, `relay`; `decisions/0012-remote-control-through-a-blind-relay.md`).
- Everything is local-first. Sessions, memory, settings and credentials stay in the user's data folder; nothing requires a server run by the project. The only server is the optional relay, and it is the owner's.

## Scope today

`feature-flags.md` lists every feature flag, what it gates and what to do with it; it is the place for how many there are.

Core:

- Terminal chat (TUI) and a one-shot print mode (`-p`) with text, JSON and stream-JSON output.
- The agent loop with built-in tools: files (read, write, edit, glob, grep), shell (foreground, background, monitor), web (fetch, search), language-server queries, tasks, sub-agents.
- Permission modes (ask, edits, plan, bypass) and allow/ask/deny rules.
- Sessions: resume, fork, rewind and branch from checkpoints, compaction.
- Providers: connect by API key or sign-in, model catalog, model roles, fallback chain, several accounts.
- Extension points: instruction files, rules, hooks, skills, custom agents, script tools, plugins, MCP servers (HTTP and stdio). The app reads only its own files: its instruction files, `AGENTS.md` and the `.<command>/` folders (see non-goal 6).
- The local web page: trace of runs, console, user documentation, a terminal attached to a session.
- Trace export to an OpenTelemetry collector (OTLP over HTTP, opt-in through an environment variable).
- Evals: cases in a folder, run and scored by `<command> eval`.
- Remote control: the same page installed on a phone as a progressive web app, through a self-hosted relay, with the terminal on by default for paired devices (`decisions/0012-remote-control-through-a-blind-relay.md`).
- Subcommands: `login`, `logout`, `models`, `mcp`, `plugin`, `remote`, `config`, `doctor`, `eval`, `teammate` (started by a lead, not by hand).

Experimental:

- Every flag whose name starts with `experimental-`, among them the write sandbox, the dependency guard, pull-request follow and the tree-sitter code map. They ship on (see non-goal 5); `feature-flags.md` gives each one an owner and an exit criterion.
- Not experimental: the rest of the scope the audit listed without a cut criterion (project memory with seeds and optimisation, schedules and loops, fleet with several live sessions in one terminal, MCP channels, the verification judge) is stable scope under this vision, not flagged experiments with an exit criterion. Where a part of it is gated, the gate is named in `feature-flags.md`: the memory scorecard, seeds, learning and the learning check are `experimental-` flags; schedules, MCP channels and memory are backend feature switches (`schedules`, `channels`, `memory`) a user can turn off; the verification judge on checked-in criteria is `project-criteria`.
- Platforms: macOS and Linux are the tested targets. Windows is compiled but several platform functions are stubs.

## Non-goals

1. Not a multi-user or hosted service. One user, their own machine and devices. There is no account system, no organisation, no multi-user server, and no service run by the project. The one server the app has is the optional remote-control relay, which the owner hosts themselves (on a server of their own, or as a Worker on their own Cloudflare account) and which only forwards encrypted frames between the owner's machine and the owner's paired devices (see `decisions/0008-single-user-local-product.md` and `decisions/0012-remote-control-through-a-blind-relay.md`).
2. Not an embeddable SDK. The client code between the terminal and the engine is internal; no API is offered for building other products on top of the agent (see `decisions/0005-typed-application-api.md`).
3. Not an IDE or a GUI editor. The web page observes runs, documents the app and, from the owner's paired devices, drives the owner's sessions (the console and the terminal attached to a session); code is edited through the agent and the user's own editor.
4. Not a network-reachable surface. The web page binds to loopback with a per-process token and accepts no connection from the network. Remote control reaches a session only through a relay the owner runs, to which the machine connects out, and only from paired devices, end to end encrypted (see `decisions/0012-remote-control-through-a-blind-relay.md`).
5. Not breadth for its own sake. A feature without an owner and a user is removed. An experimental feature ships on, with an owner and an exit criterion; the owner turns it off or promotes it to permanent. The audit's finding that "nothing was refused" in the first eight days is the reason this is written down.
6. No Claude Code compatibility. The app reads only its own files; another tool's folders, files and environment variables (`.claude/`, `CLAUDE.md`, `CLAUDE_*`) are ignored, and a test keeps them out of a run.

## Open decisions

None is open. Closed:

- Subscription providers' client identity: out of scope by the owner's decision (legal and provider-identity points are excluded from this work); the providers keep their upstream client identity. It is not an open decision for engineering.
- License: MIT, in `LICENSE` and declared in `Cargo.toml`.
