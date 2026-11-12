# Architecture

The app is a Cargo workspace of small crates with one-way
dependencies. Business rules never depend on IO. Vocabulary is in
`GLOSSARY.md`.

```
cli ──► sdk ──► backend ──► agent-engine ──► (provider APIs)
                                       │                    └──► llm-providers
                                       └──► protocol ─► domain   (everything speaks domain)
```

| Crate | Responsibility |
|---|---|
| `cli` | The `{command}` binary: a full-screen chat (ratatui) over the SDK, or with `--inline` one at the bottom whose conversation goes to the terminal's scrollback (`tui::inline`), `-p` print mode, `login`/`logout`, `models`, `mcp`, `config`, `doctor`. Presentation only. |
| `sdk` | The CLI's internal client to the backend, not a public product. `Sdk::builder()`: a `LocalBackend` plus the host's `Extensions`; `Session`/`Run` over the protocol; memory, providers, models, settings. |
| `domain` | Domain model: requests, engine events, ids, `Principal`, memory rules, usage and cost. Pure. |
| `agent-engine` | The `AgentEngine` boundary (`models`, `start`) and the app's own engine behind it: agent loop, tools, permissions, hooks, sub-agents, background tasks, MCP client, sessions, compaction, side calls, safety check; skill and agent discovery in Claude Code's layout; provider APIs behind `ModelApi`. Feature `test-support`: a scripted engine, model, HTTP and sessions for tests. |
| `protocol` | Serialisable `ClientCommand` / `ServerEvent` in an `Envelope` carrying the `Principal`; the `Backend` trait. |
| `backend` | `LocalBackend` (worker threads), `CommandHandler`: runs, memory (+ its MCP tool), settings, model providers (and their browser sign-in: PKCE, loopback redirect), secrets, the host's `Extensions`. |
| `llm-providers` | Model providers, one folder each under `src/providers/<name>/`, found by `build.rs` into a registry. Shared pieces: `Config`, OAuth (`oauth::browser`), request effort, catalog paging, the sign-in `Host`. No network of its own. |
| `subprocess` | Child processes started the same everywhere; `Tree` ends a process with its children. |
| `private-file` | Files only the current user can read. |
| `brand` | The product's name and command, from `brand.toml` (below). |

## Name

The product's name, command, company and author live in
`brand.toml`, read when the `brand` crate is built: `brand::NAME`,
`brand::COMMAND`, `brand::DIR` (`.{command}`), `brand::INSTRUCTIONS`
(`{COMMAND}.md`), `brand::env("X")` (`{COMMAND}_X`), and the same as
literals (`brand::name!()`…) for `concat!` and format strings. Texts kept
in files (prompts, built-in skills) say `{{NAME}}`, `{{DIR}}`… and are
filled with `brand::text`. Files that must spell it out (the binary's name
in `crates/cli/Cargo.toml`, the workspace's authors, the repository's own `.{command}/`) are
written by `cargo run -p xtask -- brand`; `xtask lint` fails when one is
stale or another file names the product. README.md names the product too
but is written by hand.

## No comments in code

Code carries no comments (a CLAUDE.md rule): no `//`, `///`, `//!` or
`/* */` in Rust, no `#` comments in shell, Python or TOML. Names, types
and tests say it; context belongs in docs/ or the commit message. `xtask
lint` fails on a comment, and the
`.claude/hooks/no-comments.sh` hook checks each edited file with `cargo
run -p xtask -- comments <file>`.

## Commands and events

A client sends `Envelope<ClientCommand>` to a `Backend` and polls
`ServerEvent`s, woken by a `Wake` callback. `LocalBackend` runs everything
in process on three worker threads (run control, skill and agent
queries, memory), so a slow scan never delays a cancel. Every command carries a
`Principal` and passes an `Authorizer` (`LocalAuthorizer` accepts only the
local owner); this is the seam for a future remote server.

## Engines

`AgentEngine::start` takes a `PromptRequest` and emits `EngineEvent`s;
exactly one terminal event (`Completed`, `Failed`, `Cancelled`) ends a run.
Recognised problems arrive first as `Diagnosed(EngineProblem)`.

The native engine runs the loop in process behind `ModelApi`, one wire per
protocol. A wire is a folder in `crates/agent-engine/src/model/wires/`
(`messages`, `responses`, `chat`) with a `NAME`, a reasoning `ORIGIN` tag
and `open`; `build.rs` gathers them, and a provider's own `wire/` folder too.

A provider is a folder in `crates/agent-engine/src/providers/`: a
`Provider` impl (trait in `llm-providers`) with its id, name, vendor,
`Access` (key, browser or code), secret slot, default window, which wire a
model uses, base URL, headers, body shape, catalog call and page, token
refresh and its own `sign_in` over the `Host` the backend gives it (HTTP,
loopback listener, browser, clock), plus its README, its tests
(`client_tests.rs`) and, when only it speaks one, its wire. A model family's
prices, short names and reasoning rules are a vendor folder in
`crates/llm-providers/src/vendors/`. `ModelProvider` is only the id;
`agent_engine::providers::find` looks it up. Deleting a folder removes the
provider; nothing outside the folder names one, and tests elsewhere use the
test providers from `llm_providers::testing`.

`ModelProviders` keeps one account per provider (`ConnectProvider` with an
API key or token JSON, `SignInProvider` for a subscription's sign-in). The
listed summaries carry each provider's name and `Access`, so the CLI's menus
come from the server. `ListModels` asks every connected provider's catalog.
A run uses its provider's key (`RunCredentials::model`, never serialized)
and carries the owner's other connected providers' keys
(`RunCredentials::other_models`). A model named `provider:model`, the
catalog's own id form, goes to that provider through `model::routed::Routed`
(rule in `domain::routing`); any other name stays on the run's provider.
Providers declared in JavaScript and the routing hooks are in
`docs/extending.md`.

Built-in tools live in folders under `crates/agent-engine/src/tools/builtin/`
(`files`: Read, Write, Edit, Glob, Grep, NotebookEdit; `shell`: Bash,
BashOutput, KillShell, Monitor and the background-shell registry; `notify`:
PushNotification; `web`: WebFetch, WebSearch over `Workspace::network`;
`schedules`; `advisor`; `verification`, whose `review.rs` exports a
`Review` the agent loop asks before a run completes). Each folder's `tools()`
(and `review::reviewers()` when it has a `review.rs`) is gathered by
`build.rs`; deleting a folder drops them. Tools the agent loop acts on itself stay core and are
listed in `tools::standard()`: AskUserQuestion, Enter/ExitPlanMode (plan is
a `PermissionMode`), Enter/ExitWorktree (session relocation) and
ListSessions (peers, tied to SendMessage routing).

A feature that spans layers is one folder per layer, with the same name:
`crates/agent-engine/src/tools/builtin/<name>/` (its tools),
`crates/backend/src/features/<name>/` (`handle(&Context, input)` and
`forget(client)`, reached by the generic `ClientCommand::Feature { feature,
input }` and answered with `ServerEvent::Feature`; a `run.rs` with `prepare`
changes each run's request and may return a `Finish` called when the run
ends) and `crates/cli/src/tui/features/<name>/` (its `/` commands as `Slash`,
an optional `tick`, a typed `prefix` such as `#`, a status-line marker and a
`/config` entry; state in `App::features`, results back through
`Msg::Feature`). `Context` carries the client, principal, `LocalServices`
(with its `data` folder and the features turned `off`) and a way to start a
run. Each layer's `build.rs` gathers its folders. Removing the folders of one
name removes the feature. Features: `schedules`,
`shell` (`/bashes`), `web`, `advisor`, `memory`, `verification`, `channels`,
plus the backend's `plugins`, `trace`, `workspace` and `declared_providers`.
Inbox notices are core (`TakeNotices`).

- **Built-in agents**: `Explore`, `Plan`, `general-purpose`, `fork`.
  Project or user agents of the same name replace them.
- **SendMessage** (`agent/conversations.rs`): a finished sub-agent's
  report ends with its `agent_id`; `SendMessage` continues it with its
  conversation intact, on its original card. Agents last for the run (one
  prompt); a background one takes messages once collected, a worktree one
  never (its folder is gone), and only the top-level agent sends them.
- **Steering**: `ClientCommand::SteerRun` (`Run::steer`) leaves a message
  on the run's `Control`; the loop hands it to the model with the next
  tool results, or as one more turn when the model was about to finish,
  and emits `EngineEvent::Steered`. The chat steers what is typed while
  the agent works and shows it as queued until `Steered`; a message never
  handed over (the run ended first) is sent as the next prompt.
  `ClientCommand::UnsteerRun` (`Run::unsteer`) takes back what was not
  handed over yet, returned in `EngineEvent::Unsteered`; the chat's ↑ on an
  empty prompt puts it back in the prompt to edit.
- **Permission rules** (`domain::permission_rules`, pure): the
  settings' `permissions.allow/ask/deny` (`Bash`, `Bash(git diff:*)`,
  `Bash(npm run *)`, `Edit(src/**)`, `Read(~/x)`, `Read(//etc/hosts)`,
  `mcp__server[__tool]`). Deny wins over ask, ask over allow; a compound
  command is allowed only when every part is. Matching errs toward
  asking: deny/ask see a command as run (wrappers, assignments, quotes
  and program paths stripped) and paths in any case; allow sees it as
  written and never covers `$(…)`, backticks, `<(…)`, `>`, `bash -c`,
  `eval`, `xargs` or unclosed quotes, which ask instead when a deny or ask
  rule exists. Only the owner's files (`~/.{command}/settings.json`,
  `settings.local.json`) can allow; a checked-in file only restricts.
  `permission::ruled` puts them over the mode: a deny refuses even in
  Bypass; an ask or allow never lifts Bypass or Plan, and a hook's allow
  never skips an ask rule. `Workspace::ruling` names the target: the
  command, the folder a `Glob`/`Grep` searches (a denied folder inside it
  counts, best effort for bare names), or a path judged both as written
  (`..` folded) and through links.
- **Asking**: a permission ask carries the rule "don't ask again" would
  keep (`permission_rules::suggest`: `Bash(cargo test:*)`, or the folder
  of a read outside, `Read(//dir/**)`). `AllowAlways` writes it to the
  project's `.{command}/settings.local.json` (`prompt::permissions::
  keep_allow`) and to the run's remembered rules; without one the call is
  remembered for the session. `Decision::DenyWith(note)` hands the agent
  what to do instead (the chat's "No, and tell the agent…").
- **Added folders**: `PromptRequest::add_dirs` (the SDK's
  `Session::add_dir`; the chat's `/add-dir` and `--add-dir`) and the
  owner's `permissions.additionalDirectories` are read like the project
  (`Workspace::added_dirs`, `may_read`) and named in the prompt's
  environment.
- **Tool calls**: consecutive read-only calls inside the project that no
  rule asks about run in parallel (`agent::call::batches`), the rest in
  order. The messages and responses wires stream a call's input
  (`EngineEvent::ToolInputDelta`). `Bash` waits on a command through the
  background registry (`tools::shells::wait`), so `BackgroundRun`
  (Ctrl+B) leaves it running under its id; stdout and stderr share one
  pipe. `TaskCreate`/`TaskUpdate` emit `EngineEvent::Tasks`. Asking the
  user fires the `Notification` hooks.
- **More tools**: `WebFetch` (through the engine's `Http`, rules as
  `WebFetch(domain:host)`), `WebSearch` (Brave with
  `BRAVE_SEARCH_API_KEY`, else DuckDuckGo's HTML page), `Read` of PDFs
  (`pdftotext`) and notebooks, `NotebookEdit`, `Monitor` (waits for a
  check command), and `ToolSearch`: past 20 MCP tools their schemas wait
  until loaded (`tools::tool_search`). A free query ignores words under
  three letters and ranks a word in a tool's name above one in its
  description; every answer names the tools still waiting, to load by
  `select:`. `Agent` takes `model` and `isolation: worktree` per call; a
  model the run's provider does not serve is ignored (the agent's own or
  the run's is used), and a worktree needs a repository with a commit.
- **Repeated calls** (`domain::repeats`): a third call with the same tool
  and input in one run is not run, unless another tool that may change
  something ran in between; the agent is told its earlier result holds and
  how to load another tool. Tools that wait on something running (`Bash`,
  `BashOutput`, `TaskOutput`, `TaskList`, `TaskGet`, `Monitor`) may repeat. MCP servers initialize once per run;
  Remote servers sign in with OAuth (`/mcp login <name>`, `{command} mcp
  login`): discovery, dynamic registration and PKCE rules in
  `domain::mcp_oauth`, the flow and refresh in the backend's
  `mcp_auth` (tokens in the secrets store by server URL); runs read them
  through `RunCredentials::mcp_tokens`, and a 401 says how to sign in.
  `"type": "sse"` servers use the older HTTP+SSE transport (`mcp/sse.rs`:
  a stream names the POST endpoint, same site only; answers come back on
  the stream).
  those with resources add `ListMcpResources`/`ReadMcpResource`, and a
  prompt typed as `/mcp__server__prompt args` is filled by `prompts/get`.
- **Session worktrees**: `EnterWorktree` makes a worktree under
  `.{command}/worktrees` and `ExitWorktree` (`keep`/`remove`, refused with
  changes unless `discard_changes`) goes back. The loop asks for the move
  (`Control::move_to`), ends the turn with `Ending::Moved`, and the run
  goes on in the new folder with a rebuilt prompt (`engine/moved.rs`),
  emitting `EngineEvent::Moved`. The session store remembers the worktree,
  so later runs resume there.
- **Events from outside the conversation**: each client has an inbox of
  `domain::inbox::Notice`s (`tools::inbox`). `Monitor` streams every
  line a background command prints; every background command says when it
  ends (`shells::watch`); channels (MCP servers declaring
  `experimental.claude/channel`, opened per client with `--channels`,
  `backend::channels`) push `notifications/claude/channel`; another session
  sends with `SendMessage` to its name (`tools::peers`, `ListSessions`, one
  owner's sessions in the same process only). A run reads the inbox with its next tool results
  (`EngineEvent::Notified`); an idle chat takes it with its schedule poll
  and sends it as a turn. A channel is a process, an SSE server or a
  streamable HTTP server (heard on its answers and on a GET stream it
  reopens, `mcp::http_listen`). Runs reach an open channel's tools through
  a bridge to that same server; a cancelled run's call is cancelled there
  (`notifications/cancelled`), the channel staying up. `PushNotification` shows a desktop
  notification.
- **Scheduled prompts**: `CronCreate`/`CronList`/`CronDelete` and
  `ScheduleWakeup` keep prompts per client (`tools::schedules`, like the
  background shells; cron parsing and matching in `domain::schedule`,
  local time; recurring ones expire after 7 days). The chat asks for the due
  ones each second while idle (`ClientCommand::Schedules { take_due }`) and
  sends them, or queues them behind a working run; `/schedules` lists them
  and `/loop [interval] <task>` asks the agent to schedule a task.
- **Runs from scripts**: `PromptRequest` carries `max_turns`,
  `allowed_tools`/`disallowed_tools` (rules for the run), `append_system`,
  `fallback_model` (or the settings' `fallbackModel`, one model or a
  chain: an overloaded model hands the run over, `EngineEvent::FellBack`;
  `provider:model` falls back to another
  connected provider; the `ModelError` hook intercepts any failure first,
  policy in `domain::fallback`) and `fork` (go on from
  `resume` under a new session). `{command} -p` maps them to flags and
  writes `text`, `json` or `stream-json` (`print_output`).
- **Prompt**: planning mode, output styles, tool descriptions carrying the
  working rules; project instructions and rules (`docs/native-engine.md`).
- **Keeping the context small and cached**: a session keeps the date and
  git snapshot of its first prompt (`Sessions::load_snapshot`), so later
  prompts reuse the cached prefix. The mode's reminder and the path rules
  a run meets are not in the system prompt: they ride as
  `Block::Reminder` (sent as `<system-reminder>` text, left out of
  `Message::text`) on the latest user message whenever they change
  (`agent/reminders.rs`). Providers keep one cache key per
  conversation (`model/conversation_key.rs`). Read sends a note for an
  unchanged range already read (`tools/seen_files.rs`) and stops before
  100k characters; long Bash output keeps its start and end. At the
  compaction threshold, judged by the provider's token count
  (`upkeep::Counted`), old tool results are cleared first
  (`upkeep::clear_old_results`) and only then is the conversation
  summarised, with the last turn's tools and system so its cache is read.
  The small model runs `Explore` when it names no model and is WebFetch's
  separate page reader (`tools/builtin/web/guarded.rs`). Each run picks it
  for the session's provider (`engine/small_model.rs`, rule in
  `domain::small_model`): the settings' `smallModel` if the provider offers
  it, else the provider's `small_model()` (its vendor's `small()` unless the
  provider names its own, as Kiro does), else the cheapest model in the
  account's catalog, priced by whichever vendor makes it (Copilot and Kiro
  list several vendors' models, with dotted versions) (`ApiSource::catalog`, cached per provider), else the
  session's model. A small model that fails hands WebFetch's page to the
  session's model.
- **Plan mode** (`agent/plan_mode.rs`): the top-level agent presents its
  plan with `ExitPlanMode` (`AskKind::Plan`); `Decision::Approve(mode)`
  switches the run's mode and the agent implements the plan in the same
  run, `Deny` keeps planning. `EnterPlanMode` turns planning on. Both emit
  `EngineEvent::ModeChanged`, which the SDK's `Session` follows (later
  runs keep the mode) and the chat shows; the presented plan's
  `Verification` section becomes the session's criteria.
- **Compaction** keeps only the nine-section `<summary>`.
- **Side calls** (`chores/`): `Title`, `Suggest`, `Recap`, `Side`,
  `HeadsUp`, `Rename`; one model turn without tools, never added to the
  conversation. The SDK exposes them on `Session` (`title`, `suggest`,
  `recap`, `heads_up`, `side`). The chat names a conversation after its
  first run, offers the next prompt (Tab takes it), shows a heads-up when
  there is one, recaps on `{command} -c`, and answers `/btw`.
- **Checkpoints** (`checkpoints.rs`, `engine/rewind.rs`): each prompt opens
  one (`EngineEvent::Checkpoint(n)`); `Edit`/`Write` keep a file's content
  before its first change in the run. `SessionCommand::Rewind(n)`
  (`Session::rewind`) restores those files and cuts the conversation before
  prompt `n` (only the files once the conversation was compacted). In a git
  repository a foreground `Bash` command is tracked too (`shell/changes.rs`):
  `git stash create` names the tree before it, and the files `git status`
  then shows changed keep their content from that tree, or are deleted on
  rewind if they are new. Ignored files and files untracked before the
  command are left alone. The chat's `/rewind` picks a prompt;
  `/retry` rewinds to the last one and sends it again.
- **Safety check** (`guard/`): in Bypass, every editing or executing action
  is graded by the run's model before it runs.
- **Verification** (`tools/builtin/verification/`; rules in
  `domain::verification`): a top-level run whose
  `PromptRequest.verification` holds criteria and that used a tool is
  graded by a judge agent before it completes (an answer with no tool call
  is not judged). Background shells the judge leaves running are killed
  when it ends. The judge never asks the user: commands the criteria
  name (code spans and code-block lines) and the owner's
  `verification.allow` rules (permission-rule syntax, read only from
  `~/.{command}/settings.json` and `settings.local.json`) run without
  asking and without the safety check (`domain::verification::judge_rules`,
  `Workspace::pre_approved`); a deny or ask rule still wins. Any other
  command goes to the safety check in Bypass and is refused in the other
  modes. The judge has `Read`, `Glob`, `Grep`, `Bash` and
  `Skill` (it runs tests and lint, loads `verify`/`run`), never edits, and
  shows as a `verification` sub-agent. It ends with
  `EngineEvent::Verified(Verdict)`; a failed verdict sends the agent back
  with the reasons, up to 3 attempts. A judge that cannot grade fails the
  check, never the run. The model is the run's or `judgeModel`. The SDK's
  `Session` keeps the criteria (`set_verification`, else the last plan's
  `Verification` section) and sends them with every run outside Plan mode;
  the chat sets them with `/verification`, `-p` with `--verification`.
- **Built-in skills** (`crates/agent-engine/skills/`, embedded by
  `build.rs`).

## Memory

Agents remember a project between sessions. Memories
(`domain::memory`) are episodic, semantic or procedural, with tags
and author. `MemoryStore` (`FileMemoryStore`, one JSON per project). A
fresh session gets recalled memories plus instructions to write
`{remember, forget, revise}` to a temp file, applied on `Completed`.
Search is by words (BM25). The same function serves recall and the
`memory_search` MCP tool (in process: each run gets a `{command}_memory`
server on its credentials, bound to its owner and project). All of it is the
`memory` feature: `backend/src/features/memory` (store under the services'
`data` folder, kept in the process when there is none; `run.rs` briefs a run
and applies its edits when it ends; `memory_search` in `off` drops the tool)
and `cli/src/tui/features/memory` (`/memory` and `# note`).

Memory is reversible and kept without an LLM:
- **Edits**: every change goes through `Memories::edit` (or the
  optimizer's apply). Text passes `memory::sanitize`. With an embedder, a
  remembered memory within cosine 0.92 of an older one of its kind
  revises it (`MemoryBook::merge_near_duplicates`); without vectors
  nothing merges.
- **Forgetting and revising**: `Forget` moves a memory to
  `MemoryBook::forgotten` (`Restore` for 30 days). `Revise` keeps the last
  five texts in `history`.
- **Recall**: `recall` weighs episodes by `retention`, so they halve every
  30 days unless they are used. `brief` and `memory_search` mark what they
  hand out as used.
- **Optimizer**: `ClientCommand::OptimizeMemory` is one run over
  `optimize_prompt`, with no recall and no search tool. `UndoMemory` puts
  the book back until the next edit. The CLI's `/memory
  optimize|undo|forgotten|restore <id>`.
- **Store**: an unreadable book is renamed `.json.corrupt-<secs>`, never
  overwritten. Saves `fsync` before the rename. Without an app data folder
  memory lives only in the process.

## Language servers

The agent navigates code through language servers. What they
mean is pure in `domain::lsp`: the built-in servers and the
`lspServers` layers (user, project, plugins' `.lsp.json`), the server and
project root for a file, the `LSP` tool's questions and how answers and
diagnostics read. `agent_engine::lsp` runs them: `LspClient` (one
process, `Content-Length` framing, a reader thread routing answers and
published diagnostics) and `LspPool` (one server per name and root, kept
by the `NativeEngine` across runs, ended after 10 idle minutes), behind
`LanguageServers` so tests script them (`lsp::fake`). A run gets
`Workspace::lsp` and the `LSP` tool only when one of its servers is
installed; `Edit` and `Write` append the file's errors and warnings as
`<diagnostics>`. `tests/real_lsp.rs` (ignored) runs against clangd.

## Skills, agents and MCP

The engine reads only the app's folders: skills, commands,
agents, rules and settings in the project's `.{command}/` (and its
ancestors') and `~/.{command}/`, plus the backend host's folders
(`Extensions::dirs`). Nothing in `.claude/`, `~/.claude` or `CLAUDE.md`.
MCP servers come from the host: HTTP/stdio servers go in a run's private
MCP config (`0600`, removed on the terminal event), in-process ones ride on
the run's credentials.

**Plugins** are the app's own format
(`.{command}-plugin/plugin.json`) and reach a run
whole (`agent_engine::plugins`): skills, commands and agents as
`<plugin>:<name>`, `hooks/hooks.json` joined to the settings' hooks, and
`.mcp.json` servers as `plugin_<plugin>_<server>`, with
`${{COMMAND}_PLUGIN_ROOT}` filled in. They come from the backend
(`Extensions::plugins`, never from a client) and from the folders in
`~/.{command}/plugins/`; a plugin whose
hooks or servers do not parse is left out, the run goes on. Every run
rereads them; `/reload` refreshes what the chat keeps and lists them.

## The SDK and the CLI

The `sdk` crate is the CLI's internal client to the backend, not a
public product. It speaks only `ClientCommand`/`ServerEvent` (one
dispatcher thread routes run events to their `Run`, other answers to the
waiting call), so it can move to a remote backend unchanged. What a host adds reaches every
run through the backend's `Extensions`: its agents, skills and commands are
written to `<data>/sdk/<name>/` and offered under their own names
(`Extensions::dirs`); HTTP/stdio MCP servers join the run's private
MCP config; in-process servers (`McpServer`, closures behind
`domain::mcp::McpEndpoint`) ride on `RunCredentials::mcp` and are
called through `LocalTransport`, never over the wire. A typed
`/name args` is expanded by the engine when it names a skill or command.

Each terminal runs its own backend in process: no daemon, nothing shared
between terminals but the files on disk. `SendMessage` and
`ListSessions` between sessions therefore reach only sessions in the
same process.

The CLI is a client of the SDK: `App` (state and effects, slow calls on
threads), `Transcript` (events folded into what is shown), `Input`, `ask`,
`view` (drawing), `markdown` (`tui-markdown`, renders kept per text and
width), `diffs` (edits and `git diff`, folded until Ctrl+O), `attach`
(images for the prompt) and `motion`
(`tachyonfx` effects over the drawn frame: colour only, never
characters). Its own `/commands` parse in
`domain::command`; model, mode and effort persist in the owner's
settings. `/diff` runs `git` in the client's folder, the one place the CLI
reads the project itself. Attached images travel in
`PromptRequest::images` and open the first user message.

## Secrets

Provider keys and subscription tokens live in one
sealed file (`backend::system_secrets`): `secrets.json`, each
entry encrypted with XChaCha20-Poly1305 under a key in `secrets.key`
(both `0600`), re-read on every access so two processes never drop each
other's entries. The system keychain is not used: macOS would ask to
allow each new unsigned build of the binary.

## Sessions

The engine keeps each session's transcript (`FileSessions`, one JSONL file
per owner, folder and session) so `resume` continues it after a restart.
The file only grows: messages, `Truncated` (a rewind), `Restarted` (a
compaction or any other rewrite of the history), the run's prompt
(`Prompted`) and every engine event with its time (`Event`; the run gate's
observer, `engine::record`, joins streamed text and drops partial tool
input). It also keeps each model call (`Called`: model, stop reason,
effort, and the system prompt and tool definitions the first time each
version appears, by fingerprint), how the run was set up (`Opened`: model,
permission mode, folder, git snapshot) and a rewind's restored files. A
sub-agent's calls and token usage are tagged with it: `agent::current`
knows, per thread, which sub-agent is running, so only the log, not the
events sent to the client, carries the tag. Resume reads back from the last
`Restarted`; nothing before it is deleted. Older JSON-array files load as
before and are folded into the log on the next write.

`/trace [session]` in the chat, or `{command} trace [session] [--no-open]`
(the `trace` feature; the CLI's `build.rs` registers a feature folder's
`cli.rs` as its command), reads the whole file (`sessions::records`; a
session saved before traces is replayed from its messages and marked
partial), builds a tree with `domain::trace::build` (prompts with their
run's setup, model turns with what the model received, tools called side by
side with their permission asks, sub-agents' lanes under the tool that
started them, background ones too, compactions, rewinds, fallbacks, errors)
and writes a self-contained HTML page to `~/.{command}/traces/<session>.html`,
opened in the browser.
