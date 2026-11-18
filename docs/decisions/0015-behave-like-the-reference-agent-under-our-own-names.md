# 0015 Channels, code mode, observers and teams behave like the reference coding agent, under the app's own names

Status: Accepted, 2026-11-17

## Context

The owner asked for four features to work the way they do in the most widely used terminal coding agent (the reference agent below): channels, code mode, observer agents, and agent teams (a lead, teammates, a mailbox, shared tasks). The app already had parts of the first two. Their behaviour was read from the reference agent's released binary: tool names, input schemas, message formats, file layouts and limits. No code or prompt text was copied; the app reimplements the behaviour in its own crates.

The app had decided earlier not to be compatible with the reference agent's files (its memory switch and its folder names were removed). Behaving the same way and reading the same files are different things, so the owner decided each point where they meet.

## Decision

- **Names.** Every folder, file and environment variable is the app's own: `.<command>/teams/`, `.<command>/tasks/`, `<PREFIX>_…`. Protocol names that servers and models see (`notifications/claude/channel`, `claude/channel/permission`, tool names such as `SendMessage`) keep the reference spelling, so a server written for the reference agent works here unchanged.
- **Channels.** Each message reaches the agent as its own `<channel source=… key="value">` tag; meta keys must be identifiers, and dropped keys are logged. A server's `initialize` instructions reach the system prompt. A channel that declares `claude/channel/permission` gets each permission prompt and may answer it; the first answer wins, and a channel cannot allow its own tools. Channel messages are delivered in bypass mode too, as the reference agent does; messages from other sessions stay held there.
- **Code mode.** Turning code mode on makes the REPL the agent's only tool, as in the reference agent. Unlike the reference agent, where it is opt-in, it ships on, like every flag here (owner's decision, 2026-11-17).
- **Observers.** An agent's frontmatter names up to four observers. Each gets a digest of the watched agent's turn when it ends and answers only through `ObserverReport`. Where the reference agent keeps observers running beside the agent and queues their reports, the app runs them when the turn ends and, for the main agent, lets it go on once with their reports in the same turn: a report that arrived after the turn would start a new turn on its own, which an observer could then report on again. Declaring an observer is the opt-in, so there is no flag.
- **Teams.** A session's team, its members, mailboxes and protocol messages follow the reference agent, in `~/.<command>/teams/`. Teammates run as separate processes of the app's own binary (`<command> teammate`), the reference agent's pane backend without the pane: the engine's `Agent` borrows the model client, the tools and the workspace for one run, so a teammate that outlives the lead's turns cannot live in the lead's process without reworking that ownership. A teammate is a print-mode session fed from its mailbox; it tells the lead when it goes idle and exits on a shutdown request. The engine finds the program through `<PREFIX>_TEAMMATE_COMMAND` or its own path, so it depends on the CLI's `teammate` subcommand by name.
- **Order.** Channels, then code mode, then observers, then teams, one commit each. Teammates start as background processes; `teammateMode: tmux` (or `auto` inside tmux) opens each in a tmux pane instead, run by tmux directly, without a shell. iTerm2 panes are not offered: the reference agent drives them through a separate `it2` program, which the app does not depend on.

## Costs

- In bypass mode, whoever can post to an open channel steers an agent that does not ask (`docs/threat-model.md`).
- A channel that takes permission prompts can allow any call but its own tools; the request id is a hash, not a secret.
- MCP server instructions are third-party text in the system prompt, cut to 2,048 characters.
- A watched turn ends only once its observers have answered, so each observer adds a model call's wait and cost to every turn.
- A teammate's permission prompt waits, in its own process, until the user answers it in the lead's chat; a teammate whose lead is closed waits until its team is gone. Always-allow and plan approval are not forwarded.
- Each idle notification wakes the lead for a turn, as in the reference agent, which costs a model call per teammate answer.
- The reference agent's behaviour moves between releases; the app follows the version that was read (2.1.327), not later ones.
