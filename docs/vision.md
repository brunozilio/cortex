# Vision

Status: DRAFT written from the 2026-11-16 audit, for the owner to review; nothing here is approved until the sign-off listed under "Open decisions".

This document names the product as "the app" and its binary as `<command>`; the real name lives in `brand.toml`.

## The problem

A developer who uses more than one model provider ends up with more than one terminal agent. Each has its own configuration, permission rules, session history, extension format and habits. Moving a task from one model to another means moving to another tool and losing the conversation, the rules and the context that were built up.

The second problem is trust. A coding agent runs shell commands and edits files with the user's own privileges, on instructions from a model that reads untrusted text (repositories, web pages, tool servers). The user needs to know what the agent may do without asking, and needs that boundary to hold when the input is hostile.

## Who it is for

Individual developers who want one terminal agent across several model providers, working in their own repositories, on their own machine.

The audit found no written target user; this is the working assumption. It implies: one person, one machine, local files, no administrator, no team server.

## What it does that alternatives do not

These are the differentiators the code supports today. They are hypotheses to confirm, not claims to publish.

- One agent, several providers. The same chat, tools, permission rules and session history work over API-key providers and sign-in providers, and over providers a user declares in a small JavaScript file.
- Models have roles. A session can use different models for the main loop, planning, exploring, review, a small model for auxiliary work and an advisor, with a fallback chain and several accounts per provider.
- The agent's behaviour is content, not code. Prompts, tool descriptions, hooks, output styles, built-in agents and skills are JavaScript and Markdown files embedded in the binary, and a user can override any of them.
- Runs are observable. Every run is recorded and can be inspected in a local web page (runs, spans, model calls, cost), beside the terminal.
- Everything is local. Sessions, memory, settings and credentials stay in the user's data folder; nothing requires a server run by the project.

## Scope today

The audit found about 34 feature flags, 17 of them named experimental and all on by default, and no rule for cutting scope. Phase 2.5 of the plan assigns every flag a destiny (fixed behaviour, experimental with an owner and off by default, or removed). Until that inventory lands, the split below is provisional.

Core:

- Terminal chat (TUI) and a one-shot print mode (`-p`) with text, JSON and stream-JSON output.
- The agent loop with built-in tools: files (read, write, edit, glob, grep), shell (foreground, background, monitor), web (fetch, search), language-server queries, tasks, sub-agents.
- Permission modes (ask, edits, plan, bypass) and allow/ask/deny rules.
- Sessions: resume, fork, rewind and branch from checkpoints, compaction.
- Providers: connect by API key or sign-in, model catalog, model roles, fallback chain, several accounts.
- Extension points: instruction files, rules, hooks, skills, custom agents, script tools, plugins, MCP servers (HTTP and stdio).
- The local web page: trace of runs, console, user documentation, a terminal attached to a session.
- Subcommands: `login`, `logout`, `models`, `mcp`, `plugin`, `config`, `doctor`, `eval`.

Experimental or undecided (to be classified in phase 2.5):

- Named experimental in code: the write sandbox, the dependency guard, pull-request follow, the tree-sitter code map.
- Listed by the audit as scope without a cut criterion: project memory with scorecard, seeds and optimisation; learning; schedules and loops; fleet (several live sessions in one terminal); MCP channels; the verification judge.
- Platforms: macOS and Linux are the tested targets. Windows is compiled but several platform functions are stubs.

## Non-goals

1. Not a multi-user or hosted service. One user, one machine. There is no account system, no organisation, no remote server (see `decisions/0008-single-user-local-product.md`).
2. Not an embeddable SDK. The client code between the terminal and the engine is internal; no API is offered for building other products on top of the agent (see `decisions/0005-typed-application-api.md`).
3. Not an IDE or a GUI editor. The local web page is for observing runs and reading documentation; code is edited through the agent and the user's own editor.
4. Not a network-reachable surface. The web page binds to loopback with a per-process token and is not meant to be exposed beyond the machine. Remote control of a session over a network is out of scope.
5. Not breadth for its own sake. A feature without an owner and a user is removed, and an experimental feature starts off. The audit's finding that "nothing was refused" in the first eight days is the reason this is written down.

## Open decisions

Owned by the tech lead. This document does not take a position on them.

- Subscription providers' client identity.
- License.
- Product-vision sign-off: this document, including the target user and the five non-goals.
