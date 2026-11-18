# 0015 Channels, code mode, observers and teams behave like the reference coding agent, under the app's own names

Status: Accepted, 2026-11-17

## Context

The owner asked for four features to work the way they do in the most widely used terminal coding agent (the reference agent below): channels, code mode, observer agents, and agent teams (a lead, teammates, a mailbox, shared tasks). The app already had parts of the first two. Their behaviour was read from the reference agent's released binary: tool names, input schemas, message formats, file layouts and limits. No code or prompt text was copied; the app reimplements the behaviour in its own crates.

The app had decided earlier not to be compatible with the reference agent's files (its memory switch and its folder names were removed). Behaving the same way and reading the same files are different things, so the owner decided each point where they meet.

## Decision

- **Names.** Every folder, file and environment variable is the app's own: `.<command>/teams/`, `.<command>/tasks/`, `<PREFIX>_…`. Protocol names that servers and models see (`notifications/claude/channel`, `claude/channel/permission`, tool names such as `SendMessage`) keep the reference spelling, so a server written for the reference agent works here unchanged.
- **Channels.** Each message reaches the agent as its own `<channel source=… key="value">` tag; meta keys must be identifiers, and dropped keys are logged. A server's `initialize` instructions reach the system prompt. A channel that declares `claude/channel/permission` gets each permission prompt and may answer it; the first answer wins, and a channel cannot allow its own tools. Channel messages are delivered in bypass mode too, as the reference agent does; messages from other sessions stay held there.
- **Code mode.** Turning code mode on makes the REPL the agent's only tool, as in the reference agent. Unlike the reference agent, where it is opt-in, it ships on, like every flag here (owner's decision, 2026-11-17).
- **Observers.** An agent's frontmatter names up to four observers. As in the reference agent, observers run beside the agent: after each step that ran tools, each free observer is handed, in a background job of the run, a digest of what happened since its last one, and its reports reach the agent between steps through the inbox. They answer only through `ObserverReport`. When the turn ends, the run waits for digests still being read, sends the last items with how the turn ended, and takes back any report the agent has not read: for the main agent it goes on once with them in the same turn, because a report left for after the turn would start a new turn on its own, which an observer could then report on again. Declaring an observer is the opt-in, so there is no flag.
- **Teams.** A session's team, its members, mailboxes and protocol messages follow the reference agent, in `~/.<command>/teams/`. Teammates run as separate processes of the app's own binary (`<command> teammate`), the reference agent's pane backend without the pane: the engine's `Agent` borrows the model client, the tools and the workspace for one run, so a teammate that outlives the lead's turns cannot live in the lead's process without reworking that ownership. A teammate is a print-mode session fed from its mailbox; it tells the lead when it goes idle and exits on a shutdown request. The engine finds the program through `<PREFIX>_TEAMMATE_COMMAND` or its own path, so it depends on the CLI's `teammate` subcommand by name.
- **Order.** Channels, then code mode, then observers, then teams, one commit each. Teammates start as background processes; `teammateMode: tmux` (or `auto` inside tmux) opens each in a tmux pane instead, run by tmux directly, without a shell. `teammateMode: iterm2` (or `auto` inside iTerm2) opens an iTerm2 pane through AppleScript (`osascript`, part of macOS) instead of the reference agent's separate `it2` program; the pane's command holds only the program's path and a spec file's path, both limited to letters, digits and `/._-`, and the teammate reads its team, name, folder, model, mode and instructions from that file (`teammate --spec`), so no shell sees them.

## Costs

- In bypass mode, whoever can post to an open channel steers an agent that does not ask (`docs/threat-model.md`).
- A channel that takes permission prompts can allow any call but its own tools; the request id is a hash, not a secret.
- MCP server instructions are third-party text in the system prompt, cut to 2,048 characters.
- Each observer adds model calls to every step that runs tools, and a watched turn ends only once its observers have read the end of it, which adds one model call's wait to the end of every turn.
- A teammate's permission prompt or plan waits, in its own process, until the user answers it in the lead's chat; when the lead's session closes, its teammates are asked to shut down and stop waiting. An iTerm2 pane needs paths without spaces or quotes.
- Each idle notification wakes the lead for a turn, as in the reference agent, which costs a model call per teammate answer.
- The reference agent's behaviour moves between releases; the app follows the version that was read (2.1.327), not later ones.
