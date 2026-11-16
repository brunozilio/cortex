# Architecture

The app is a Cargo workspace of small crates with one-way
dependencies. Business rules never depend on IO. Vocabulary is in
`GLOSSARY.md`.

```
cli ──► sdk ──► backend ──► agent-engine ──► llm-providers ──► domain
         │        │              │
         └────────┴──► protocol ─┴─────────────────────────────► domain

leaf crates: brand (everyone), code-map (backend), content (cli, agent-engine), language-servers (agent-engine),
             platform (cli, backend, agent-engine), script (agent-engine)
```

`domain` is the only crate every layer shares; nothing below `sdk` knows
the CLI, and `cli` reaches `backend` and `agent-engine` only in tests.

| Crate | Responsibility |
|---|---|
| `cli` | The `{command}` binary: a full-screen chat (ratatui) over the SDK, or with `--inline` one at the bottom whose conversation goes to the terminal's scrollback (`tui::inline`), `-p` print mode, `login`/`logout`, `models`, `mcp`, `config`, `doctor`. Presentation only. |
| `sdk` | The CLI's internal client to the backend, not a public product. `Sdk::builder()`: a `LocalBackend` plus the host's `Extensions`; `Session`/`Run` over the protocol; memory, providers, models, settings. |
| `domain` | Domain model: requests, engine events, ids, `Principal`, memory rules, usage and cost. Pure. |
| `agent-engine` | The `AgentEngine` boundary (`models`, `start`) and the app's own engine behind it: agent loop, tools, permissions, hooks, sub-agents, background tasks, MCP client, sessions, compaction, side calls, safety check; skill and agent discovery in Claude Code's layout; provider APIs behind `ModelApi`. Feature `test-support`: a scripted engine, model, HTTP and sessions for tests. |
| `protocol` | Serialisable `ClientCommand` / `ServerEvent` in an `Envelope` carrying the `Principal`; the `Backend` trait. |
| `backend` | `LocalBackend` (worker threads), `CommandHandler`: runs, memory (+ its MCP tool), settings, model providers (and their browser sign-in: PKCE, loopback redirect whose page answers after the token exchange and names the account when the answer carries an email), secrets, the host's `Extensions`. |
| `llm-providers` | Model providers, one folder each under `src/providers/<name>/`, found by `build.rs` into a registry. Shared pieces: `Config`, OAuth (`oauth::browser`), request effort, catalog paging, the sign-in `Host`. No network of its own. |
| `language-servers` | Language servers: `LspClient` (one process), `LspPool` (kept per name and root), first-use installs; downloads through a `Fetch` the engine gives it (`agent_engine::NetFetch`). Re-exported as `agent_engine::lsp`. |
| `platform` | What differs per operating system: child processes started the same everywhere (`command`), `Tree` ending a process with its children, `bounded` waits, `private_file` (files only the current user can read), the data folders. |
| `content` | Prompts, agents, styles, tools and hooks as JavaScript modules, plus built-in skills, from `content/`, embedded at build time; a user's `~/.{command}/` copy overrides one. `text`/`render` run a module's `default` export (a string or a function of `ctx`), `value` reads a static export such as `spec`; a run that called nothing back is cached by source and data. A run's start copies the whole folder into `~/.{command}/` once per process (`install_once`), with `types.d.ts` for editors; `.content.json` keeps each copy's fingerprint, so an untouched copy follows the built-in when it changes or goes, and an edited one is the user's and is kept. A copy of a built-in tool in `~/.{command}/tools/` is that tool's override, never a script tool. |
| `script` | The QuickJS runtime that runs JavaScript tools, skills and declared providers against a `Host`. |
| `brand` | The product's name and command, from `brand.toml` (below). |
| `code-map` | Experimental tree-sitter index of a repository (Rust, TypeScript/TSX, JavaScript, Python, Go): its functions and types and who calls whom (`extract`), ranked against a prompt into a short `<code-map>` block of pointers (`map`, pure), refreshed incrementally from a JSON cache keyed by path, size and mtime (`scan`, IO behind `Source`). |

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
but is written by hand, and so do the plugins kept in `plugins/`
(`docs/plugins.md`): a plugin's manifest folder and
`${{COMMAND}_PLUGIN_ROOT}` are spelled out, as in any installed plugin, so
a renamed product renames them by hand.

## No comments in code

Code carries no comments (a CLAUDE.md rule): no `//`, `///`, `//!` or
`/* */` in Rust, no `#` comments in shell, Python or TOML. JavaScript in
`content/` carries only JSDoc type annotations pointing at
`content/types.d.ts` (each hook's input and result per event, the hook's
and the tool's `ctx`, `ToolSpec`), so an editor completes `input` and
`ctx`; `tsc --checkJs --strict` passes on them. Names, types
and tests say it; context belongs in docs/ or the commit message. `xtask
lint` fails on a comment, and the
`.claude/hooks/no-comments.sh` hook checks each edited file with `cargo
run -p xtask -- comments <file>`.

## Commands and events

A client sends `Envelope<ClientCommand>` to a `Backend` and polls
`ServerEvent`s, woken by a `Wake` callback. `LocalBackend` runs everything
in process on two worker threads (run control; skill, agent and feature
queries), so a slow scan never delays a cancel. Every command carries a
`Principal` and passes an `Authorizer` (`LocalAuthorizer` accepts only the
local owner); this is the seam for a future remote server. An
`Envelope` also names its `protocol::VERSION`; the handler refuses another
(or none) with `ServerEvent::Error` before authorizing it.

### Seams for a multi-user server

Everything runs where the backend runs: the engine, its tools, hooks, MCP
and model calls. A server hosts the backend and swaps these, leaving the
engine and the CLI as they are:

- **Each person's folder** (`backend::Homes`): skills, agents, commands,
  hooks, settings, instructions, plugins, MCP config, agent memory and
  declared providers are read from `Homes::home(principal)`, which the
  handler puts in the run's `Extensions::home`. The engine never reads the
  machine's home (`brief::home`). `OwnerHome::system()` gives the local
  owner `~` and a member nothing.
- **Built-in content** (prompts, styles, tool texts, built-in skills) is the
  installation's, not a person's: `content::use_folder` points it at the
  organization's copy once per process, `~/.{command}` otherwise.
- **Declared providers** register in one process-wide table, so on a server
  they belong to the installation, not to each person.
- **Model credentials** (`backend::SharedCredentials`): a person's own
  account wins, else the holder the trait names (an organization's service
  account) lends its key; tokens never leave the backend. `NothingShared`
  locally.
- **Where commands run** (`agent_engine::tools::Launcher`, set with
  `NativeEngine::with_launcher`): Bash, background shells, hooks and test
  runs start through it; `OnThisMachine` applies the local sandbox, a
  server can wrap each command into a person's container. File tools read
  and write the workspace folder directly, so a container mounts it.
  MCP stdio servers and language servers do not go through it yet.

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
a `PermissionMode`), Enter/ExitWorktree (session relocation),
ListSessions (peers, tied to SendMessage routing) and Watch (calls another
of the run's tools).

Every built-in tool is declared in `content/tools/<Name>.js`, in the same
shape as a script tool: `spec` (`name`, `access`: read, edit or execute,
`input` schema, `description`; `input` and `description` may be functions
of the data the Rust tool passes) and `execute(input, ctx)`. The Rust
tool's `spec()` and `access()` read it (`tools::declared`,
`declared_access`); the Rust side keeps only the behaviour. Each call runs
`execute` (`Agent::execute_declared`), and `ctx.builtin(input)` enters the
native path, with its permission check, so a user's copy in
`~/.{command}/tools/` can change the input, the result or call other tools
around it. When `execute` returns the built-in's text unchanged, the
built-in's whole outcome (images, shown diff, error flag) is kept. A script
tool exports `execute` too (`run` is still read).

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
- **How a sub-agent starts and ends** (`agent/sub_agent.rs`,
  `agent/sub_agent_report.rs`): one started while planning is offered only
  the tools planning allows (`permission::refused_while_planning`), so Bash,
  the web tools and the editing tools are left out instead of refused call
  by call. A model the provider does not serve is ignored and the report
  says which model ran. A cancelled one ends as `AgentStatus::Cancelled`,
  not `Failed`, and its result carries the answer it was still writing.
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
- **Secret redaction** (`redact-secrets`, a security flag): every tool
  result is passed through `domain::redact` right after
  `untrusted::neutralize` (`agent/dispatch.rs`), so the model, the
  client, a persisted long result and the session log never get the
  secret; the shown diff and the `ToolStarted` summary and input are
  redacted too. It hides known token shapes (`ghp_`, `sk-`, AWS keys,
  JWTs, `Bearer x`, `password=x`) and every value of eight characters
  or more from the project's `.env` and `.env.*` files (not `.example`
  or `.sample`), read once per run into `Policy::redact`; a sub-agent
  keeps its parent's. Unlike `memory::sanitize`, which uses the same
  word rules, it keeps the text's layout. What the model itself writes
  in a tool call is not changed.
- **Budget** (`budget`; rule in `domain::budget`): the chat's `/budget
  <dollars>|off` (the SDK's `Session::set_budget`) caps what a session
  spends. The SDK `Session` adds up every `Usage` event of its runs,
  sub-agents' and side calls' included (`Spending`), and sends the limit
  and what was spent before with each run as `PromptRequest::budget`.
  `Metered::turn` refuses the next model call once what was spent before
  plus the run's priced calls reach the limit, so the run fails saying
  how to raise it; calls without a price count as nothing. The status
  line shows spent against the limit.
- **Sessions in one folder** (`experimental-session-claims`; rule in
  `domain::claims`): every `Edit` or `Write` records which session (and
  process) changed which file in a registry shared by all the terminals
  open on that folder (`tools/workspace/claims.rs`: a JSON file in the
  temporary folder named after the folder, under
  `platform::file_lock`). When another live session changed the same
  file in the last ten minutes, the result says so in `<other-session>`
  and asks the agent to re-read before editing again. Entries of ended
  processes (`platform::alive`) and those older than an hour are
  dropped.
- **Following a PR** (`experimental-pr-follow`): the built-in
  `follow-pr` skill (`content/skills/follow-pr/`) has the agent start one
  `Monitor` over a `gh pr view` loop that prints a line only when the
  PR's state, failing checks, reviews or comments change, so each change
  wakes it to fix CI or answer a review, until the PR is merged or
  closed. A skill behind a flag is left out of the run's skills while
  the flag is off (`domain::features::skill_flag`, `Brief::load`).
- **Affected tests** (`experimental-affected-tests`; rule in
  `domain::affected`): after an `Edit` or `Write` inside the project,
  `shell/affected.rs` starts the tests that cover the file as a
  background command (confined like `Bash`) named by `TestRun::label`
  (the command and its folder, so `go test .` in two packages stays two
  runs), and the result says so in `<tests>`. A run of the same label
  still going is stopped first, and the result of one that already ended
  is withdrawn from the inbox if the agent has not taken it yet
  (`inbox::withdraw`): either was started before this change, so its
  result would be stale. The command's end reaches the agent through
  the inbox. A Rust file runs `cargo test -p`
  of its crate with `CARGO_TARGET_DIR` at `target/agent-tests`, so it
  never waits on the user's own build; Go tests its package; Python and
  JavaScript run the test file beside the source (`test_<name>.py`,
  `<name>.test.ts`), JavaScript only with a `package.json`.
- **Shorter command output** (rules in `domain::output`, applied by
  `shell/shaped.rs` to a foreground `Bash` command that exited; each step
  has its own flag, read into `Policy::output` as a `Shaping`):
  `experimental-output-cleanup` runs the command with `PLAIN_ENV`
  (`NO_COLOR`, `CARGO_TERM_COLOR=never`…) and `clean`s what it printed:
  escape codes go, a line redrawn with `\r` keeps its last state, three or
  more equal lines become one with `(×N)`, blank runs one blank line.
  `experimental-output-filters` hides noise lines (passing tests, build
  steps, progress) of cargo, git, npm/pnpm/yarn/bun/jest/vitest,
  pytest/pip and go test, chosen by each part of the command
  (`filter`); a piped command is left alone, a filter only drops lines
  it knows, and one that saves under 400 characters gives way. Under
  cargo, a warning that comes three times or more (same heading, whatever
  it names in backticks) is shown once, followed by the places of the
  others (`filters/warnings.rs`); errors always stay whole. The
  whole (cleaned, redacted) output is kept in the session's
  `persisted::results_dir` and the footer names the file, so the agent
  can Read what was hidden; exit codes are untouched.
  `experimental-output-savings` records each command's characters before
  and after (`shell::savings`, per client, in memory) and the chat's
  `/gain` (`output_savings` folders in the backend and CLI) shows them per
  program (`report`). Output over 30,000 characters is still saved whole
  and previewed by `agent/dispatch.rs`, with or without these flags.
- **Automatic review** (`auto-review`; folder
  `tools/builtin/code_review/`, rules in `domain::review`): a reviewer
  next to the judge. When a top-level run changed files inside its
  folder (`Edit`, `Write`, `NotebookEdit`; a memory file in the
  temporary folder does not count) and is about to finish, a read-only child
  agent (`Read`, `Glob`, `Grep`, the run's system prompt with its
  instructions, plus `content/prompts/reviewer.js`) reviews the changed
  files for defects and for docs or instructions the change made untrue,
  and answers JSON findings. `EngineEvent::Reviewed` carries them to the
  chat; a `high` finding sends the agent back once with a message that
  starts `# Code review`, and a run that has one is not reviewed again.
  A reviewer that gives no findings lets the run finish.
- **Project criteria** (`project-criteria`): a project may check in
  `.{command}/verification.md`; the nearest one above the folder (never
  the owner's own) becomes `Settings::project_criteria`
  (`prompt/criteria_file.rs`, 64 KiB at most), and a run outside Plan
  mode with no criteria of its own is judged against it
  (`CriteriaSource::Project`; the user's and the plan's win). Because
  the file is not the owner's, the commands it names get no
  pre-approval: the judge runs only what the owner's
  `verification.allow` rules allow (`Criteria::judge_allow`).
- **Escalation when stuck** (`experimental-escalation`; rule in
  `domain::stuck`): the loop counts, per run, the same call (tool and
  input) failing and calls `domain::repeats` holds back
  (`agent/stuck.rs`). Three failures of one call, or two held back,
  count as stuck, at most twice a run. Then the last result tells the
  agent to call the Advisor when the run has one (`advisorModel`), or
  else the rest of the run goes to the owner's `escalationModel` (only
  the owner's files set it), emitting `FellBack`, with a note saying
  why.
- **Cache warning** (`cache-warning`; rule in `domain::cache_window`): the
  chat keeps when the last model call was made and how many tokens it
  read from or wrote to the prompt cache; while idle, with 10k or more
  cached tokens, the status line counts down the last two of the cache's
  five minutes and then says the next turn rewrites it. The chat learns
  which flags are off once at start, through the `flags` feature
  (`App::turned_off`, `features::flags::on`), so a flag also turns off
  what only the chat shows.
- **Suggested rules** (`suggested-rules`; rule in
  `domain::suggested_rules`): the chat's `/rules` reads the `Asked` and
  `Answered` events of the project's recent sessions (the owner's
  `conversations` list) and offers a rule asked about and allowed five
  times with no denial as `allow`, or denied three times with no approval
  as `deny`, unless a rule with that text is already kept; `/rules keep
  <n>` writes it to the project's `settings.local.json`
  (`prompt::permissions::keep_rule`). A flag named with hyphens turns off
  the feature folder named with underscores.
- **Dependency guard** (`experimental-dependency-guard`, a security
  flag): before a `Bash` command that installs packages (`npm`/`pnpm`/
  `bun`/`yarn add`, `cargo add`, `pip`/`uv`/`poetry` installs; rule in
  `domain::dependencies`), `agent/permission.rs` looks each package up in
  its registry through `Workspace.services.network`
  (`guard::packages`). A package the registry does not know, one first
  published under 30 days ago, or one a letter away from a popular name
  makes the call an ask that says why, in every mode, Bypass included,
  before any rule or hook could allow it; a deny rule still wins, and
  dontAsk refuses it. A registry that does not answer leaves the command
  to the usual checks.
- **What the loop decided**: besides the tool events, the loop records
  what it decided so traces and later features need not read it out of
  free text: `Answered { ask, decision }` once the user answers a
  permission ask, `Refused { tool, reason }` whenever a call is refused
  (a rule, the mode, a hook, the safety check, dontAsk or the user),
  `Repeated { tool }` when `domain::repeats` holds back a call, and
  `Edited` (`domain::changes::Edited`: path, first line, lines removed
  and added) after an `Edit` or `Write`, carried out of the tool on
  `Outcome::editing`.
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
  `WebFetch(domain:host)`), `WebSearch` (the provider's own search, offered only where
  `Provider::web_search` names one: Anthropic and Claude get the
  `web_search` server tool, OpenAI, ChatGPT and Grok the Responses
  `web_search` tool; `llm_providers::web_search::put` swaps it in for the
  function tool. Its blocks come back as `Block::Native`, sent back unchanged
  only to the wire that made them, and show as a `WebSearch` call; a
  `pause_turn` stop goes on with no new prompt; each search is counted in
  `Usage::searches` and priced by `Vendor::search_price` into the run's cost), `Read` of PDFs
  (`pdftotext`) and notebooks, `NotebookEdit`, `Monitor` (waits for a
  check command), and `ToolSearch`: past 20 MCP tools their schemas wait
  until loaded (`tools::tool_search`); loaded ones join the end of the
  tool list in the order they were loaded, so the tools already sent stay
  a cached prefix. A free query ignores words under
  three letters and ranks a word in a tool's name above one in its
  description; every answer names the tools still waiting, to load by
  `select:`. `Agent` takes `model` and `isolation: worktree` per call; a
  model the run's provider does not serve is ignored (the agent's own or
  the run's is used), and outside a repository with a commit the agent
  runs without the worktree and its result says so. The same holds for a
  main agent whose definition asks for a worktree: `agent_runtime::prepare`
  makes one only when the folder has a commit, and otherwise the agent
  works in the folder.
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
- **Watches** (`Watch`, `tools::watch`, rules in `domain::watch`): the
  app calls a tool on a fixed pace (60 s to a day, default 5 min) and
  compares one value of its result (a dot `path` into its JSON; `equals`,
  `contains`, or any change), without model turns; one inbox notice
  (`watch watch_N`) says when it is met or why it gave up (three failed
  checks in a row, or 7 days). The loop runs the first check as an
  ordinary call (`agent/watch.rs`) through the usual permission path, with
  the watched tool's own rules and remembered approvals. Every check, the
  first and the later ones, runs the watched tool's `PreToolUse` and
  `PostToolUse` hooks under that tool's name (`tools::watch::before`,
  `after`): a hook that blocks it fails the check, one that rewrites its
  input or output is obeyed. Later checks have no model, so a model hook
  fails them. Shell commands are
  `Monitor`'s; loop-run tools cannot be watched. An MCP tool keeps its
  server (`Tool::kept`) after the run ends, with the run's sign-in, so a
  token that expires ends the watch with the failure. `KillShell` and
  `/bashes` stop and list watches; a client's go when it does. A run
  that started a watch still running cannot call `ScheduleWakeup` or
  `CronCreate`, and a second watch of the same tool and input is refused
  (`domain::watch::while_watching`, `duplicate`): the agent is told to
  end its turn and wait for the event.
- **A project's scripts**: JavaScript checked into a project is not the
  owner's. A script tool in the project's `.{command}/tools` asks before
  it runs, even in Bypass, and is left out when its name is already
  taken by a built-in, MCP or the owner's tool (`script_tool::load`,
  `join`); the owner's own may replace one. A script skill or command
  from the project's folders asks the same way when the agent loads it
  (`Agent::skill_call`, the ask names the file), never takes the place of
  the owner's, a plugin's or a built-in skill of that name
  (`skill::discover`), and gets no file, process or network access from
  an agent's initial prompt; typed by the user it runs with the
  settings' `scripts` access. The `scripts` setting follows the
  permission rules' rule: the owner's files set it, a checked-in file
  only turns access off (`ScriptAccess::layered`).
- **Feature flags** (`domain::features`, pure): the settings' `features`
  object turns a feature that is not core off (`"features": {
  "experimental-sandbox": false }`). Every feature is on until turned off; a
  feature still being tried carries `experimental-` in its name. The
  owner's files set a flag either way. A checked-in file only restricts:
  it may turn an ordinary feature off, never back on, and may turn a
  security feature on, never off (`Features::layered`). `KNOWN` lists the
  flags a person can switch, with what each does. The chat's `/features`
  (the `flags` feature: CLI and backend folders) lists them and turns one
  on or off in the owner's `~/.{command}/settings.json`
  (`prompt::owner_settings::keep_feature`); `{command} doctor` says
  whether the sandbox confines commands. The backend reads the same flags
  at start (`LocalServices::turned_off_in`), so a backend feature folder
  turned off in the settings joins `off` and its `handle` and `run.rs`
  are skipped.
- **Git offer** (`domain::git_offer`, pure; flag `git-offer`): the chat
  holds a new conversation's first prompt when the folder is not in a git
  repository and asks, with the questions panel, whether to initialize
  one (`tui/git_offer.rs`). The backend's `workspace` feature says whether
  to offer (`git-offer`) and runs `git init` with an empty first commit,
  so worktrees work at once (`git-init`; without a git identity the
  commit is skipped and the chat says so). Any answer
  sends the held prompt; "don't offer again" turns the flag off. It is
  offered once per chat, never on a resumed conversation or in print mode.
- **Sandbox** (`experimental-sandbox`, macOS only): `Bash` and `Monitor`
  commands run confined by the operating system. The rule is pure in
  `domain::sandbox`: a command reads anything but writes only inside the
  project, the added folders, a session worktree's repository, the
  temporary folders, a short list of package caches under the home folder
  and the owner's `sandbox.allowWrite`; the project's and added folders'
  `.{command}/settings.json` and `settings.local.json` stay unwritable, so
  a command cannot grant itself permissions; `sandbox.network: false`
  cuts the network. Only the owner's files widen it; a checked-in file
  can only cut the network. `platform::sandbox::confined` enforces a
  `Confinement` (on macOS `sandbox-exec` with a generated Seatbelt profile,
  `macos/seatbelt.rs`; elsewhere the command runs as it is, `SUPPORTED`
  says which) and knows nothing of the domain. `Policy::sandbox` holds the
  run's settings (`None` when the flag is off; a sub-agent keeps its
  parent's) and `Workspace::confined` joins the two for each command, so
  a session that moved to a worktree is confined to it. A command the
  sandbox refused ends with a note saying so and how the owner widens it.
  Command hooks are confined the same way (`hooks/handlers.rs`); MCP
  servers, script tools (which have their own `scripts` access) and
  language servers are not. The agent may ask to run one `Bash` command
  with `unsandboxed: true`: when the sandbox is on, that call is always
  an ask that says so, in every mode, Bypass included, before any rule,
  hook or pre-approval could allow it (`agent/permission.rs`); dontAsk
  refuses it.
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
  conversation (`model/conversation_key.rs`); on the `messages` wire the
  system prompt and the last tool each end a cached prefix and the
  request's own `cache_control` caches the conversation up to its last
  block, which the Anthropic and Claude providers send as they are. Read
  sends a note for lines of an unchanged file that lie inside a range it
  already showed (`tools/seen_files.rs`) and stops before 100k
  characters; long Bash output keeps its start and end. A background
  agent's report handed over when the run was about to finish is saved
  and previewed like any long tool result (`agent/mod.rs`). Grep groups matching lines under their
  file's path and shows 25 matches a file, 300 characters a line and 200
  results (`head_limit` changes the last), saying how much each left out.
  An MCP tool's structured result is sent as one line of JSON. At the
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
  session's model. A page of 6,000 characters or less skips the reader and
  arrives fenced as untrusted content (`experimental-web-small-pages-direct`).
- **WebFetch's page** (`tools/builtin/web/page.rs`; each way of reading is
  its own flag, gathered in `domain::web_reading::WebReading` and kept in
  `Policy::web`: `experimental-web-main-content`, `-cleanup`, `-links`,
  `-formats`, `-empty-page-error`): asks for Markdown first,
  then reads the body by kind. HTML is parsed into a tree (`tree.rs`), its
  main content (`main`, `role=main`, a single `article`, else the column
  holding most of the prose) leads and the rest follows a divider; hidden
  parts, controls, labels repeated on every item and long runs of menu
  links outside the content are dropped or shortened; links are numbered
  once, deduplicated and listed at the end, same-site ones as paths. JSON
  becomes one line per record with the fields every record shares said
  once; RSS and Atom become their entries; Markdown and text pass through.
  An HTML page with almost no text is an error (built by JavaScript, a
  consent wall, a login, a bot check), not an empty answer.
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
- **Learning from corrections** (`experimental-learning`; rules in
  `domain::learning`): the SDK `Session` collects, from its conversation
  runs, what the user corrected (a denial with a note, a steer, repeats
  held back, a failed verification). After a run with two or more,
  `Session::learn` sends them as the `Learn` side call
  (`chores::lesson`, `content/prompts/chore-learn.js`), which proposes at
  most one lasting lesson as JSON: a memory, a skill or a permission
  rule, with why. The chat shows it; `/learn keep` saves it (the memory
  through project memory, the rule through `suggested_rules`, the skill
  by asking the agent to write it with skill-creator) and `/learn skip`
  drops it. Nothing is kept without the user.
- **Checkpoints** (`checkpoints.rs`, `engine/rewind.rs`): each prompt opens
  one (`EngineEvent::Checkpoint(n)`); `Edit`/`Write` keep a file's content
  before its first change in the run. `SessionCommand::Rewind(n)`
  (`Session::rewind`) restores those files and cuts the conversation before
  prompt `n` (only the files once the conversation was compacted). In a git
  repository a foreground `Bash` command is tracked too (`shell/changes.rs`):
  `git stash create` names the tree before it, and the files `git status`
  then shows changed keep their content from that tree, or are deleted on
  rewind if they are new. Ignored files and files untracked before the
  command are left alone, except before a command that destroys work
  git cannot bring back (`destructive-snapshot`; rule in
  `domain::destructive`: `rm` with paths, `git clean`, `git reset
  --hard`, `git checkout -- …`/`git restore .`, `git stash -u`): then
  `shell/snapshot.rs` keeps every file under the removed paths, and the
  untracked files (ignored ones too for `git clean -x`/`-X`), in the
  run's file history before the command starts, up to 5,000 files and
  200 MB; the result says how many were left out. The chat's `/rewind` picks a prompt;
  `/retry` rewinds to the last one and sends it again. When a run fails
  because the provider stayed unavailable (`FailureKind::Unavailable`,
  after the model's own retries), the chat counts down 30 s and retries
  once by itself (`tui/auto_retry.rs`); Esc or a new prompt cancels it.
- **Live output**: a tool that executes (Bash) runs inside
  `tools::progress::watched`; the shell's wait loop reports the last line
  of output and the agent emits it, redacted, as
  `EngineEvent::ToolProgress`, which the chat shows under the running tool
  until its result replaces it.
  `SessionCommand::Branch(n)` (`branches`; `Session::branch`, the chat's
  `/branch [n]`) instead starts a new session holding the conversation
  before prompt `n` and the checkpoints before it (`checkpoints::branch`,
  refused once the conversation was compacted past it), leaves the
  original session and the files as they are, and emits `SessionStarted`
  for the new id and `Branched`; the chat cuts its transcript there and
  puts the prompt back to edit.
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
- **Team memory** (`team-memory`; rules in `domain::memory::team`): a
  project may check in `.{command}/memory/*.md`, one memory per file
  with optional `kind` and `tags` front matter. They are read on every
  brief and search (`features/memory/team.rs`), sanitized, given ids
  from 1,000,000 up and an origin `team:<file>`, and recalled and found
  next to the owner's own; they are never marked used or changed by the
  agent. `/memory share <id>` writes one of the owner's memories there
  to commit for the team.
- **Store**: an unreadable book is renamed `.json.corrupt-<secs>`, never
  overwritten. Saves `fsync` before the rename. Without an app data folder
  memory lives only in the process.

Context features (each `experimental-`, flags in `domain::features::learnings`;
on until turned off, like every flag):
- **Memories about files** (`experimental-memory-anchors`; rules in
  `domain::memory::anchors`): a memory may name the files or folders it is
  about (`Memory::touches`, project-relative, at most eight; a checked-in
  file's `touches: [..]` front matter). The brief tells the agent it may
  add `"touches"` to a `remember` entry and hands the run every anchored
  memory its prompt did not recall (`MemoryBrief::anchored`). The engine
  keeps them in `Workspace::anchored_memories`; `Read`, `Edit` and `Write`
  append the memories about the file as `<memory>`, each once per run
  (`Tracking::seen_memories`), with whose it is when the file says so.
- **Decisions** (`experimental-structured-learnings`; rules in
  `domain::memory::decision`): a `remember` entry may carry `decided`,
  `ruled_out` and `watch_out` in place of `text`; it is kept as one line
  (`Decided: … Ruled out: …; … Watch out: ….`). `/memory share` writes such
  a memory as `## Decided`, `## Tried and ruled out` and `## Watch out`
  sections, with `author` (`git config user.name`) and `date`, and reads
  them back the same (`Memory::credit`).
- **Checking a change** (`experimental-learning-check`; rules in
  `domain::memory::check`): `/check` (CLI folder `learning_check`) asks
  the memory feature for `check`: the backend finds the changes
  (`features/memory/check.rs`: since the merge base with `origin/HEAD`,
  `main` or `master`, or since `HEAD` on that branch, plus untracked
  files), picks the memories about the changed files and those the
  diff's words find, and returns a prompt the chat sends as the user's
  own. The agent reads the diff, changes nothing, and lists each conflict
  with the memory it goes against.
- **Seeding from past sessions** (`experimental-memory-seed`; rules in
  `domain::memory::seed`): `/memory seed` reads the prompts of the
  project's recent sessions (`features/memory/seed.rs`, the same list
  `/rules` reads) and offers, numbered, the sentences said in two or more
  of them that no memory already holds; `/memory seed keep <n>…` keeps
  them as the user's procedural memories, tagged `seeded`.
- **Code map** (`experimental-code-map`; crate `code-map`, backend folder
  `code_map`): inside a git repository (never the home folder) each
  prompt refreshes the project's tree-sitter index, cached in the data
  folder's `code-map/<project>.json` and re-parsed only for files whose
  size or time changed, and `run.rs` puts the files and symbols that
  match the prompt, with how many places call them, in
  `PromptRequest::code_map`, which `compose` adds after the prompt as a
  `<code-map>` block. Calls are matched by name, so common names gather
  callers from unrelated types.

## Language servers

The agent navigates code through language servers. What they
mean is pure in `domain::lsp`: the built-in servers and the
`lspServers` layers (user, project, plugins' `.lsp.json`), the server and
project root for a file, the `LSP` tool's questions and how answers and
diagnostics read. The `language-servers` crate (`agent_engine::lsp`) runs them: `LspClient` (one
process, `Content-Length` framing, a reader thread routing answers and
published diagnostics) and `LspPool` (one server per name and root, kept
by the `NativeEngine` across runs, ended after 10 idle minutes), behind
`LanguageServers` so tests script them (`lsp::fake`).

A built-in server missing from PATH is installed on first use:
`domain::lsp::install` says how for each platform, every server at a
pinned version (rust-analyzer's and clangd's GitHub releases, each with
the SHA-256 its download must have, or nothing is installed; `npm
install` for typescript, on TypeScript 6 because 7 has no `tsserver`,
and pyright; `go install` for gopls) and `lsp::Downloads` does it into
`<local data>/lsp/<server>/`,
staging in a sibling folder and renaming it into place; an install
another process finished meanwhile is kept as it is. The backend's
engine turns it on (`LspPool::installing`); without it only PATH
counts. Only the `LSP` tool starts an install and waits for it
(`Wait::ForInstall`); `Edit`/`Write` diagnostics never start or wait for
one (`Wait::Never`). An install runs once, on its own thread, and a
failure (a crashed installer too) is kept and repeated until the pool's
idle sweep forgets it. While one runs, an edit's `<diagnostics>` say the
server is still installing, and the `LSP` answer that waited for it starts
with "Installed the <server> language server on first use."
A run gets `Workspace::lsp` and the `LSP` tool when one of its servers is
available: on PATH, already installed, or installable, which needs the
recipe's tool on PATH (`Recipe::tool`: `npm`, `go`; none for a download)
and no failed install still remembered. The system prompt's environment
names the available servers and their extensions, while the tool knows
every configured one, so a file whose server is missing is answered with
what is not installed; `Edit` and `Write` append the
file's errors and warnings as `<diagnostics>`. `tests/real_lsp.rs`
(ignored) runs against clangd and installs rust-analyzer, typescript and
clangd from the network.

With `experimental-blast-radius` (`Workspace::blast_radius`; rules in
`domain::lsp::blast`), `Edit` and `Write` also tell the agent who uses
what it changed: the lines between the unchanged head and tail of the
file, the innermost functions, methods, types and constants around them
(`textDocument/documentSymbol`, at most three) and their references
outside the file (`textDocument/references`, two seconds each), grouped
by file in `<callers>`. Like diagnostics it never starts or waits for an
install (`Lsp::callers`).

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
waiting call), so it can move to a remote backend unchanged.
`Builder::backend` takes any `protocol::Backend` instead of the in-process
one (`tests/remote.rs` runs a session through a backend that only sees
JSON); what the builder adds itself (agents, skills, commands, MCP
servers) needs the local backend and is refused otherwise. What a host adds reaches every
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

## Logs

The binary installs a `tracing` subscriber at start (`cli::logging`) that
writes to `<local data>/logs/{command}.log`, never to the terminal the chat
draws on. `{COMMAND}_LOG` takes an `EnvFilter` (`backend=debug`, `info`);
warnings and errors are kept otherwise. A log past 5 MB is moved to
`.log.1` when the next process starts.

## Secrets

Provider keys and subscription tokens live in one
sealed file (`backend::system_secrets`): `secrets.json`, each
entry encrypted with XChaCha20-Poly1305 under a key in `secrets.key`
(both `0600`), re-read on every access and changed only under an exclusive
lock (`platform::file_lock`, a hidden `.secrets.json.lock` beside it), so two
processes never drop each other's entries. The provider book and the memory
book change the same way (`ProviderStore::update`, `MemoryStore::update`), and
every store writes through a unique temporary file and a rename. The system keychain is not used: macOS would ask to
allow each new unsigned build of the binary.

## Conversations and the catalog

- **The conversation is folded in `domain`** (`domain::conversation`): a
  `Transcript` takes `EngineEvent`s and keeps the entries shown, the
  session's `SessionStatus`, the ask that waits (and which sub-agent asks:
  a sub-agent's `Asked` is first emitted as its own `AgentEvent`), each
  sub-agent's own conversation and the folder a session moved to. The
  terminal draws from this fold. A `Transcript::recording()` also tells what
  each event changed (`Change`); `replay` rebuilds a conversation from a
  session's log.
- **The catalog of sessions is the backend's** (`SessionCatalog`, one
  `sessions.json` per owner; rules in `domain::catalog`). Every run that is
  not a side chore lists its session when it starts and updates its status
  when it asks, is answered and ends (`catalog::Listing`, from the run
  sink). The `sessions` feature lists, renames, archives, deletes and marks
  them seen, and reads one's history back (`Prompted` and `Event` records).
  The conversations the terminal kept in the settings are brought in once.
  The terminal's `/resume` lists the folder's sessions from this catalog,
  and the name the terminal gives a conversation is written to it.
- **One SDK client per session.** Queries carry no correlation id and
  notices, schedules and background commands are kept per client, so
  `Sdk::sibling()` opens another client over the same `LocalServices`.
- **A line typed with `!`** runs through the backend's `shell` feature
  (`run`, with `platform::typed`) and is told to the agent with the next
  prompt (`conversation::with_shell`). It runs on this machine as the user,
  not through the engine's `Launcher`, so a multi-user server must refuse or
  wrap it. The feature's `output` shows a background command's last output
  without moving what the agent has read.

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
side with their permission asks and how long each waited for an answer,
the notes a tool collected (repeated, refused, edited), sub-agents' lanes
under the tool that started them, background ones too (each run numbers
its sub-agents from zero, so `trace::route` gives every `AgentSpawned` a
key of its own and sends that index's later events and calls to it),
compactions,
rewinds, fallbacks, verdicts, review findings, errors). `trace::runs` then
cuts that tree into one run per prompt, each a flat list of spans timed
from the run's start (`run`, `llm`, `tool`, `agent`, `wait`, `eval`,
`event`) with its cost, tokens, cache share, waiting time, wasted time
(failed tools) and signals; a model call's context split into system
prompt, tool definitions and conversation is estimated from the recorded
system prompt and tools. The backend's `trace` feature does this for the
session and for the project's other conversations of the last 30 days
(their texts cut shorter) and adds `trace::insights`: usage per day and
model, latency and cache per model, calls, failures and asks per tool,
where the time went, check counts and the latest findings. The CLI
writes the report into `trace.html` and keeps the page at
`~/.{command}/traces/<session>.html`. `{command} trace` opens that file;
`/trace` in the chat serves the page live instead: `trace::live` listens
on 127.0.0.1 on a free port, answers only requests carrying a random
token, and streams Server-Sent Events. The feature's tick asks the
backend once a second while a run is going (and once after it ends) for
the followed session's runs alone (`"only": "current"`) and sends them
when they changed; the page injects a `#live` block naming the stream,
swaps that session in place and follows its newest run. The server lives
as long as the chat; when it cannot start, `/trace` opens the file.

The live page also shows the folder's sessions as a board (needs you,
running, done, failed) from the catalog (`trace::board`, read at `GET
/sessions` and refreshed by the tick), and writes to the session it shows:
`POST /prompt` (the token, and the page's own origin when a browser sends
one) leaves the text for the tick, which hands it to the chat as if typed
when it is the chat's own session, steers a run the page started, or
otherwise resumes that session on a sibling SDK client with the chat's
model, mode and effort (`trace::outside`). Such a run answers no asks: they
are denied, as in `Run::text`. `POST /watch` names the session the page
opened, so its runs are sent once and again while the catalog says it is
active.

`trace.html` is built, not written by hand: its source is the React,
Tailwind and shadcn/ui app in `web/trace` (types in `src/types.ts` mirror
`trace::insights::Report`). `npm run build` there type-checks, bundles it
into one file with no network access at runtime and copies it over the
CLI's template; the built file is committed so a Rust build never needs
Node. The page reads the report from the `{{TRACE}}` JSON block and,
when it is left unfilled, shows a sample report for development.
