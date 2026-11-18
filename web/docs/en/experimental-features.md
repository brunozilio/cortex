---
title: Feature flags
description: Every feature flag {{NAME}} knows, what each one changes, and how to turn them on or off with /features, /config or {{COMMAND}} config.
---

{{NAME}} ships a set of named features that you can switch on or off one by one: safety guards, ways of shaping command and web output, reviewers that check the agent's work, memory helpers and more. Each one is a **feature flag** with a fixed name such as `budget` or `experimental-sandbox`.

> [!IMPORTANT]
> Every flag is **on** until you turn it off, including the ones whose name starts with `experimental-`. A flag you never touched is on, [`experimental-code-mode`](#experimental-code-mode) included, which makes REPL the agent's only tool.

## Turn a flag on or off

### From the chat

Type `/features` to list every flag with its state and a one-line description:

```text
Features (/features <name> on|off):
  on   experimental-sandbox
       Confine Bash and Monitor commands to the project's folders (macOS; Linux with Landlock)
  on   experimental-output-cleanup
       Bash runs commands without colours and drops escape codes, redrawn progress and repeated lines
  ...
```

Switch one with its exact name followed by `on` or `off`:

```text
/features experimental-follow-through off
/features budget on
```

The change is saved in your user settings file, `~/{{DIR}}/settings.json`, and the chat answers `Saved in your settings; new runs follow it.` A run already going keeps the flags it started with; the next prompt uses the new state.

`/features` refuses a name it does not know (`<name> is not a feature; /features lists them`) and any other form of argument (`Use /features to list them, or /features <name> on|off.`).

A flag that cannot work on your system is still listed, marked `(not on this system)`. Today that applies only to `experimental-sandbox` on platforms without the sandbox.

`/config` (also `/settings`) opens the settings picker; its **Features** entry shows the same list.

### From the command line

`{{COMMAND}} config` edits settings files without opening the chat. Flags live under the `features` key, so use the dotted path `features.<name>`:

```bash
{{COMMAND}} config set features.experimental-sandbox false
{{COMMAND}} config get features.experimental-sandbox
{{COMMAND}} config unset features.experimental-sandbox
```

Without `--project` the command writes your user file, `~/{{DIR}}/settings.json`. With `--project` it writes this project's `{{DIR}}/settings.local.json`:

```bash
{{COMMAND}} config set --project features.budget false
```

`true` and `false` are read as JSON booleans. See [Settings](./settings) for the rest of the `config` command.

### In a settings file

Write the flags as an object of booleans under `features`:

```json
{
  "features": {
    "experimental-follow-through": false,
    "experimental-sandbox": true
  }
}
```

A value that is not `true` or `false` is an error: `features.<name> must be true or false`.

## Who can change a flag

{{NAME}} reads the settings files from your home folder down to the project folder (see [Settings](./settings)). For flags, the files fall into two groups:

| File | Who it belongs to | What it may do with a flag |
|---|---|---|
| `~/{{DIR}}/settings.json` | You | Turn any flag on or off |
| `{{DIR}}/settings.local.json` (in the project or a folder above it) | You | Turn any flag on or off |
| `~/{{DIR}}/managed-settings.json` | You | Turn any flag on or off; read last |
| `{{DIR}}/settings.json` checked into a project (or a folder above it) | The project | Only make things stricter |

A checked-in project file can only make things stricter:

- it can turn an ordinary flag **off**, but cannot turn back on a flag you turned off;
- it can turn a **security** flag **on**, but cannot turn one off.

The security flags are `experimental-sandbox`, `redact-secrets` and `experimental-dependency-guard`. So a repository can require the sandbox for everyone who works in it, while a stray `"redact-secrets": false` in a cloned repository has no effect.

Files are read in order and a later file you own wins. `/features` writes `~/{{DIR}}/settings.json`, which is read first, so a `settings.local.json` or `managed-settings.json` that sets the same flag still decides.

> [!TIP]
> To turn a flag off for one project only, use `{{COMMAND}} config set --project features.<name> false`. It writes the project's `settings.local.json`, which is yours and not checked in.

## Stable and experimental flags

A flag whose name starts with `experimental-` is experimental: its behaviour may still change or be removed. The other flags are stable. Both kinds start on.

Experimental features ship on so that they get used. Each one is on its way to one of two ends: once it has run in daily use without a reported problem it becomes a permanent part of {{NAME}} (its flag goes away, or it turns into an ordinary setting), or it is removed. Until then you can turn any of them off with `/features <name> off`.
## Flags at a glance

| Flag | Area | Security | What it does |
|---|---|---|---|
| `experimental-sandbox` | Safety | Yes | Confines Bash and Monitor commands to the project's folders (macOS; Linux with Landlock) |
| `redact-secrets` | Safety | Yes | Hides tokens, keys and `.env` values in tool output |
| `destructive-snapshot` | Safety | | Keeps files before `rm -r`, `git clean`, `git reset --hard` and the like so `/rewind` can bring them back |
| `experimental-dependency-guard` | Safety | Yes | Asks you before installing a missing, brand-new or look-alike package |
| `suggested-rules` | Permissions | | `/rules` offers permission rules from answers you keep giving |
| `experimental-output-cleanup` | Command output | | Strips colours, escape codes, redrawn progress and repeated lines |
| `experimental-output-filters` | Command output | | Hides passing tests, build steps and progress from common tools |
| `experimental-output-savings` | Command output | | `/gain` shows how much shorter command output became |
| `experimental-web-main-content` | Web | | WebFetch puts a page's main content first |
| `experimental-web-cleanup` | Web | | WebFetch drops hidden parts, buttons and runs of menu links |
| `experimental-web-links` | Web | | WebFetch numbers links and lists their addresses at the end |
| `experimental-web-small-pages-direct` | Web | | WebFetch hands short pages straight to the agent |
| `experimental-web-formats` | Web | | WebFetch asks for Markdown and compacts JSON and feeds |
| `experimental-web-empty-page-error` | Web | | WebFetch reports a page with almost no text as an error |
| `budget` | Cost | | `/budget` stops a session's model calls at a spending cap |
| `cache-warning` | Cost | | The status line counts down the prompt cache's last two minutes |
| `branches` | Git and sessions | | `/branch <n>` starts a new conversation from before an earlier prompt |
| `git-offer` | Git and sessions | | Offers to initialize git in a folder that is not a repository |
| `experimental-session-claims` | Git and sessions | | Warns the agent when another session edited the same file |
| `experimental-pr-follow` | Git and sessions | | Enables the `follow-pr` skill |
| `auto-review` | Review | | A read-only reviewer checks the changes before the run ends |
| `project-criteria` | Review | | Judges runs against `{{DIR}}/verification.md` |
| `experimental-follow-through` | Review | | Sends the agent back when it ends promising more work |
| `experimental-affected-tests` | Review | | Runs the tests covering an edited file in the background |
| `experimental-escalation` | Review | | Sends a stuck agent to the Advisor or to `escalationModel` |
| `experimental-code-mode` | Tools | | REPL becomes the agent's only tool, and it scripts every other tool in JavaScript |
| `experimental-load-on-call` | Tools | | A tool held back for search that the agent calls by name is loaded and run |
| `experimental-learning` | Learning and memory | | Offers one lesson to keep after a run where you corrected the agent |
| `team-memory` | Learning and memory | | Recalls memories checked into `{{DIR}}/memory` |
| `experimental-memory-anchors` | Learning and memory | | Memories name their files and show up when those files are touched |
| `experimental-structured-learnings` | Learning and memory | | Decisions are kept as decided, ruled out and watch out |
| `experimental-learning-check` | Learning and memory | | `/check` compares the branch's changes with the memories about them |
| `experimental-memory-seed` | Learning and memory | | `/memory seed` offers what you asked for in several sessions |
| `experimental-memory-scorecard` | Learning and memory | | One request in 20 runs without memory, for `/memory score` |
| `experimental-code-map` | Code intelligence | | Points each prompt at the matching code from a tree-sitter index |
| `experimental-blast-radius` | Code intelligence | | Tells the agent which other files call what an edit changed |
| `memory-search` | Built-in features | | The `memory_search` tool the agent uses to search memory |
| `schedules` | Built-in features | | `/schedules`, `/loop` and scheduled prompts |
| `plugins` | Built-in features | | Installing, updating, listing and removing plugins |
| `trace` | Built-in features | | The data behind the `/trace` and `/console` pages |
| `channels` | Built-in features | | MCP servers that stay open and send messages to the agent |
| `declared-providers` | Built-in features | | Loads the model providers that settings files declare |

The sections below describe each flag in detail.

## Safety

### `experimental-sandbox`

> [!EXPERIMENTAL]
> Security flag. Default: on. Available only where the sandbox is supported (macOS; Linux 5.13 or later with Landlock); elsewhere `/features` shows it as `(not on this system)`.

Confines the commands the agent runs with **Bash** and **Monitor** to the project's folders, using the `sandbox` settings. Commands the agent starts to find a file's tests (see [`experimental-affected-tests`](#experimental-affected-tests)) are confined the same way. When the flag is off, commands run without the sandbox.

See [Sandbox and safety](./sandbox-and-safety) for what the sandbox allows, how to configure it and how the agent asks to leave it. `{{COMMAND}} doctor` reports whether the sandbox is on and available.

### `redact-secrets`

> [!NOTE]
> Security flag. Default: on.

Hides secrets in what tools return before the model and the session log see them. It replaces with `[redacted]`:

- tokens that start with `sk-`, `sk_`, `ghp_`, `gho_`, `ghs_`, `ghu_`, `github_pat_`, `glpat-` (20 characters or more), or `xoxb-`, `xoxp-`, `xoxa-`, `xoxs-` (10 or more);
- AWS access key ids (`AKIA…` or `ASIA…`, 20 characters) and JSON Web Tokens (`eyJ…` with three parts);
- the value after a key whose name ends in `password`, `passwd`, `secret`, `token`, `api_key`, `apikey` or `api-key` (as in `API_KEY=…` or `token: …`), and the word after `Bearer`;
- every value of 8 characters or more, without spaces, found in the project's `.env` and `.env.*` files (but not `*.example` or `*.sample`).

Redaction applies to tool results, to the live progress lines of running commands, to the tool summaries shown in the chat, and to the full output files kept by [`experimental-output-filters`](#experimental-output-filters). Sub-agents inherit it.

### `destructive-snapshot`

> [!NOTE]
> Default: on.

Before Bash runs a command that destroys files, {{NAME}} keeps a copy of those files so that [`/rewind`](./sessions) can bring them back. The commands it recognises are:

- `rm` with paths (all files under them, recursively);
- `git clean` (untracked files; also ignored files with `-x` or `-X`);
- `git reset --hard`, `git checkout .` or `git checkout -- …`, `git restore .` or `git restore -- …`, `git stash -u`/`--include-untracked`/`-a`/`--all` (untracked files in the repository).

At most 5,000 files and 200 MB are kept per command. When more had to be left out, the tool result says `(before this command N files were kept for /rewind; M more were too many or too large to keep)`.

### `experimental-dependency-guard`

> [!EXPERIMENTAL]
> Security flag. Default: on.

Before a Bash command installs packages, {{NAME}} looks each one up in its registry and asks you first when one looks wrong. It recognises:

| Ecosystem | Commands |
|---|---|
| npm | `npm`, `pnpm` or `bun` with `add`, `install` or `i`; `yarn add` |
| crates.io | `cargo add` |
| PyPI | `pip`, `pip3` or `pipx install`; `uv pip install`; `uv add`; `poetry add`; `python -m pip install` |

Local paths, URLs and requirement files (`-r requirements.txt`) are not looked up. A package raises a concern when:

- its registry answers that it does not exist (`<name> is not published in its registry: the name may be made up`);
- its name is one letter away from, or swaps two letters of, a popular package (`<name> looks like the popular <known> with a typo`);
- it was first published less than 30 days ago (`<name> was first published N days ago`).

The permission prompt then starts with `Check the package first: …`. In a run that cannot ask (such as [headless mode](./headless)), the command is refused with `Not run: … Ask the user before installing it.` A lookup that fails for another reason raises nothing.

## Permissions

### `suggested-rules`

> [!NOTE]
> Default: on.

Enables `/rules`, which reads the permission answers you gave in this project's recent sessions and suggests rules for the ones you keep repeating:

- an **allow** rule after you allowed the same request 5 times and never denied it;
- a **deny** rule after you denied it 3 times and never allowed it.

Rules you already have are not suggested.

```text
/rules
Your answers suggest these rules (/rules keep <n> saves one):
  1. allow Bash(cargo test:*)  (you allowed it 7 times)
  2. deny Bash(git push:*)  (you denied it 3 times)

/rules keep 1
Kept Bash(cargo test:*) as a allow rule in {{DIR}}/settings.local.json.
```

Turning the flag off also stops [`/learn keep`](./learning) from saving a rule or an instruction line, because both go through the same feature. It takes effect at once, without restarting: with the flag off, `/rules` and `/learn keep` answer `suggested_rules is turned off (/features suggested-rules on)`.

## Command output

These three flags change the output of the commands the agent runs with Bash before it reaches the model.

### `experimental-output-cleanup`

> [!EXPERIMENTAL]
> Default: on.

Commands run with `NO_COLOR=1`, `CLICOLOR=0`, `CARGO_TERM_COLOR=never`, `CARGO_TERM_PROGRESS_WHEN=never` and `PY_COLORS=0`. Their output then loses terminal escape codes and backspaces, keeps only the last redraw of a line rewritten with carriage returns, collapses runs of blank lines into one, and shows a line repeated 3 or more times once, as `<line> (×N)`.

### `experimental-output-filters`

> [!EXPERIMENTAL]
> Default: on.

Hides noise from well-known tools and keeps the whole output in a file the agent can read:

| Filter | Applies to | Hides |
|---|---|---|
| `cargo` | `cargo …` | `Compiling`, `Checking` and other build steps, `test … ok` lines; groups repeated warnings |
| `git` | `git …` | Progress lines (`Counting objects:`…) and `(use "git …")` hints |
| `javascript` | `npm`, `pnpm`, `yarn`, `bun`, `jest`, `vitest` | Passing test lines (starting `✓`, `√`, `✔` or `PASS`), `Progress: resolved` lines and lines of `+` signs |
| `python` | `pytest`, `py.test`, `pip`, `pip3`, `uv pip` | `PASSED` test lines, progress lines made only of dots, and pip steps (`Collecting`, `Downloading`, `Using cached`, `Requirement already satisfied:`…) |
| `go test` | `go test` | `=== RUN`, `=== PAUSE`, `=== CONT`, `--- PASS:` |

A filter is used only when it shortens the output by at least 400 characters. The shortened output ends with a footer such as:

```text
[37 lines of cargo noise hidden (passing tests, progress, build steps, repeated warnings); the whole output is in /…/bash-1234-1.txt]
```

Failures, errors and summaries are never hidden.

### `experimental-output-savings`

> [!EXPERIMENTAL]
> Default: on.

Measures how much shorter each command's output became through cleanup and filtering, and enables `/gain` to show it. See [Budget and cost](./budget-and-cost#see-how-much-output-was-saved).

## Web

These flags change how **WebFetch** reads a page. See [Sandbox and safety](./sandbox-and-safety) for how fetched pages are kept apart from instructions.

### `experimental-web-main-content`

> [!EXPERIMENTAL]
> Default: on.

When a page has a main content area, WebFetch writes it first and puts the rest (menus, header, footer) after a `--- around the main content ---` line. Off, the page reads in its own order.

### `experimental-web-cleanup`

> [!EXPERIMENTAL]
> Default: on.

Drops hidden elements and buttons, link labels repeated on every item of a list, and long runs of menu links (keeping the first 8 of a run). Off, they all stay.

### `experimental-web-links`

> [!EXPERIMENTAL]
> Default: on.

Numbers the links in the text as `[1]`, `[2]`… and lists each address once in a `Links` section at the end, writing same-site addresses as paths. When a long page is clipped, the list keeps only the links the remaining text refers to. Off, each address is written in brackets beside its text.

### `experimental-web-small-pages-direct`

> [!EXPERIMENTAL]
> Default: on.

A page of 6,000 characters or less is handed to the agent whole, fenced as untrusted content. Longer pages, and every page when the flag is off, go to a separate reader model (`smallModel` when set, otherwise the session's model) that answers what the agent was looking for.

### `experimental-web-formats`

> [!EXPERIMENTAL]
> Default: on.

WebFetch asks for `text/markdown` first (then plain text, HTML and JSON), compacts JSON to one line per record, and reads RSS and Atom feeds as a list of entries. Off, it sends no `Accept` header and treats JSON as plain text.

### `experimental-web-empty-page-error`

> [!EXPERIMENTAL]
> Default: on.

When an HTML page yields no readable text, or under 400 characters of text out of more than 10,000 characters of markup, WebFetch returns an error saying the page is probably built by JavaScript or behind a consent wall, a login or a bot check, instead of an almost empty page.

## Cost

### `budget`

> [!NOTE]
> Default: on.

Lets `/budget <dollars>` cap what a session spends: once the session has spent the budget, the next model call is refused. With the flag off, runs ignore the cap even if you set one. See [Budget and cost](./budget-and-cost#cap-a-sessions-spending).

### `cache-warning`

> [!NOTE]
> Default: on.

Adds a prompt-cache countdown to the status line while the agent is idle. See [Budget and cost](./budget-and-cost#keep-the-prompt-cache-warm).

## Git and sessions

### `branches`

> [!NOTE]
> Default: on.

Enables `/branch <n>`, which starts a new conversation from before the n-th prompt of this one. Off, the command answers `Branches are turned off in your settings (/features branches on).` See [Branches and git](./branches-and-git#branch-from-an-earlier-prompt).

### `git-offer`

> [!NOTE]
> Default: on.

On the first message of a new conversation in a folder that is not a git repository, {{NAME}} asks whether to initialize one. Answering **No, and don't offer again** turns this flag off. See [Branches and git](./branches-and-git#initialize-git-in-a-new-folder).

### `experimental-session-claims`

> [!EXPERIMENTAL]
> Default: on.

When the agent edits or writes a file that another live session in the same folder changed less than 10 minutes ago, the tool result warns it to re-read the file and to tell you if you both need it. See [Branches and git](./branches-and-git#several-sessions-in-one-folder).

### `experimental-pr-follow`

> [!EXPERIMENTAL]
> Default: on.

Makes the built-in `follow-pr` skill available. Off, the skill is removed from the list the agent sees. See [Branches and git](./branches-and-git#follow-a-pull-request-to-merge).

## Review

### `auto-review`

> [!NOTE]
> Default: on.

When a run changed files, an independent read-only reviewer checks the changes, and the documentation that describes them, before the run ends. Serious findings send the agent back once. See [Verification and review](./verification-and-review#automatic-code-review).

### `project-criteria`

> [!NOTE]
> Default: on.

When you set no criteria of your own, every run is judged against the project's checked-in `{{DIR}}/verification.md`. See [Verification and review](./verification-and-review#project-criteria).

### `experimental-follow-through`

> [!EXPERIMENTAL]
> Default: on.

When the agent's last answer ends by promising more work ("now I'll run the tests"), a small model checks whether the work was left undone and, if so, sends the agent back to do it, at most twice per run. See [Verification and review](./verification-and-review#follow-through).

### `experimental-affected-tests`

> [!EXPERIMENTAL]
> Default: on.

After the agent edits or writes a file, the tests that cover it start in the background and their result reaches the agent when they end. See [Verification and review](./verification-and-review#affected-tests).

### `experimental-escalation`

> [!EXPERIMENTAL]
> Default: on.

When the agent keeps failing the same call, it is told to ask the Advisor (when `advisorModel` is set) or the next turns run on `escalationModel`. See [Verification and review](./verification-and-review#escalation-when-the-agent-is-stuck).

## Tools

### `experimental-code-mode`

> [!EXPERIMENTAL]
> Default: on. Turn it off with `/features experimental-code-mode off` to give the agent its tools back.

Makes **REPL** the agent's only tool, besides the plan-mode and worktree switches; every other tool, asking you questions included, is described inside REPL's description as a typed function and is called from scripts. Tools held back for search (with many MCP tools) are found from a script with `searchTools("words")`, and `describeTool(name)` gives one tool's declaration. Instead of calling tools one turn at a time, the agent writes a JavaScript script in which every other tool is a function (`await Grep({ pattern: "TODO" })`), loops and filters in code, and reads only what the script returns. Searching many files or making the same edit in many places then takes one turn, and the files it only needed to filter never enter the conversation.

Each call a script makes is an ordinary tool call: it follows your permission mode, rules and hooks, asks you exactly as the tool would, and shows up in the chat inside the script's own **REPL** block, the way a sub-agent's calls do. A call refused by a rule fails inside the script and is listed after its output; a call you answer No to stops the script there. What a script declares stays for the next one in the same session until {{NAME}} exits. A script still running after about 30 seconds (a build, a test suite) goes on in the background, in the same context, while the agent works on; its result reaches the agent like a background agent's, at the latest before the turn ends. Turn the flag off to give the agent its tools back. It replaces `experimental-code-mode-only`, merged into it on 2026-11-17; a setting that still names the old flag is ignored.

### `experimental-load-on-call`

> [!EXPERIMENTAL]
> Default: on.

With many MCP tools, most are held back behind ToolSearch and the agent loads them before calling them. With this flag, when the agent calls a held-back tool by name without loading it, the tool is loaded and run right away instead of failing with a request to use ToolSearch. From then on it is offered like a tool the search loaded. Permissions, rules and hooks apply as before. Off, the call is refused until the agent loads the tool.

## Learning and memory

### `experimental-learning`

> [!EXPERIMENTAL]
> Default: on.

After a run in which you corrected the agent at least twice, {{NAME}} proposes one lesson to keep (a memory, a skill, a permission rule or an instruction line), which `/learn keep` saves. See [Learning](./learning).

### `team-memory`

> [!NOTE]
> Default: on.

Recalls the memories checked into the project's `{{DIR}}/memory` folder along with your own. `/memory share <id>` writes one of yours there. See [Memory](./memory#team-memory).

### `experimental-memory-anchors`

> [!EXPERIMENTAL]
> Default: on.

Memories can name the files they are about; when the agent reads, edits or writes one of those files, the memories about it are shown to it. See [Memory](./memory#memories-tied-to-files).

### `experimental-structured-learnings`

> [!EXPERIMENTAL]
> Default: on.

Decisions are kept as what was decided, what was tried and ruled out, and what to watch out for. See [Learning](./learning#structured-decisions).

### `experimental-learning-check`

> [!EXPERIMENTAL]
> Default: on.

Enables `/check`, which has the agent compare the branch's changes with the memories about the files they touch. See [Learning](./learning#check-a-branch-against-memory).

### `experimental-memory-seed`

> [!EXPERIMENTAL]
> Default: on.

Enables `/memory seed`, which offers sentences you wrote in two or more past sessions as memories to keep. See [Memory](./memory#seed-memory-from-past-sessions).

### `experimental-memory-scorecard`

> [!EXPERIMENTAL]
> Default: on.

One request in 20 runs without recalled memory, and `/memory score` compares corrections and cost with and without it. See [Memory](./memory#measure-whether-memory-helps).

## Code intelligence

### `experimental-code-map`

> [!EXPERIMENTAL]
> Default: on.

Indexes the repository's functions, types and callers with tree-sitter and adds to each prompt a short list of the files and symbols that match it. See [Code intelligence](./code-intelligence#code-map).

### `experimental-blast-radius`

> [!EXPERIMENTAL]
> Default: on.

After an edit, asks the language server which other files use the symbols the edit changed and tells the agent. See [Code intelligence](./code-intelligence#blast-radius).

## Turning off a whole built-in feature

Some flags switch a whole built-in feature rather than one behaviour of it. They are ordinary flags: `/features` lists them, `/features <name> on|off` switches them at once, and they are on until you turn them off.

| Flag | What it turns off |
|---|---|
| `memory-search` | The `memory_search` tool the agent uses to search memory; recall with each prompt stays |
| `schedules` | `/schedules`, `/loop` and scheduled prompts firing |
| `plugins` | Installing, updating, listing and removing plugins |
| `trace` | The data behind the `/trace` and `/console` pages |
| `channels` | MCP servers that stay open for the session and send messages to the agent |
| `declared-providers` | Loading the model providers that files in your settings folder and the project declare |
| `suggested-rules` | `/rules`, and saving rules and instruction lines from `/learn keep` |
| `experimental-code-map` | The code map |
| `experimental-output-savings` | `/gain` |

A request to a feature that is off answers with its name and the command that turns it back on, for example `trace is turned off (/features trace on)`.

Only these names switch a feature. A name `/features` does not list does nothing in the `features` object, whatever it resembles: `"code_map": false`, `"output-savings": false` or a misspelled flag turn nothing off. Five built-in features cannot be turned off at all, because {{NAME}} does not work without them: the feature flags themselves (`/features`), project memory, the session list, background commands, and the workspace (the git offer, `@` file mentions and change listings). To stop only the git offer, turn off [`git-offer`](#git-offer).

> [!NOTE]
> Earlier versions accepted any built-in feature's folder name here, with hyphens or underscores, including `workspace`, `sessions`, `shell` and `flags`. Those four are now ignored; `code-map` and `output-savings` are spelled `experimental-code-map` and `experimental-output-savings`.
