# Feature flags: inventory and what to do with each

## The rule: experimental features ship on

Decided by the owner, 2026-11-17. Every experimental feature is on by default.
It ships on, has an owner and an exit criterion, and the owner either turns it
off or promotes it to permanent; no experimental feature waits, off, for
someone to find it. This is what the code does: `Features::on` returns `true`
for any flag nothing turned off.

The one exception is `experimental-code-mode`. It stays off (`OPT_IN` in
`crates/domain/src/features.rs`) because turning it on makes REPL the agent's
only tool: every other tool is then reachable only from a script, which changes
how the agent works rather than adding something beside it. Only the user's own
settings can turn it on. On 2026-11-17 the owner merged `experimental-code-mode-only`
into it, so code mode works as in the reference agent: off, or REPL only
(`decisions/0015-behave-like-the-reference-agent-under-our-own-names.md`).

"The owner" in this document is the project's author, named as `author` in
`config.toml`; files under `docs/` do not spell that name. Every remaining
experimental flag is owned by them; "Owners and exit criteria" below lists
each flag with its criterion.

This rule supersedes the proposal, in the first version of this document, to
make the flags in "Keep as experimental" default-off. The other
recommendations ("Make permanent", "Keep as a user setting", "Remove the
feature", section 4's cross-cutting changes and section 5's chore settings)
stay as a backlog: none of them is done, and each still needs the owner's go.

## About this inventory

Written 2026-11-16 from a read of the code; no build or test was run for it.
Updated 2026-11-17 for the default-on rule above; the paths in the tables
were checked against the code again that day. Paths are relative to the
repository root. Where something could not be determined from the code, the
text says so.

The inventory below covers every flag constant in
`crates/domain/src/features.rs` and
`crates/domain/src/features/learnings.rs`. Since 2026-11-17 there are more,
in `crates/domain/src/features/backend.rs`: the declared switches of whole
backend features (`memory-search`, `schedules`, `plugins`, `trace`,
`channels`, `declared-providers`). They are described in "Backend feature
switches" below and are not part of the recommendations table. The numbers
below count the rows of each section; `xtask lint` keeps them equal.

| Recommendation | Flags |
| --- | --- |
| Make permanent | 21 |
| Keep as a user setting, not a flag | 6 |
| Keep as experimental | 8 |
| Remove the feature | 1 |

## 1. How flags work today

### Default

`Features::on(name)` returns the stored value, or, when nothing is stored,
`true` unless the name is in `OPT_IN`. `OPT_IN` holds one name,
`experimental-code-mode`. So every other flag is on from the first run,
including every flag whose name starts with `experimental-`. That is the rule
the owner decided on 2026-11-17 (see the top of this document), not an
accident to correct.

The same rule applies to any string. `Features::on("typo")` is `true`. A
misspelled name in a settings file is stored, never matches a real flag, and
the real flag stays on. Nothing warns. `Features::layered` only checks that
each value is a boolean.

`/features <name> on|off` is stricter than the file: it goes through
`keep_feature` in `crates/agent-engine/src/prompt/owner_settings.rs`, which
refuses a name that `domain::features::known` does not find. Only hand-edited
settings can carry an unknown name.

### Backend feature switches

Until 2026-11-17 unknown names were load-bearing: the backend copied every
name that was `false` in the `features` object into `LocalServices::off`, once
with hyphens and once with underscores, and the dispatcher skipped a feature
folder whose folder name was in that set. `"workspace": false` or
`"sessions": false` therefore turned off a folder that no flag named, and the
typed `FeatureOff` error told the user to run `/features <folder> on`, a
command that refuses every one of those names.

A backend feature is now turned off only through a declared, known flag:

- `FeatureName::switch` (`crates/app/src/api/features/mod.rs`) names the flag
  of each feature folder, or `None` for a folder the program cannot work
  without.

  | Feature folder | Switch |
  | --- | --- |
  | `channels` | `channels` |
  | `code_map` | `experimental-code-map` |
  | `declared_providers` | `declared-providers` |
  | `output_savings` | `experimental-output-savings` |
  | `plugins` | `plugins` |
  | `schedules` | `schedules` |
  | `suggested_rules` | `suggested-rules` |
  | `trace` | `trace` |
  | `flags`, `memory`, `sessions`, `shell`, `workspace` | none: always on |

- `TurnedOff::follow` (`crates/app/src/turned_off.rs`) keeps only the names
  `domain::features::known` finds, as written. There is no
  hyphen-to-underscore copy, so a folder name, a misspelling or any other
  unknown name in a settings file is inert in the backend, as it already was
  in the engine.
- The dispatcher (`crates/app/src/features/mod.rs`) refuses a request, and
  skips a folder's part in preparing a run, when the folder's switch is in
  that set. `Context::on` (`crates/app/src/features/context.rs`) reads the
  same set by flag name (`git-offer`, `memory-search`).
- `FeatureError::FeatureOff` prints `<folder> is turned off (/features
  <switch> on)`. Tests in `crates/app/src/features/switch_tests.rs` type the
  command each refusal names and check the feature answers again, and that
  the five always-on folders ignore every name.
- Project memory has had no switch since 2026-11-17: the owner decided it is
  always on. `"memory": false` left in a settings file is an unknown name and
  is ignored like any other.
- `memory-search` used to be the program-only name `memory_search`, reachable
  from a settings file through the underscore copy. It is a declared flag now;
  tests turn it off by that name.
- `declared-providers` off also skips loading declared providers at startup
  (`LocalServices::in_project`).

What stopped working: `"workspace"`, `"sessions"`, `"shell"` and `"flags"`
set to `false` in a settings file (now ignored), and the folder spellings
`code-map`, `code_map`, `output-savings`, `output_savings`,
`suggested_rules`, `declared_providers`, `memory_search` (use the flag's
name).

`Features::on("typo")` is still `true` and `Features::layered` still stores an
unknown name; rejecting them is the remaining step in section 4.

### Layering

`crates/agent-engine/src/prompt/settings.rs` (`load`) reads, in this order:
the home `.<command>/settings.json` and `settings.local.json`, then for each
ancestor folder down to the working folder its `.<command>/settings.json` and
`settings.local.json`, then the home `.<command>/managed-settings.json` last.
Each file is an owner layer or a project layer:

- Owner: anything in the home folder, the managed file, and a
  `settings.local.json` that git does not track
  (`ProjectTrust::local_is_the_owner_s`).
- Project: a checked-in `settings.json`, or a tracked `settings.local.json`.

`Features::layered(value, owners)` applies a layer. An owner layer may set any
flag either way. A project layer is accepted only when it restricts:

- for a flag in `SECURITY` (`experimental-sandbox`, `redact-secrets`,
  `experimental-dependency-guard`), only `true` is taken;
- for every other flag, only `false` is taken.

Later layers overwrite earlier ones, so a project can turn off a non-security
flag the owner explicitly turned on, and can turn on a security flag the owner
turned off. The managed file has the last word. Tests:
`a_checked_in_file_only_restricts` (domain),
`only_the_owner_turns_the_sandbox_off_or_widens_it` and
`managed_settings_in_the_home_have_the_last_word_over_the_project`
(`crates/agent-engine/src/prompt/settings_tests.rs`).

### Where flags are read

There are four readers with three different lifetimes.

| Reader | When | What it decides |
| --- | --- | --- |
| `crates/agent-engine/src/engine/job.rs` (`Job::run`) | Every run, from freshly loaded settings | Fills `workspace.policy` (sandbox, redact, snapshots, dependency guard, escalation, auto review, follow-through, affected tests, session claims, code mode, web reading, output shaping), drops the budget, filters project criteria, anchors and blast radius |
| `crates/agent-engine/src/brief.rs` | Every run | Hides the `follow-pr` skill (`skill_flag`) |
| `crates/app/src/services.rs`, `crates/app/src/features/flags/mod.rs` | When the client is built (`crates/app/src/client/mod.rs`), and again at every `/features` list or change | The `off` set described above |
| `crates/app/src/features/memory/*`, `code_map/mod.rs` | Every request to those features; each call reloads the settings files | Memory and code-map flags |
| `crates/cli/src/tui/features/flags/mod.rs` | Once on the first tick, and after each `/features` call | `App::turned_off`, read by `tui::features::on` |

So a `/features` change reaches the engine on the next run, and the terminal
and the backend's `off` set at once. A settings file edited by hand reaches
the backend's `off` set at the next `/features` call or restart.

Chore runs (section 5) return from `Job::run` before settings are loaded and
read no flag at all.

### How the user toggles them

- `/features` lists every known flag with its state; `/features <name> on|off`
  writes `features.<name>` into the home `.<command>/settings.json`. The same
  screen is the "Features" entry in the settings list
  (`crates/cli/src/tui/settings_picker.rs`).
- By hand: a `features` object of booleans in any of the settings files above.
- Picking "never" on the git offer writes `git-offer: false` the same way.
- `doctor` (`crates/cli/src/doctor.rs`) reports the sandbox flag.

`Listed.experimental` is computed from the name prefix; `Listed.security` from
the `SECURITY` list. `crates/cli/src/tui/features/docs/coverage.rs` fails if a
known flag is missing from the `experimental-features` help page.

## 2 and 3. The flags, grouped by recommendation

Columns: what it gates; where it is read; what OFF does; cost when ON; tests
for both states. "Tests" means: is there a test that turns the flag itself on
and off, or only a test of the code behind it? For most engine flags the
gated code is tested through `workspace.policy` directly and the one line in
`job.rs` that maps the flag to the policy has no test. Those are marked
"partial (policy only)".

Security controls are marked **S** after the name.

### Make permanent (21)

Remove the flag and the OFF path, keep the behaviour.

| Flag | Gates | Read in | When OFF | Cost when ON | Tests on/off | Why permanent |
| --- | --- | --- | --- | --- | --- | --- |
| `redact-secrets` **S** | Replaces tokens, keys and `.env` values in tool output, progress lines and permission prompts | `crates/agent-engine/src/engine/job.rs`; applied through `Policy::redacted` in `tools/workspace/policy.rs`, `agent/dispatch/mod.rs`, `agent/dispatch/after.rs`, `agent/permission/mod.rs`, `tools/builtin/shell/shaped.rs` | Secrets reach the model and the session log as they are | Reads the project's `.env*` files once per run; a text pass over each tool result. No model call, no process | Partial (policy only: `crates/agent-engine/tests/agent_loop/redaction.rs` sets the policy; no test names the flag) | A product that logs transcripts should not have a "leak secrets" mode. If false positives appear, fix the patterns |
| `destructive-snapshot` | Copies files that `rm -r`, `git clean`, `git reset --hard` and similar would destroy, so `/rewind` can restore them | `job.rs`; `tools/builtin/shell/snapshot.rs` | `/rewind` cannot bring back files a shell command deleted | Only on a destructive command: a directory walk and, for untracked files, `git rev-parse` plus `git status`. Capped at 5,000 files and 200 MB | Partial (policy only; `snapshot.rs` tests both policy values) | It is part of what `/rewind` promises. The cost is bounded and paid only on destructive commands |
| `experimental-output-cleanup` | Runs Bash with plain-output environment variables and strips escape codes, redrawn progress and repeated lines | `crates/domain/src/tools/output/mod.rs` (`Shaping::from`); `tools/builtin/shell/shaped.rs` | The model receives raw terminal output | A text pass per command | Yes for the mapping (`each_step_follows_its_own_flag` only toggles the filters flag, so partial for this one); policy tested in `bash.rs` | Pure text cleanup that only removes noise. No reason to offer the noisy variant |
| `experimental-web-main-content` | WebFetch puts the page's main content first | `crates/domain/src/tools/web_reading.rs`; `tools/builtin/web/distill.rs` | Page is read in document order | None beyond parsing | Yes (`web_reading.rs` tests the mapping, `page_flag_tests.rs` the OFF output) | Deterministic HTML-to-text choices. Six flags for one tool's formatting is the clearest case of the audit's complaint |
| `experimental-web-cleanup` | WebFetch drops hidden parts, buttons, repeated labels and menu runs | Same two files | Those parts stay in the text | None | Yes (same tests) | Same |
| `experimental-web-links` | WebFetch numbers links and lists addresses once at the end | Same two files | Each address stays beside its text | None | Yes (same tests) | Same |
| `experimental-web-formats` | WebFetch asks for Markdown first and compacts JSON and feeds | `web_reading.rs`; `tools/builtin/web/mod.rs`, `page.rs` | No `Accept` header; JSON and feeds come back as sent | None | Yes (same tests, plus one in `web/mod.rs`) | Same |
| `experimental-web-empty-page-error` | WebFetch reports a near-empty page as an error | `web_reading.rs`; `tools/builtin/web/page.rs` | The empty shell is handed to the model as content | None | Yes (same tests) | Same; the OFF state is strictly worse |
| `experimental-web-small-pages-direct` | A fetched page of 6,000 characters or less goes to the agent fenced, without the separate reader | `web_reading.rs`; `tools/builtin/web/mod.rs` picks `guarded::digest` or `guarded::read_apart` | Every fetch makes one extra model call to the reader (small model, falling back to the session's model) | ON is the cheaper state: it saves a model call per small page | Partial (mapping tested; `guarded.rs` tests both functions; both values of the choice tested in `tools/after.rs`) | The flag only exists to turn a saving off |
| `branches` | The `/branch` command | `crates/cli/src/tui/features/branches/mod.rs` | `/branch` prints "turned off" | None until used | No test of the OFF notice was found; the command's listing is tested | A flag on a command the user must type is noise. The engine side (`SessionCommand::Branch`) is not gated at all |
| `budget` | Whether a `/budget` limit is passed to the meter | `job.rs` (sets `request.budget = None`) | `/budget 5` still answers "model calls stop once this session has spent it", and nothing stops: the terminal command (`crates/cli/src/tui/features/budget/mod.rs`) does not read the flag | None; acts only when the user set a limit | No test names the flag | The OFF state is a trap: a spending cap that silently does not apply. The user already opts in by setting a limit |
| `suggested-rules` | The `/rules` command's backend | `FeatureName::SuggestedRules.switch()` in `crates/app/src/api/features/mod.rs`, checked by the backend dispatcher (`crates/app/src/features/mod.rs`) | `/rules` fails with "suggested_rules is turned off (/features suggested-rules on)" | None until used; then it reads recent session logs | Yes (`crates/app/src/features/switch_tests.rs`, `flags/mod.rs`) | Same as `branches` |
| `team-memory` | Reading checked-in memories from `.<command>/memory/*.md` | `crates/app/src/features/memory/team.rs` (`load_if`), used by `memory/mod.rs` and `memory/check.rs` | Checked-in memories are ignored; `/memory share` still writes them | Reads the folder's Markdown files on every request that carries a memory brief, after reloading settings | No test names the flag | The folder's existence is the switch. A project that does not want it has no files there |
| `project-criteria` | Using the project's checked-in `verification.md` as the run's criteria when the user set none | `job.rs` | Runs are not judged unless the user set criteria | When the file exists: a judge on the review-role model at the end of each run that is not in plan mode (`tools/builtin/verification/review.rs`; not read in detail) | No test names the flag; `crates/domain/src/run/verification/tests.rs` covers the criteria | The file is the opt-in. Note for the decision: a checked-in file can make the owner pay for a judge on every run, and today the owner's only defence is this flag. If that matters, make it an owner setting instead |
| `experimental-escalation` | After three identical failed calls, nudges the agent to the Advisor, or switches the run to `escalationModel` (twice at most) | `job.rs`; `StuckWatch` in `agent/run.rs` and `agent/turn/tools.rs`; rules in `crates/domain/src/run/stuck.rs` | Nothing happens when the agent loops | Nothing by itself. `domain::stuck::help` returns `None` when there is neither an Advisor model nor `escalationModel`, so with default settings ON does nothing. With `escalationModel` set, later turns run on that model | Partial (policy only: `tests/agent_loop/escalation.rs`) | It is already switched by two settings (`advisorModel`, `escalationModel`). The flag is a third switch for the same thing |
| `experimental-session-claims` | Warns the agent when another live session in the same folder edited the file it is changing | `job.rs`; `tools/workspace/claims.rs`, called from `tools/builtin/files/edit.rs` and `write.rs` | No warning | Per edit: an exclusive file lock and a read and rewrite of one small JSON file in the temp folder | Partial (policy only; `claims.rs` tests both policy values) | Cheap, and it only adds a warning. Two sessions overwriting each other is a real failure with no other guard found |
| `experimental-load-on-call` | A held-back tool the agent calls by name is loaded and run instead of refused | `policy.rs` (`flagged`); `agent/dispatch/mod.rs` (`loads_on_call`) through `Deferred::load` | The call fails with "load it with ToolSearch" | None: a lookup in the held-back list on a call to a tool that is not offered | Yes (`tests/agent_loop/safeguards.rs`, both values; `policy_tests.rs`) | Weak models skip ToolSearch and spend a turn on the refusal; nothing found depends on the refusal |
| `experimental-memory-anchors` | Memories name the files they are about and are shown when the agent reads or edits those files | `job.rs`; `crates/app/src/features/memory/mod.rs`; shown by `tools/workspace/anchors.rs` from `read.rs`, `edit.rs`, `write.rs` | Memories are only recalled by prompt | A lookup per file read or edit in an in-memory list | Yes (`crates/agent-engine/tests/engine/anchors.rs`, `crates/app/src/features/memory/anchors_tests.rs`) | It changes the memory format. With no users, pick one format now; carrying two means every memory test doubles |
| `experimental-structured-learnings` | Decisions are stored as decided / ruled out / watch out for; shared files carry author and date | `memory/mod.rs`, `memory/team.rs`; `crates/domain/src/knowledge/memory/brief.rs`, `team.rs` | Free-text memories | None; `/memory share` runs `git config user.name` | Partial (OFF tested in `anchors_tests.rs` through the brief; the `share` branch was not found tested with the flag) | Same reason as anchors: one format |
| `experimental-learning-check` | The `/check` command | `crates/cli/src/tui/features/learning_check/mod.rs`; `crates/app/src/features/memory/check.rs` | `/check` prints "turned off" | None until typed; then a git diff and one ordinary agent run | Yes (ON in `learning_check/mod.rs`, OFF in `anchors_tests.rs`) | A flag on a typed command, checked twice |
| `experimental-memory-seed` | `/memory seed` | `crates/cli/src/tui/features/memory/mod.rs`; `crates/app/src/features/memory/seed.rs` | The command prints "turned off" | None until typed; then it reads recent session logs. No model call was found in `seed.rs` | Partial (OFF in `anchors_tests.rs`; no ON test was looked for beyond the constant) | Same |

### Keep as a user setting, not a flag (6)

These are preferences or security postures. They should be named settings
with a documented default, not entries in an experiment list.

| Flag | Gates | Read in | When OFF | Cost when ON | Tests on/off | What the setting should be |
| --- | --- | --- | --- | --- | --- | --- |
| `experimental-sandbox` **S** | Confines Bash and Monitor commands, command hooks, scripts' `ctx.exec` and stdio MCP servers to the project's folders. `platform::sandbox::support()` says whether this system can (macOS; Linux with Landlock). `sandbox.required`, true by default on Linux and macOS, refuses confined commands and does not start stdio MCP servers where it cannot; the owner can set it to false, a project only to true | `job.rs`; `tools/workspace/confined.rs`; `crates/domain/src/safety/permission/engine.rs` (`leaves_the_sandbox`); `crates/domain/src/safety/sandbox.rs` (`REQUIRED_BY_DEFAULT`, `ServerSandbox`); `crates/agent-engine/src/mcp/stdio_sandbox.rs`; listed as unavailable elsewhere by `crates/app/src/features/flags/mod.rs`; `crates/cli/src/doctor.rs` | Commands and stdio MCP servers run unconfined | Each command is launched through the platform's confinement. The launcher itself was not read, so the per-command overhead is not determined | Yes (`settings_tests.rs`, `owner_settings.rs`, `flags/mod.rs` for the flag; `tests/agent_loop/unsandboxed.rs` for the policy) | There is already a `sandbox` settings object (`allowWrite`, `network`). Add `sandbox.enabled` there and keep the rule that a project layer can only turn it on. It is not an experiment: it is on by default and a security control |
| `experimental-dependency-guard` **S** | Before a Bash command that installs packages, looks each one up in its registry and asks the user when it is missing, days old or a near-typo | `job.rs`; `agent/permission/policy.rs` (`package_concerns`); `guard/packages.rs`; rules in `crates/domain/src/safety/dependencies.rs` | Installs run under the normal permission rules only | One HTTP GET per package named in an install command, before the command runs. Ordinary commands look nothing up (tested). A failed lookup counts as unknown | Partial (policy only: `tests/agent_loop/dependency_guard.rs`) | A security setting beside the sandbox, same project-can-only-turn-on rule. Someone on a private registry or offline will need to turn it off, so it cannot be permanent |
| `auto-review` | When a run changed files, a read-only reviewer sub-agent checks them before the run ends | `job.rs`; `tools/builtin/code_review/review.rs` | No review | The most expensive flag. One sub-agent per run that changed files, on the session's own model (`agent.request.clone()`), up to 30 turns, with tools. Its findings are sent back to the agent, which then works again. Skipped once a review already happened in the run | Partial (policy only: `tests/agent_loop/code_review.rs`) | `autoReview: true|false` in settings. Whether it defaults to on is a product and cost decision for the tech lead; it should not be hidden in the list of flags |
| `experimental-learning` | After a run in which the user corrected the agent, offers one lesson to keep | `crates/cli/src/tui/chores.rs` only | No offer | One model call on the session's model with the whole conversation, 600 output tokens at most, but only when at least two correction signals were collected (`crates/domain/src/knowledge/learning.rs`) | No test names the flag | One of the chore settings in section 5 |
| `cache-warning` | Status-line countdown of the prompt cache's last two minutes | `crates/cli/src/tui/features/cache_warning/mod.rs` | No marker | None; computed from the last call's time on each draw | No test names the flag (the marker itself lives in `crates/domain/src/models/cache_window.rs`) | A display preference, like any status-line item |
| `git-offer` | Offers to run `git init` on the first message in a folder that is not a repository | `crates/cli/src/tui/git_offer.rs`; `crates/app/src/features/workspace/mod.rs` (through the startup `off` set) | No offer | One backend call on the first prompt of a session | Yes (`crates/cli/src/tui/git_offer_tests.rs`, `crates/app/src/features/workspace/tests.rs`) | It already behaves as a remembered answer: "never" writes the flag. Store it as that (`gitOffer: "never"`), not as a feature |

### Keep as experimental (8)

Each of these adds work the user did not ask for on every edit, prompt or
request, or changes how the agent uses its tools. None has a measurement in
the repository showing it pays for itself. This section first proposed making
them default-off; under the 2026-11-17 rule they stay on (except
`experimental-code-mode`, off), and the owner decides each one
from daily use or an evaluation (`crates/domain/src/run/evals` exists; it was
not checked whether any of these flags has an eval). Owner and exit criterion:
"Owners and exit criteria" below.

| Flag | Gates | Read in | When OFF | Cost when ON | Tests on/off | What is unfinished or costly |
| --- | --- | --- | --- | --- | --- | --- |
| `experimental-follow-through` | When an answer ends promising more work, a small model judges it and the agent is sent back, twice at most | `job.rs`; `tools/builtin/follow_through/review.rs`; `crates/domain/src/run/follow_through` | The turn ends as the model ended it | A phrase match on the answer's ending first; only on a match, one call to the small model (300 output tokens, low effort), then possibly a further agent turn | Partial (policy only: `tests/agent_loop/follow_through.rs`) | Triggered by a fixed list of phrases. No evidence of how often it nudges wrongly. Area: agent-loop quality |
| `experimental-affected-tests` | After an edit, starts the tests covering the changed file in the background | `job.rs`; `tools/builtin/shell/affected.rs`, from `edit.rs` and `write.rs` | No background tests | A test process per edit to a file `domain::affected::tests_for` can map; the previous run with the same label is killed first. Its result is delivered to the agent later, adding context | Partial (policy only; `affected.rs` has three ON tests) | Starts processes nobody asked for; on a large project each edit can start a long test run. Area: tools |
| `experimental-blast-radius` | After an edit, tells the agent which files call the changed symbols | `job.rs`; `tools/workspace/blast.rs`, from `edit.rs` and `write.rs` | No caller list | Language-server requests inside the edit call (the test scripts a symbols answer and a references answer), so the edit waits for the server. Needs a language server to be available | Partial (`blast.rs` has one test of both field values; none names the flag) | Latency on every edit, and value depends on a server being installed. Area: language-servers |
| `experimental-code-map` | Indexes symbols with tree-sitter and adds pointers to matching code to each prompt | `crates/app/src/features/code_map/mod.rs` (reloads settings itself), run from `code_map/run.rs` | No pointers | On every prompt in a git project: a settings reload, a stat of the project's files against a cache, extraction for changed files, and up to 12 files of pointers added to the request. Index time on a large repository was not measured | Yes (`code_map/mod.rs` tests the flag off, `run.rs` the ON path) | Adds tokens to every prompt with no measure of whether they help. Area: code-map |
| `experimental-code-mode` | Off by default. On, REPL is the agent's only tool: it writes JavaScript that calls every other tool as a function | `crates/domain/src/tools/code_mode.rs` (`Mode::of`); `tools/repl.rs`, `agent/turn/model_call.rs`, `agent/repl.rs` | The agent calls its tools directly | Replaces the tool list with REPL, whose description declares every other tool; scripts run in the `script` crate's runtime, up to ten minutes each | Yes for the mapping (`code_mode_is_off_until_turned_on_and_then_the_repl_is_the_only_tool`); policy in `tests/agent_loop/repl.rs` | It changes how the agent works, so it is the one flag that starts off. Merged with `experimental-code-mode-only` on 2026-11-17. Area: agent-engine |
| `experimental-output-filters` | Bash hides passing tests, build steps and progress from cargo, git, npm, pytest and go test, keeping the whole output in a file | `crates/domain/src/tools/output/mod.rs`; `tools/builtin/shell/shaped.rs` | The model sees the full output | A file written per filtered command; fewer tokens to the model | Yes (`each_step_follows_its_own_flag`; `shaped.rs`) | It decides what the model does not see. A filter that hides the line that mattered costs more than it saves, and there is no measurement of that. Area: tools |
| `experimental-output-savings` | Records how much shorter each command's output became, for `/gain` | `output/mod.rs`; `shaped.rs`; `crates/cli/src/tui/features/output_savings/mod.rs` | `/gain` prints "off"; nothing is recorded | A counter per command | Partial (the OFF notice text is tested, not the gate) | It is the instrument for judging the filters. Keep or drop it together with them |
| `experimental-pr-follow` | Whether the `follow-pr` skill is listed | `crates/agent-engine/src/brief.rs` through `skill_flag` | The skill is hidden | The skill's entry in the skill list | Yes (`brief.rs` tests the flag off) | Not determined: the skill and its watcher were not read, so its maturity is unknown. It is the only skill with a flag, through a one-line special case. If the watcher works, make it permanent and delete `skill_flag` |

### Remove the feature (1)

| Flag | Gates | Read in | When OFF | Cost when ON | Tests on/off | Why remove |
| --- | --- | --- | --- | --- | --- | --- |
| `experimental-memory-scorecard` | One request in 20 runs without recalled memory so `/memory score` can compare | `crates/app/src/features/memory/mod.rs`, `memory/score.rs` | Every request gets its memories; nothing is recorded | A store update on every request to count it, and a deliberately worse answer on one request in 20 | No test names the flag | It is an A/B experiment run on the user's real work, on by default, in a product with no users to produce a meaningful sample. The question it asks belongs in an offline evaluation. Until removed it is the one flag that makes the product worse on purpose |

## 4. Cross-cutting recommendations

**Default for experimental flags: on (decided 2026-11-17).** This document
first proposed "experimental means off until turned on". The owner decided the
opposite: an experimental feature ships on, with an owner and an exit
criterion, and is turned off or promoted from there. `OPT_IN` stays, holding
only `experimental-code-mode`. What remains of this recommendation is the
exit criteria below: "experimental" should be a short stay, not a label.

**Reject unknown names.** `Features::layered` should return an error naming
the file and the key, as it already does for a non-boolean value. Two things
must happen first:

1. Give the backend folder switches a declared mechanism, or drop them, and
   remove the hyphen-to-underscore copy. Done 2026-11-17: see "Backend feature
   switches" in section 1.
2. Decide what a removed flag's name does in an existing settings file. With
   no users, an error is acceptable; otherwise keep a short list of retired
   names that are ignored with a notice (`Settings.notices` exists).

**Make "experimental" and "security" fields, not conventions.** `Flag` should
carry them: `Flag { name, about, stage, security }`. Today `experimental` is
`name.starts_with("experimental-")` and security is a separate list, so
graduating a flag means renaming it, which breaks every settings file that
mentions it. With a typed field the name stays stable. A flag enum instead of
`&str` constants would also make `on("typo")` a compile error inside the code
base, and would catch a flag constant that nothing reads (`SUGGESTED_RULES` is read, by `FeatureName::SuggestedRules` in `crates/app/src/api/features/mod.rs`).

**Read flags in one place per lifetime.** The backend's startup `off` set, the
per-request reloads in `memory/team.rs` and `code_map/mod.rs`, and the
terminal's `turned_off` copy can disagree within one session. After the
removals few readers are left; they should all take a `Features` value loaded
for the run.

**Order of removals.**

1. `budget`: the OFF state misleads about spending. One line in `job.rs`.
2. `experimental-memory-scorecard`: remove the feature and its hold-out.
3. The six web flags and `experimental-output-cleanup`: `WebReading` shrinks
   to nothing or to one field, `page_flag_tests.rs` goes away. Largest
   reduction in doubled paths for the least risk.
4. The command gates: `branches`, `suggested-rules`,
   `experimental-learning-check`, `experimental-memory-seed`.
5. The memory format flags (`experimental-memory-anchors`,
   `experimental-structured-learnings`), together, with `team-memory`.
6. `redact-secrets`, `destructive-snapshot`, `experimental-session-claims`,
   `experimental-escalation`, `project-criteria`: each is one line in
   `job.rs` and one `policy` field.
7. Move the six settings out of `features` (sandbox and dependency guard
   keeping the security layering rule).
8. Add the typed `Flag` fields, then turn on rejection of unknown names. (This
   step first also flipped the "Keep as experimental" section to default off;
   the 2026-11-17 rule drops that.)

Steps 1 to 6 each delete code and tests; none of the steps changes a default.

## Owners and exit criteria (26)

Every flag whose name starts with `experimental-`, as the code has them today.
Each ships on except `experimental-code-mode`. Owner: the owner, for all
of them (see the top of this document). The default exit criterion is:
promote to permanent or remove after it has run in daily use without a
reported problem, and review by 2027-02-17 (each row starts with that date).
Where the backlog above already names a better end, the row
says so.

| Flag | Default | Owner | Exit criterion |
| --- | --- | --- | --- |
| `experimental-sandbox` | On | The owner | Review by 2027-02-17. Becomes the security setting `sandbox.enabled`, keeping the rule that a project can only turn it on ("Keep as a user setting") |
| `experimental-dependency-guard` | On | The owner | Review by 2027-02-17. Becomes a security setting beside the sandbox, with the same project rule ("Keep as a user setting") |
| `experimental-escalation` | On | The owner | Review by 2027-02-17. Promote to permanent or remove after it has run in daily use without a reported problem; `advisorModel` and `escalationModel` stay its switches |
| `experimental-follow-through` | On | The owner | Review by 2027-02-17. Promote to permanent or remove after it has run in daily use without a reported wrong nudge |
| `experimental-affected-tests` | On | The owner | Review by 2027-02-17. Promote to permanent or remove after it has run in daily use without a reported problem, including on a large project |
| `experimental-pr-follow` | On | The owner | Review by 2027-02-17. Promote to permanent (and delete `skill_flag`) or remove after the `follow-pr` watcher has run in daily use without a reported problem |
| `experimental-session-claims` | On | The owner | Review by 2027-02-17. Promote to permanent or remove after it has run in daily use without a reported problem |
| `experimental-load-on-call` | On | The owner | Review by 2027-02-17. Promote to permanent or remove after it has run in daily use without a reported problem |
| `experimental-output-cleanup` | On | The owner | Review by 2027-02-17. Promote to permanent or remove after it has run in daily use without a reported problem |
| `experimental-output-filters` | On | The owner | Review by 2027-02-17. Promote to permanent or remove after it has run in daily use without a reported case of a hidden line that mattered |
| `experimental-output-savings` | On | The owner | Review by 2027-02-17. Promoted or removed together with `experimental-output-filters`, which it measures |
| `experimental-learning` | On | The owner | Review by 2027-02-17. Becomes `chores.learn` (section 5) |
| `experimental-web-main-content` | On | The owner | Review by 2027-02-17. Promote to permanent or remove after it has run in daily use without a reported problem |
| `experimental-web-cleanup` | On | The owner | Review by 2027-02-17. Promote to permanent or remove after it has run in daily use without a reported problem |
| `experimental-web-links` | On | The owner | Review by 2027-02-17. Promote to permanent or remove after it has run in daily use without a reported problem |
| `experimental-web-small-pages-direct` | On | The owner | Review by 2027-02-17. Promote to permanent or remove after it has run in daily use without a reported problem |
| `experimental-web-formats` | On | The owner | Review by 2027-02-17. Promote to permanent or remove after it has run in daily use without a reported problem |
| `experimental-web-empty-page-error` | On | The owner | Review by 2027-02-17. Promote to permanent or remove after it has run in daily use without a reported problem |
| `experimental-code-mode` | Off | The owner | Review by 2027-02-17. Promote to a permanent setting or remove after it has run in daily use without a reported problem (merged with `experimental-code-mode-only` on 2026-11-17) |
| `experimental-memory-anchors` | On | The owner | Review by 2027-02-17. Promote to permanent or remove, together with `experimental-structured-learnings` (one memory format), after it has run in daily use without a reported problem |
| `experimental-structured-learnings` | On | The owner | Review by 2027-02-17. Promote to permanent or remove, together with `experimental-memory-anchors` |
| `experimental-learning-check` | On | The owner | Review by 2027-02-17. Promote to permanent or remove after it has run in daily use without a reported problem |
| `experimental-memory-seed` | On | The owner | Review by 2027-02-17. Promote to permanent or remove after it has run in daily use without a reported problem |
| `experimental-memory-scorecard` | On | The owner | Review by 2027-02-17. Removed once an offline evaluation (`<command> eval`) answers whether memory helps ("Remove the feature") |
| `experimental-code-map` | On | The owner | Review by 2027-02-17. Promote to permanent or remove after it has run in daily use without a reported problem |
| `experimental-blast-radius` | On | The owner | Review by 2027-02-17. Promote to permanent or remove after it has run in daily use without a reported problem |

## 5. Per-turn chores

After every finished run the terminal (`App::after_run` in
`crates/cli/src/tui/chores.rs`) starts background runs through
`crates/app/src/client/chores.rs` and `crates/app/src/client/session.rs`. The engine handles
them in `Job::chore` (`crates/agent-engine/src/engine/phases/load_session.rs`), which
dispatches to `crates/agent-engine/src/chores/`.

Facts that hold for all of them, from `Job::chore`:

- **Model:** the session's own model (`self.request.model.id`), at low
  effort. The small model is not used: a chore returns from `Job::run` before
  settings are loaded and before the small model is picked.
- **Budget:** the meter is built with `budget: None`, so chore spending is
  not stopped by `/budget`. Whether it is added to the session's displayed
  spending was not traced.
- **Flags:** none is read in the engine. Only `learn` is gated, in the
  terminal.

| Chore | When | Input sent | Output cap | Gate today |
| --- | --- | --- | --- | --- |
| title | After a run while the conversation has no title | The first prompt only | 200 tokens | None |
| suggest | After every run | The whole conversation plus the suggest prompt | 60 tokens | None; the result is shown only if the input box is empty and nothing is running |
| heads-up | After every run | The whole conversation (trailing tool calls dropped) plus the heads-up prompt | 1,500 tokens | None |
| learn | After a run, when at least two correction signals were collected | The whole conversation plus the signals | 600 tokens | `experimental-learning`, checked in the terminal |

So a normal turn costs two extra calls on the main model that each resend the
whole conversation (suggest and heads-up), plus title once and learn
sometimes. Whether these calls hit the prompt cache was not determined; they
use a different system prompt (`chores`) from the main run, which suggests
they do not share its cached prefix.

`recap` (on returning to a conversation), `side` and `rename` are also chores
but run on demand, not per turn.

A setting to turn each off could look like this, read by the terminal before
it starts the run and by the engine so other clients get the same answer:

```json
{
  "chores": {
    "title": true,
    "suggest": false,
    "headsUp": false,
    "learn": true,
    "model": "small"
  }
}
```

- One boolean per chore replaces `experimental-learning` and gives the other
  three the switch they lack.
- `chores.model` (`"small"`, `"session"` or a model name) addresses the larger
  cost: today a session on the most expensive model pays that model's price
  for a 60-token suggestion. `smallModel` already exists in settings and is
  already used for the web reader and follow-through.
- These are owner preferences: a project layer should not be able to turn
  them on.
- Suggested defaults, for the tech lead to confirm: title on, the rest off
  until their cost per turn has been measured.
