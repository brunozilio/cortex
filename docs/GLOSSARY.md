# Glossary

The project's own vocabulary. Code carries no comments (see `decisions/0010-comments-stay-out-of-code.md`), so a name is the only documentation next to the code and this file is where a name is explained. Paths are relative to the repository root and point at the main place a term lives today; the restructuring plan moves several of them.

The product is written as "the app", its binary as `<command>`, its project folder as `.<command>/` and its user folder as `~/.<command>/`.

## Runs and sessions

**Run.** One execution of the engine: one prompt (or one session command such as compact or rewind) from start to a terminal event. It has a `RunId`, runs on its own thread and ends with exactly one of `Completed`, `Failed`, `Cancelled`. Lives in `crates/agent-engine/src/engine/mod.rs`; the events are in `crates/domain/src/events.rs`.

**Turn.** One iteration of the agent loop inside a run: one model call plus the tool calls that reply asked for. A run has at most 200 turns. It does not mean "one user prompt and its answer"; that is a run. The TUI also has a `Turn` struct (`crates/cli/src/tui/turn.rs`) that holds the state of the run in progress, which is a different thing. Lives in `crates/agent-engine/src/agent/mod.rs`.

**Session.** A conversation kept on disk and continued across runs. It has a `SessionId` (a free string, a UUID when the engine creates it) and belongs to one project folder. Today nothing of a session stays in memory between runs: each prompt reloads it. The `SessionStart` and `SessionEnd` hooks fire once per run today, not once per session (see `decisions/0001-long-lived-session.md`). Lives in `crates/agent-engine/src/sessions/mod.rs`.

**Job.** The object that prepares and drives one run. It is not a queue entry and there is no scheduler; "job" and "run" have the same lifetime. Lives in `crates/agent-engine/src/engine/job.rs`.

**Run gate.** The point every engine event passes through on its way out. It lets exactly one terminal event through and nothing after it, so a caller can rely on a run ending once. Lives in `crates/agent-engine/src/boundary/gate.rs`.

**Brief.** What the engine gathers from a folder before a run: plugins, settings, skills, agents, rules, instruction files and a git snapshot. It is rebuilt for every run today. `Brief` is also the name of a hook event that may rewrite that result, and `MemoryBrief` (`crates/domain/src/memory/brief.rs`) is the unrelated piece of memory that goes into a request. Lives in `crates/agent-engine/src/brief.rs`.

**Checkpoint / rewind.** A checkpoint is taken once per prompt and keeps the previous content of every file the run edited, with the position in the conversation. Rewind restores those files and cuts the conversation back to that prompt (the cut is skipped if a compaction changed the history since). Branch starts a new conversation from before a prompt. Lives in `crates/agent-engine/src/checkpoints.rs`. The `Checkpoint` in `crates/domain/src/conversation/checkpoint.rs` is a separate, test-only mechanism scheduled for removal.

**Upkeep / compaction.** Upkeep is the module that keeps a conversation inside the model's context window: it measures usage, replaces old tool results with a marker, and compacts. Compaction asks the model to summarise the older part of the history and keeps the most recent tail verbatim. It runs automatically near the limit, after a "too long" error from the provider, or on `/compact`. Lives in `crates/agent-engine/src/upkeep/`.

**Chore.** An auxiliary model call made around a run, not part of the conversation: session title, next-prompt suggestion, recap on resume, side question, heads-up, branch name, lesson. The TUI starts several after each completed run, so they cost model calls the user does not see. A chore is not a background job and is not recorded in the session catalog. Lives in `crates/agent-engine/src/chores/mod.rs`; triggered from `crates/cli/src/tui/chores.rs`.

**Steer.** Text the user sends while a run is in progress. It is queued and delivered to the model with the next tool results or at the end of the turn; "unsteer" takes back what was not delivered yet. It does not interrupt the run. Lives in `crates/agent-engine/src/control.rs`.

**Ask.** A question the engine puts to the user in the middle of a run, which waits for the answer: a permission request, a set of questions, or a plan approval. The type is `AgentAsk`; the answer is a `Decision`. "Ask" is also the name of a permission mode and of a rule kind. Lives in `crates/domain/src/approval.rs`.

**Fleet.** The sessions that are live in one terminal process, shown by `/fleet`. It is a list of sessions, not a group of agents or machines. Lives in `crates/cli/src/tui/fleet/mod.rs`.

**Parked session.** A live session in the fleet that is not the one on screen. Its state is held aside and swapped into the TUI's `App` when one of its events arrives. Lives in `crates/cli/src/tui/fleet/parked.rs`.

**Peer.** Another client in the same process, registered under a name built from its folder and client number. The `ListSessions` tool lists peers and `SendMessage` can address one. Lives in `crates/agent-engine/src/tools/peers.rs`.

## Permissions

**Permission mode.** How much the agent may do without asking: `Ask`, `AcceptEdits` (the default, shown as "edits"), `Plan` (no edits, no execution), `Bypass`. There is no fifth "auto" mode: the safety classifier that reviews actions in bypass and the automatic model picker are separate mechanisms. Lives in `crates/domain/src/approval.rs`.

**Access.** The class of a tool call that the permission decision uses today: `Reads`, `Edits`, `Executes`, and `ReadsOutside`, which no tool declares and which is derived when a reading tool reaches outside the project. A tool's access is declared as a string in its `content/tools/<Name>.js` file, not in Rust. Lives in `crates/agent-engine/src/tools/mod.rs`. `Access` in `crates/domain/src/model.rs` is unrelated: it is how a provider is signed in to (key, browser, code).

**Effect.** The planned replacement for access: what one call will do (read a path, write a path, execute a command, reach a host, spawn), declared by the tool in Rust and judged by one policy engine. It does not exist in the code yet; see `decisions/0003-effects-based-permission-policy.md`.

**Permission rule.** An allow, ask or deny line, written `Tool` or `Tool(specifier)`. Deny wins over ask, ask over allow; with no rule the mode decides. Lives in `crates/domain/src/permission_rules/mod.rs`.

## Models

**Provider vs vendor.** A provider is a way to reach models: an endpoint plus an authentication method (an API key, a sign-in). A vendor is a model family: its prices, aliases and reasoning rules. The same vendor's models can be reached through more than one provider. The `Provider` trait is in `crates/llm-providers/src/lib.rs`, one folder per provider is in `crates/agent-engine/src/providers/`, and `Vendor` is in `crates/llm-providers/src/vendor.rs`.

**Wire.** The request and streaming format of a model API: `chat`, `messages`, `responses`. A provider names the wire each of its models speaks. It is not the protocol between the terminal and the backend. Lives in `crates/agent-engine/src/model/wires/`.

**Model roles: small model, advisor, judge.** A session can assign different models to different jobs. The small model does cheap auxiliary work, such as reading a fetched web page before the main model sees it. The advisor is a second model the agent can consult through the `Advisor` tool. The judge is the model that decides whether a run met its verification criteria. Other roles are plan, explore and review. Roles are in `crates/domain/src/model_roles/mod.rs` and resolved in `crates/agent-engine/src/engine/roles.rs`.

**Verification.** Criteria that a run's result is judged against when it finishes, taken from the user, from the plan, or from the project's `verification.md`. A failed verdict sends the agent back to work, up to three attempts; nothing is verified in plan mode. It is not the `verify` skill and not the repository's own test gate. Lives in `crates/domain/src/verification/mod.rs`.

## Configuration

**Workspace vs project.** `Workspace` is an engine type: the per-run context handed to every tool (folders, policy, models, services). It is not a folder and not the Cargo workspace. The project is the folder the user works in, with the `.<command>/` folders of it and its ancestors. "Workspace trust" in `decisions/0004-workspace-trust.md` is about the project folder. `Workspace` lives in `crates/agent-engine/src/tools/workspace/mod.rs`.

**Settings vs UI state.** Settings are the configuration files a user or a project writes: `~/.<command>/settings.json`, `<project>/.<command>/settings.json` and `settings.local.json` (rules, hooks, flags, model roles). They are read by `crates/agent-engine/src/prompt/settings.rs`. UI state is what the TUI remembers for itself (model, mode, prompt history, last session per folder), stored in the data folder. The code calls both "settings"; the UI state is written by `crates/cli/src/tui/settings.rs` through `crates/backend/src/settings.rs`.

**Feature vs feature flag.** A feature is a folder of code that plugs into a host: `crates/backend/src/features/<name>/` on the backend and `crates/cli/src/tui/features/<name>/` in the TUI. A feature flag is a named on/off switch in the settings' `features` key, defined in `crates/domain/src/features.rs`; every flag is on unless turned off, except the opt-in ones (`experimental-code-mode-only`), which are off until the owner turns them on. A Cargo feature is a third, unrelated thing.

**Content.** The `content/` folder: prompts, tool descriptions, hooks, output styles, built-in agents and skills, written in JavaScript and Markdown and embedded in the binary at build time. A file with the same path under `~/.<command>/` overrides the embedded one. It does not mean message content. The `content` crate is in `crates/content/src/lib.rs`; the JavaScript contract is `content/types.d.ts`.

**Hook events.** The points where user code can observe or change a run. There are seventeen: `Brief`; `SessionStart`, `SessionEnd`, `UserPromptSubmit`; `ModelRequest`, `ModelResponse`, `ModelError`; `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `PermissionRequest`; `PreCompact`, `PostCompact`; `Stop`, `SubagentStart`, `SubagentStop`; `Notification`. A hook is a shell command, an HTTP call, a script, a model prompt or a sub-agent. The list is in `crates/agent-engine/src/hooks/validate.rs`.

**Script tool.** A tool a user or project writes in JavaScript under `.<command>/tools/`, run in the embedded QuickJS runtime. It is distinct from the `content/tools/<Name>.js` files, which describe and wrap the built-in Rust tools. Lives in `crates/agent-engine/src/tools/script_tool.rs`; the runtime is `crates/script/src/lib.rs`.

**Code mode / REPL.** The `REPL` tool, behind the `experimental-code-mode` flag: the model writes JavaScript in which every other tool it was offered is an async function, and only what the script returns or passes to `self.log` reaches the model. Each call inside a script is an ordinary tool call, with its hooks, rules and permission asks. With the opt-in `experimental-code-mode-only` flag, REPL is the only tool besides the turn-level ones and every other tool is declared inside its description. The JavaScript context persists per session and agent, so what one script declares the next can read; it is a `script::session::Session`, distinct from the conversation **Session** above and from the one-shot runs of script tools. See `decisions/0011-code-mode.md`. The tool is in `crates/agent-engine/src/tools/repl.rs` and `crates/agent-engine/src/agent/repl.rs`, the context in `crates/script/src/session.rs`, the output rules in `crates/domain/src/code_mode.rs`.

**Skill.** A folder with a `SKILL.md` (front matter plus instructions, optionally supporting files) that the model loads through the `Skill` tool or the user runs as `/name`. Built-in skills are in `content/skills/`. Lives in `crates/agent-engine/src/tools/skill.rs`.

**Plugin.** A git checkout under `~/.<command>/plugins/<name>/` that contributes hooks, MCP servers and language-server definitions. Lives in `crates/agent-engine/src/plugins.rs`.

**Worktree.** A git worktree the engine creates under `<repository>/.<command>/worktrees/` so a sub-agent or a session can work in isolation. It is removed at the end if nothing changed and kept otherwise. Lives in `crates/agent-engine/src/agent_runtime/mod.rs`.

## Memory and messages to the agent

**Memory kinds.** A project memory entry is `Episodic`, `Semantic` or `Procedural`. Only episodic entries decay with time; an entry given without a kind is stored as semantic. Lives in `crates/domain/src/memory/mod.rs`. Three different stores are all called memory: project memory (per project, in the data folder), team memory (Markdown files in `<project>/.<command>/memory/`) and agent memory (a `MEMORY.md` per custom agent).

**Inbox / notice / channel.** The inbox is a queue of short messages waiting to be shown to the agent during a run. A notice is one such message, for example a line from a monitored background command. A channel is an MCP server that stays open for the session and pushes notices into the inbox. Notices are treated as untrusted data. The rules are in `crates/domain/src/inbox.rs`, the queue in `crates/agent-engine/src/tools/inbox.rs`, channels in `crates/agent-engine/src/mcp/channel.rs`. The in-process message channels between threads are unrelated.

## Observability

**Trace / lane / span.** The trace is the record of what happened in a session's runs: prompts, events, model calls. It is written into the session's log as the run proceeds and projected three ways. The first projection is a `Trace` holding a `Lane` of steps, each step a set of timed nodes. The second is a list of runs, each a tree of `Span`s (run, model call, tool, agent, task, eval, wait, event); this is what the trace page draws. The third is the report with totals and insights. Lives in `crates/domain/src/trace/`.

## Names that collide

The audit lists these; until they are renamed, check which one a file means.

- "Task": the agent's to-do items (`TaskCreate`, `TaskList`), background agents (`TaskOutput`, `TaskStop`), and a `Task` struct in the loop.
- "guard": `crates/agent-engine/src/guard/` is the bypass safety classifier; `agent_runtime/guard.rs` cleans up a worktree.
- "permission": `crates/agent-engine/src/permission.rs` (the pure verdict), `agent/permission.rs` (the flow), `prompt/permissions.rs` (reading and writing rules in settings).
- "Conversations": the message delta recorded for the trace, and the stored sub-agent conversations.
- `prompt/` in the engine holds project configuration (settings, rules, styles), not only prompts.
- `Decision` and `Verdict` each name more than one type across `domain` and the engine.
- "Native" in `NativeEngine` and `native-sessions` means only "the engine built into the app".
