---
title: Agents
description: Define sub-agents with their own instructions, tools, model and permissions, and let {{NAME}} delegate focused tasks to them.
---

A **sub-agent** is a separate agent that {{NAME}} starts for one task. It works in its own conversation, with its own instructions and tools, and sends back a single report. Delegating keeps the main conversation focused: the sub-agent reads twenty files, and only its conclusion comes back.

{{NAME}} has built-in sub-agents, and you can define your own as Markdown files.

## Quick start

Create `{{DIR}}/agents/reviewer.md` in your project:

```markdown
---
name: reviewer
description: Reviews a diff for bugs and missing tests. Use after making a change, before committing.
tools: Read, Grep, Glob, Bash
model: inherit
---

You are a strict code reviewer. Read the diff with `git diff`, then the
changed files. Report each bug with its file and line, and each change that
has no test. Do not edit anything.
```

From the next prompt, the agent can delegate to it ("have the reviewer check this"), and it picks it on its own when the description matches the task. `/agents` lists every agent available here.

> [!TIP]
> `/agents new <what it should do>` asks the agent to write the file for you in `{{DIR}}/agents/<name>.md`.

## How delegation works

The main agent delegates with the `Agent` tool:

| Input | Meaning |
|---|---|
| `description` | A 3 to 5 word summary, shown in the chat. Required. |
| `prompt` | The task. The sub-agent knows only what is written here. Required. |
| `subagent_type` | The agent to start; `general-purpose` when left out |
| `run_in_background` | `true` runs it while the main agent goes on; its report is collected with `TaskOutput` |
| `isolation` | `worktree` runs it in a git worktree of its own |
| `max_turns` | At most this many model turns, 1 to 200 |
| `model` | A model for this one call; ignored if the current provider does not serve it |

The tool's description lists every available agent with its description, so a clear `description` is what makes the agent pick yours.

Rules of delegation:

- A sub-agent cannot start other sub-agents.
- Its report is returned to the main agent, which passes on what matters; it is not shown to you as is.
- A sub-agent gets the same instruction files as the main agent, unless it is a read-only built-in agent or sets `omitClaudeMd: true`. It does not get the conversation's output style.
- Hooks fire for it too: `SubagentStart` when it starts, `SubagentStop` when it wants to finish (see [Hooks](./hooks)).

## Built-in agents

| Agent | What it is for | Tools |
|---|---|---|
| `general-purpose` | Research and tasks of several steps (the default) | All of the main agent's |
| `Explore` | Fast read-only search: files by pattern, symbols, "where is X defined" | No `Agent`, `Edit`, `Write` or `StructuredOutput`; starts without instruction files |
| `Plan` | Designs an implementation plan: steps, critical files, trade-offs | Same as `Explore` |
| `fork` | A copy of the main agent with its whole conversation and model, for one side task | All of the main agent's |

`Explore` uses the small model when it names none (see [Models and providers](./models-and-providers)). A file of yours with the same name replaces a built-in agent.

## Where agents live

| Location | Scope |
|---|---|
| `{{DIR}}/agents/*.md` in the working folder or any folder above it | The project |
| `~/{{DIR}}/agents/*.md` | You, every project |
| `<plugin>/agents/*.md` | An installed plugin, named `<plugin>:<name>` |

When two agents have the same name, the first one found wins: the working folder, each folder above it, yours, plugins, then the built-in agents. Agents are found again at the start of every prompt.

> [!NOTE]
> Agents in another tool's folders, such as `.claude/agents/`, are not read.

## File format

An agent is a Markdown file with YAML frontmatter. The body is its instructions, added to its system prompt under `# Your instructions (<name>)`.

```markdown
---
name: migrations
description: Writes and checks database migrations. Use for any schema change.
tools: [Read, Edit, Write, Bash, Grep, Glob]
disallowedTools: [WebFetch]
model: inherit
effort: high
maxTurns: 60
permissionMode: acceptEdits
skills: [db-conventions]
memory: project
isolation: worktree
---

You write reversible migrations...
```

| Field | Value | Meaning |
|---|---|---|
| `name` | Text | The agent's name. Defaults to the file's name without `.md`. |
| `description` | Text | When to use it. The main agent chooses from this. |
| `tools` | List or comma-separated text | The only tools it gets. A name ending in `*` matches by prefix: `mcp__docs__*`. All tools when left out. |
| `disallowedTools` | List or comma-separated text | Tools it never gets, with the same syntax |
| `mcpServers` | List or comma-separated text | The only MCP servers whose tools it gets |
| `model` | A model id, a short name, `provider:model`, or `inherit` | Its model. `inherit` (or nothing) uses the main agent's. |
| `effort` | `low`, `medium`, `high`, `xhigh` or `max` | Its effort level |
| `maxTurns` | 1 to 200 | At most this many model turns |
| `permissionMode` | `default`, `acceptEdits`, `plan`, `bypassPermissions` or `dontAsk` | The most it may do without asking (see below) |
| `skills` | List or comma-separated text | Skills loaded into its system prompt when it starts |
| `memory` | `user`, `project` or `local` | Gives it a memory folder of its own (see below) |
| `isolation` | `worktree` | Runs it in a git worktree of its own |
| `background` | `true` or `false` | Runs it in the background unless the call says otherwise |
| `omitClaudeMd` | `true` or `false` | `true` starts it without the instruction files |
| `hooks` | Like `"hooks"` in settings | Hooks that apply only while it runs |
| `initialPrompt` | Text | Used when it is the main agent (see below) |
| `autoCompactWindow` | 100000 to 1000000 | Compacts its conversation once it reaches this many tokens |
| `observer` | Agent names, up to 4 | Agents that watch this one's turns (see [Observers](#observers)) |
| `observerMessage` | Text | Added at the end of every digest its observers get |
| `observeSubagents` | `true` (default) or `false` | Whether its sub-agents are watched by the same observers |

A tool selector may contain only letters, digits, `_`, `-` and `:`, plus a final `*`; `Bash(git:*)` is not a selector. A file with an invalid field still appears in the list, but starting it fails with `Agent <name>: <problem>`, for example `Agent migrations: maxTurns must be within 1..200`.

A project's agent keeps its `hooks`, `mcpServers`, `permissionMode` and `initialPrompt` only once you trust the project's folder; until then they are dropped and the agent is offered without them (see [Workspace trust](./sandbox-and-safety#workspace-trust)).

### Model

| `model` | Result |
|---|---|
| Left out or `inherit` | The main agent's model |
| A short name the provider knows, such as `haiku` | That provider's model for it |
| A model the current provider serves | That model |
| `provider:model` for a connected provider | That provider's model |

Anything else makes the call fail with *the agent model must be served by the current provider or name a connected one as provider:model*. A `ModelRequest` hook with matcher `sub-agent` can also route every sub-agent's calls (see [Hooks](./hooks#modelrequest)).

### Permission mode

`permissionMode` is a ceiling: it can lower what the sub-agent may do, never raise it above the main conversation's mode. `bypassPermissions` under an `ask` conversation still asks.

| Value | Mode |
|---|---|
| `default` | `ask` |
| `acceptEdits` | `edits` |
| `plan` | `plan` (read only) |
| `bypassPermissions` | `bypass`, if the conversation allows it |
| `dontAsk` | `ask`, but never shows a prompt: what would be asked is refused |

The app's own names (`ask`, `edits`, `plan`, `bypass`) and the other spellings listed in [Permission modes](./permissions#permission-modes) are read too, in any letter case.

### Memory

`memory` gives the agent a private folder that survives between runs, and the `Read`, `Write` and `Edit` tools to use it:

| Value | Folder |
|---|---|
| `user` | `~/{{DIR}}/agent-memory/…` |
| `project` | `<repository>/{{DIR}}/agent-memory/…` |
| `local` | `<repository>/{{DIR}}/agent-memory-local/…` (keep it out of git) |

Each agent gets its own subfolder. The first 200 lines (at most 25 KB) of its `MEMORY.md` are added to its system prompt, and it is told to keep that index short and to store details in other files. This memory is always on for an agent that declares a `memory` scope; leave `memory` out to keep an agent from remembering (see [Memory](./memory)).

### Worktree isolation

`isolation: worktree` (or `isolation` in the `Agent` call) runs the agent in a new git worktree, so its edits do not touch your working tree until you take them. When it changed something, the worktree is kept and its path is added to the report (*Changes retained in worktree: …*). Outside a git repository with at least one commit, the agent runs without isolation.

### Hooks in an agent

```markdown
---
name: careful-editor
description: Makes risky edits with checks.
hooks:
  PostToolUse:
    - matcher: "^(Edit|Write)$"
      hooks:
        - type: command
          command: cargo check --quiet
  Stop:
    - hooks:
        - type: command
          command: ./scripts/verify.sh
---
```

These hooks run only while this agent works. A `Stop` hook here runs as `SubagentStop` when the agent is a sub-agent.

### Plugin agents

Agents from a plugin are named `<plugin>:<name>`. For safety, their `hooks`, `mcpServers`, `permissionMode` and `initialPrompt` fields are ignored.

### Observers

An observer is an agent that watches another one's work and speaks up only when something is worth it. Name it in the watched agent's `observer` field (or in the main agent's, see below):

```markdown
---
name: migrations
description: Writes database migrations
observer: reviewer
observerMessage: Flag any migration that drops a column.
---
```

- While the watched agent works, each observer gets a digest of what happened since its last one: the request, the tool calls and their results, what the agent said, and, at the end, how the turn ended. A digest goes out after each step that ran tools, in the background, so the agent does not wait; an observer still reading the last one gets the next items later, together. The digest is wrapped in a tag with a fresh nonce and told to the observer as data, not instructions. Each item is cut to 2,000 characters (the request to 16,000), and a digest keeps at most about 64,000 characters, dropping the oldest items first.
- The observer runs on its own definition's instructions and model, with one tool, `ObserverReport`. It keeps its conversation across turns of the same session, so it can follow the work.
- A report reaches the watched agent as advice from its observer, never as your words or consent. It reaches the agent between its steps, while it works. A report that is still unread when the turn ends makes the main agent go on once in the same turn to weigh it, instead of starting a turn of its own; a sub-agent's observer reports reach the main agent between its steps.
- When the turn ends, it waits for digests still being read and sends the last items with how the turn ended. A turn you stop is not observed further.
- Sub-agents inherit their parent's observers, two levels down, unless `observeSubagents: false`. A sub-agent that names its own observers uses those instead. An observer is never observed itself.
- An observer name that matches no agent is skipped and logged; past four names, the rest are left out.

## Running the conversation as an agent

The `agent` setting makes the main conversation itself run as one of your agents:

```json
{ "agent": "migrations" }
```

The agent's instructions are added to the system prompt under `# Agent instructions (<name>)`, and its `tools`, `disallowedTools`, `mcpServers`, `model`, `effort`, `permissionMode`, `skills`, `memory`, `isolation` and `hooks` apply to the conversation. Its `initialPrompt` is put before your first message of a new conversation; an `initialPrompt` that starts with `/` runs that skill or command.

## Agent teams

A sub-agent works once and reports. A **teammate** keeps working: it runs in its own process, takes messages while it lives, and waits idle between tasks. The agent you talk to is the team's **lead** (`team-lead`); it starts teammates and coordinates them.

- **Starting one.** The lead calls `Agent` with a `name` (letters, digits, `_`, `-`) and the task in `prompt`, optionally a `subagent_type` and a `model`. A name already taken in the team gets `-2`, `-3`…; `main`, `team-lead` and `user` are reserved. The teammate runs `{{COMMAND}} teammate` in the same folder, with the lead's model (unless another is asked for) and the lead's permission mode, and the agent definition's instructions. With `teammateMode` set to `tmux` or `iterm2` (or `auto` inside either) it opens in a new pane beside the lead and prints there; otherwise it runs in the background and prints to its log. An iTerm2 pane is opened through AppleScript and reads its settings from `spawn/<name>.json` in the team folder, so the app's path and your home folder may hold only letters, digits and `/._-`.
- **The team.** Each session has one team, created with its first teammate, in `~/{{DIR}}/teams/team-<session>/`: `config.json` lists the members, `inboxes/<name>.json` holds each one's mail, and `logs/<name>.log` what each teammate printed. A teammate that exits is marked inactive.
- **Messages.** `SendMessage` takes a teammate's name (or `team-lead`, also spelled `main`, from a teammate), a `message` and a short `summary`. Mail reaches the lead between its steps, or as a turn of its own when the chat is idle, as `<teammate-message teammate_id="…" summary="…">`. A teammate reads its mail when it is idle; the lead's task arrives as its first message.
- **Idle and shutdown.** After each answer a teammate tells the lead it is idle (`idle_notification`). The lead ends a teammate with `SendMessage` and `message: {"type": "shutdown_request", "reason": "…"}`; the teammate answers `shutdown_approved` and exits. `shutdown_response` (with `request_id` and `approve`) and `plan_approval_response` are the other structured messages.
- **Permission prompts.** A teammate cannot ask you itself: when one of its calls needs your permission, it sends the prompt to the lead (`permission_request`) and waits. The lead's chat asks you, as *Bash (teammate tester)* with what the call will do, one prompt after another; your Yes or No goes back to that teammate only (`permission_response`). The lead's model never sees these prompts. *Yes, and don't ask again* is offered when the teammate could keep the rule, and the teammate keeps it for its own session. A plan a teammate wants approved (`plan_approval_request`) is shown to you the same way; approving it sets the mode the teammate carries it out in, and a refusal with a note sends the note back. When the lead's session closes, every teammate still working is asked to shut down, and one waiting for an answer stops waiting.
- **Who can do what.** Only the lead starts teammates; a teammate, or a sub-agent, cannot. A teammate reports with `SendMessage` to `team-lead`: what it prints is not read by anyone but its log.

- **Shared tasks.** `TaskCreate`, `TaskUpdate`, `TaskGet` and `TaskList` work on one list per lead session, in `~/{{DIR}}/tasks/<session>/tasks.json`, shared by the lead and every teammate and kept across turns. Ids are `1`, `2`…; each task shows its `owner`, what it waits for (`blocked_by`) and what it blocks. `TaskUpdate` with `owner` claims a task; a task someone else holds cannot be claimed (the lead can reassign it), a task cannot start or end while one it waits for is open, and giving a task to a teammate sends them a `task_assignment` message. A teammate that stops hands its open tasks back, and the lead is told which.

## Listing agents

`/agents` lists every agent available here (yours, the project's, the plugins' and the built-in ones) with its description. `/reload` re-reads them and shows how many were found.

## Changing agents from a hook

A `Brief` hook receives every agent found (name, description, instructions and configuration) and can return a new list, for example to add a team's reviewer or hide one. See [Hooks](./hooks#brief).

## See also

- [Skills](./skills) to preload knowledge into an agent.
- [Permissions](./permissions) for modes and rules.
- [Branches and git](./branches-and-git) for worktrees.
