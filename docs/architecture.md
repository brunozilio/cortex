# Architecture

The app is a Cargo workspace of small crates with one-way
dependencies. Business rules never depend on IO. Decisions are in
`docs/decisions.md` (cited as `ADR NNNN`), vocabulary in `GLOSSARY.md`.

```
cli ──► sdk ──► backend ──► agent-engine ──► (provider APIs)
                                       │                    └──► llm-providers
                                       ├──► mlx ─► (mlx_lm.server)
                                       └──► protocol ─► domain   (everything speaks domain)
```

| Crate | Responsibility |
|---|---|
| `cli` | The `<command>` binary: a full-screen chat (ratatui) over the SDK, or with `--inline` one at the bottom whose conversation goes to the terminal's scrollback (`tui::inline`), `-p` print mode, `login`, `models`, `pull`/`rm`, `update` (the latest GitHub release, rules in `domain::release`, ADR 0024). Presentation only. |
| `sdk` | `Sdk::builder()`: a `LocalBackend` plus the host's `Extensions`; `Session`/`Run` over the protocol; memory, providers, models, settings; `LocalModels` (pull/remove local MLX models). |
| `domain` | Domain model: requests, engine events, ids, `Principal`, memory rules, usage and cost. Pure. |
| `agent-engine` | The `AgentEngine` boundary (`models`, `start`) and the app's own engine behind it (ADR 0015): agent loop, tools, permissions, hooks, sub-agents, background tasks, MCP client, sessions, compaction, side calls, safety check; skill and agent discovery in Claude Code's layout; provider APIs behind `ModelApi`. Feature `test-support`: a scripted engine, model, HTTP and sessions for tests. |
| `mlx` | Local MLX models on Apple silicon: pulled from Hugging Face (`pull`/`remove` behind a `Hub`), which are downloaded, and one `mlx_lm.server` started on demand. |
| `protocol` | Serialisable `ClientCommand` / `ServerEvent` in an `Envelope` carrying the `Principal`; the `Backend` trait. |
| `backend` | `LocalBackend` (worker threads), `CommandHandler` and the daemon: runs, memory (+ its MCP tool), settings, model providers (and their browser sign-in: PKCE, loopback redirect), secrets, the host's `Extensions`. |
| `llm-providers` | LLM provider contracts (ADR 0016): credential config, OAuth shapes, request and catalog shapes. Pure. |
| `subprocess` | Child processes started the same everywhere; `Tree` ends a process with its children. |
| `private-file` | Files only the current user can read. |

## Name

The product's name, command, owner and release repository live in
`brand.toml`, read when the `brand` crate is built: `brand::NAME`,
`brand::COMMAND`, `brand::DIR` (`.<command>`), `brand::INSTRUCTIONS`
(`<COMMAND>.md`), `brand::env("X")` (`<COMMAND>_X`), and the same as
literals (`brand::name!()`…) for `concat!` and format strings. Texts kept
in files (prompts, built-in skills) say `{{NAME}}`, `{{DIR}}`… and are
filled with `brand::text`. Files that must spell it out (the binary's name
in `crates/cli/Cargo.toml`, the workspace's author, install.sh's defaults,
README.md from `docs/README.template.md`, release-please's package name,
the repository's own `.<command>/`) are written by `cargo run -p xtask --
brand`; `xtask lint` fails when one is stale or another file names the
product. The workflows read the command from `brand.toml`.

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

The native engine (ADR 0015) runs the loop in process behind `ModelApi`
(`anthropic::Messages`, `openai::Responses`, `kiro::Conversation`,
`chat::Chat` for local servers). `ModelProviders` keeps one account per
provider (`ConnectProvider` with an API key or token JSON, `SignInProvider`
for a subscription's browser sign-in). `ListModels` asks every connected
provider's catalog and lists downloaded MLX models. A run uses its
provider's key (`RunCredentials::model`, never serialized).

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
  rule exists. Only the owner's files (`~/.<command>/settings.json`,
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
  project's `.<command>/settings.local.json` (`prompt::permissions::
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
  order. Anthropic and OpenAI stream a call's input
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
  until loaded (`tools::tool_search`). `Agent` takes `model` and
  `isolation: worktree` per call. MCP servers initialize once per run;
  Remote servers sign in with OAuth (`/mcp login <name>`, `<command> mcp
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
  `.<command>/worktrees` and `ExitWorktree` (`keep`/`remove`, refused with
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
  owner's sessions only). A run reads the inbox with its next tool results
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
  `fallback_model` (or the settings' `fallbackModel`: an overloaded model
  hands the run over, `EngineEvent::FellBack`) and `fork` (go on from
  `resume` under a new session). `<command> -p` maps them to flags and
  writes `text`, `json` or `stream-json` (`print_output`).
- **Prompt**: planning mode, output styles, tool descriptions carrying the
  working rules; project instructions and rules (`docs/native-engine.md`).
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
  there is one, recaps on `<command> -c`, and answers `/btw`.
- **Checkpoints** (`checkpoints.rs`, `engine/rewind.rs`): each prompt opens
  one (`EngineEvent::Checkpoint(n)`); `Edit`/`Write` keep a file's content
  before its first change in the run. `SessionCommand::Rewind(n)`
  (`Session::rewind`) restores those files and cuts the conversation before
  prompt `n` (only the files once the conversation was compacted). Changes
  made with `Bash` are not tracked. The chat's `/rewind` picks a prompt;
  `/retry` rewinds to the last one and sends it again.
- **Safety check** (`guard/`): in Bypass, every editing or executing action
  is graded by the run's model before it runs.
- **Verification** (`judge.rs`, `agent/verify.rs`; rules in
  `domain::verification`, ADR 0020): a top-level run whose
  `PromptRequest.verification` holds criteria is graded by a judge agent
  before it completes. The judge has `Read`, `Glob`, `Grep`, `Bash` and
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

Local models (`mlx`) live in
`<local data>/models/mlx/<org>/<name>/` on Apple silicon; one
`mlx_lm.server` runs at a time and stops when idle. `<command> pull` fetches
only the files `mlx_lm` reads (config, tokenizer, chat template,
safetensors) from Hugging Face (`HF_TOKEN` for gated repos) into a hidden
`.<name>.partial` folder renamed into place when complete, so a cancelled
pull is never listed; `<command> rm` deletes one. They run in the CLI's own
process through the SDK's `LocalModels` (the backend re-exports
`mlx` as `local_models`), not as backend commands: they touch only
this computer's folder, which `ListModels` reads on every call.

## Memory

Agents remember a project between sessions. Memories
(`domain::memory`) are episodic, semantic or procedural, with tags
and author. `MemoryStore` (`FileMemoryStore`, one JSON per project). A
fresh session gets recalled memories plus instructions to write
`{remember, forget, revise}` to a temp file, applied on `Completed`.
Search is BM25 plus embedding similarity (fastembed/ONNX,
`<COMMAND>_MEMORY_MODEL=minilm|mpnet|off`), fused by reciprocal rank; the
same function serves recall and the `memory_search` MCP tool (in process:
each run gets a `<command>_memory` server on its credentials, bound to its
owner and project).

Memory is reversible and kept without an LLM (ADR 0022):
- **Edits**: every change goes through `Memories::edit` (or the
  optimizer's apply). Text passes `memory::sanitize`. A remembered memory
  within cosine 0.92 of an older one of its kind revises it
  (`MemoryBook::merge_near_duplicates`).
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

The agent navigates code through language servers (ADR 0023). What they
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

The engine reads only the app's folders (ADR 0021): skills, commands,
agents, rules and settings in the project's `.<command>/` (and its
ancestors') and `~/.<command>/`, plus the SDK host's folders
(`Extensions::dirs`). Nothing in `.claude/`, `~/.claude` or `CLAUDE.md`.
MCP servers come from the host: HTTP/stdio servers go in a run's private
MCP config (`0600`, removed on the terminal event), in-process ones ride on
the run's credentials.

**Plugins** (ADR 0019) are the app's own format
(`.<command>-plugin/plugin.json`) and reach a run
whole (`agent_engine::plugins`): skills, commands and agents as
`<plugin>:<name>`, `hooks/hooks.json` joined to the settings' hooks, and
`.mcp.json` servers as `plugin_<plugin>_<server>`, with
`${<COMMAND>_PLUGIN_ROOT}` filled in. They come from the backend
(`Extensions::plugins`, never from a client) and from the folders in
`~/.<command>/plugins/`; a plugin whose
hooks or servers do not parse is left out, the run goes on. Every run
rereads them; `/reload` refreshes what the chat keeps and lists them.

## The SDK and the CLI

The SDK speaks only `ClientCommand`/`ServerEvent` (one dispatcher thread
routes run events to their `Run`, other answers to the waiting call), so it
can move to a remote backend unchanged. What a program adds reaches every
run through the backend's `Extensions`: its agents, skills and commands are
written to `<data>/sdk/<name>/` and offered under their own names
(`Extensions::dirs`); HTTP/stdio MCP servers join the run's private
MCP config; in-process servers (`McpServer`, closures behind
`domain::mcp::McpEndpoint`) ride on `RunCredentials::mcp` and are
called through `LocalTransport`, never over the wire. A typed
`/name args` is expanded by the engine when it names a skill or command.

### The daemon

On Unix every `<command>` talks to one background daemon per computer
(`backend::daemon`), started on first use (`<command> daemon
serve`, detached) and gone 5 s after its last client leaves (so commands
run one after another reuse it). It loads the services
once (MLX servers, the embedding model, memory books) and gives each
client connection its own `LocalBackend` on clones of them, so a client
gets only its own runs' events and answers, and its runs end when it
disconnects. The wire is newline-delimited JSON over `daemon.sock`
(`0600`, in a `0700` folder): `Envelope<ClientCommand>` in, `ServerEvent`
out. A lock file keeps one daemon per folder; `daemon.json` holds its pid
and build so a newer `<command>` can say the daemon is stale (`<command>
daemon stop`). `<COMMAND>_NO_DAEMON=1` (and Windows) run the backend in
process. SDK programs embed their own backend by default (their
in-process tools cannot cross processes); `Sdk::connect_daemon`
joins the daemon instead.

The CLI is a client of the SDK: `App` (state and effects, slow calls on
threads), `Transcript` (events folded into what is shown), `Input`, `ask`,
`view` (drawing), `markdown` (`tui-markdown`, renders kept per text and
width), `diffs` (edits and `git diff`, folded until Ctrl+O), `attach`
(images for the prompt), `logo` (`tui-big-text`) and `motion`
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

The engine keeps each session's transcript (`FileSessions`, one JSON per
owner, folder and session) so `resume` continues it after a restart.
