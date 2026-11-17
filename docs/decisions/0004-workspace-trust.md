# 0004 Workspace trust

Status: Accepted, 2026-11-16; implemented 2026-11-17 (see "As implemented")

## Context

Opening the app in a cloned repository and sending the first prompt can execute code from that repository, with no step where the user agrees.

- The `hooks` key is accepted from the project's versioned `settings.json`; unlike `permissions.allow` it has no owner-only filter (`crates/agent-engine/src/prompt/settings.rs`).
- Script hooks in the `hooks/` folder of the project's `.<command>/` directory, and of every ancestor directory, are registered automatically (`crates/agent-engine/src/hooks/`).
- A file is treated as written by the owner if it is in the user's folder or if it is named `settings.local.json`. A repository that commits a file with that name gains allow rules, MCP approval, extra directories and language-server commands.
- Project script tools, MCP servers and declared providers each have their own, different approval rule.

The audit found no notion of trust anywhere in the terminal client, the backend or the client library. Combined with unrestricted writes (`0003-effects-based-permission-policy.md`), the model can also write these files itself and keep the privilege.

## Decision

Trust is a first-class, per-folder decision.

- The decision is stored in the user's data folder, never inside the project.
- Until a project is trusted, nothing from it that can execute or widen permissions is loaded: no hooks, no scripts, no `settings.local.json`, no MCP servers, no declared providers, no language-server commands. Instruction files and restricting rules (ask, deny) still apply.
- The TUI asks once per folder with a dialog. Print mode does not ask; it takes `--trust`.
- Folders that already have sessions are not trusted automatically. Past use is not consent to what the repository contains today.
- The policy engine consults trust as one of its inputs.

This is phase 6.1 of the plan.

## Consequences

- A hostile repository cannot run code or grant itself permissions before the user has agreed.
- The several ad hoc approval rules collapse into one question with one stored answer.

What gets worse:

- Behaviour changes for existing users: every folder asks once, including folders used daily.
- `-p` stops running project hooks unless `--trust` is passed, which breaks scripts and CI jobs that depend on them until they add the flag.
- Trust is all or nothing per folder. A user who wants a project's MCP servers but not its hooks has no finer switch.
- The trust record is keyed by folder, so moving or re-cloning a repository asks again, and a trusted folder stays trusted when its content changes (for example after a pull).

## As implemented

The rule lives in `crates/domain/src/safety/trust/` (pure), the gate in `crates/agent-engine/src/trust/`, the store in `crates/app/src/trust.rs`, the dialog and `/trust` in `crates/cli/src/tui/features/trust/`.

### Which folder is trusted

- The key is the trust root of the folder the session runs in: the nearest ancestor that contains `.git`, else the folder itself. Paths are canonical, so a symbolic link names the folder it points at. A git worktree the user made has its own `.git` and is its own root. A worktree the engine makes under `.<command>/worktrees/` for a run or a sub-agent inherits the trust of that run: the answer travels with the run, it is not looked up again.
- A folder is trusted when its trust root is in the store, exactly. Trusting `/a/repo` trusts `/a/repo/src`; it does not trust `/a`, nor `/a/repo/vendor/x` when that has its own `.git`.
- The user's home folder and a file-system root are refused as a trust root, with a message. A home folder kept in git is skipped when looking for the root, so a folder under it without its own repository is its own root.
- `~/.<command>/` is always trusted. A session whose folder is the home folder has nothing project-level to load and is never asked.
- A `.<command>/` folder above the trust root (for example `~/work/.<command>/` when the repository is `~/work/repo`) is never a source of hooks or owner settings, trusted or not. It is still read as a restricting layer. The same holds for a folder without a repository: only its own `.<command>/` counts, not its parents'.

### What waits for trust

One gate, `Gate::honours(file)`: a file is honoured when it is under `~/.<command>/`, or when the folder is trusted and the file is inside the trust root. Until then the engine skips:

- `hooks` in the project's `settings.json` and `settings.local.json`;
- `.<command>/hooks/*.js` in the folder and its ancestors up to the trust root;
- the project's `settings.local.json` as an owner layer: it is read like `settings.json`, so only deny and ask rules, feature restrictions and sandbox restrictions apply;
- project MCP servers (`mcp.json` and `mcp/*.js`): not started and not listed for approval;
- project declared providers, script tools, skills and commands written as scripts (the file is not evaluated, not even to read its description);
- `command`, `args` and `env` of `lspServers` (already dropped from `settings.json`; now also from an untrusted `settings.local.json`);
- `hooks`, `mcpServers`, `permissionMode` and `initialPrompt` of project agent definitions.

Still loaded, because they inform or restrict: instruction files, `.<command>/rules/*.md`, deny and ask rules, output styles, `verification.md`, Markdown skills and commands, an agent's instructions.

What was skipped is said once per settings load, as one notice built from a scan of the folder that evaluates no project code (`agent_engine::trust::contributions`). The same scan is the typed summary the dialog shows.

Trusted does not mean approved. A trusted project's MCP servers still need their per-server approval, its declared providers their id in `enabledProviders`, and its scripts get no file, process or network access unless an owner layer grants `scripts.*`. Those answer a different question.

### Where the answer is kept and how it travels

- `users/local/trusted-folders.json` in the data folder, a compact list of `{ "path", "trusted_at" }`, through `platform::store::Document`.
- `ClientCommand::Trust { path, trusted }` stores or removes it; `ClientCommand::TrustForThisProcess { path }` trusts without storing (`--trust`); `ClientCommand::TrustState { path }` returns a `TrustReport`: the root, the state, a refusal if any, and what the folder brings.
- The engine reads `Extensions.trust` from the request's host context. The handler builds that context from the store for every run; whatever a client put in the request is dropped (`crates/app/src/handler/sent.rs`, `crates/app/src/extensions.rs`).

### What the user sees

- The chat asks before the first prompt when the folder is untrusted and brings something: "Trust this folder", "Continue without trusting" (the default), "Exit". Continuing is remembered for the process. A folder that brings nothing does not ask.
- `/trust` shows the state and the summary; `/trust yes` and `/trust no` change it, from the next run on.
- `--print` never asks. It runs untrusted and prints one line to the error stream when items were skipped; `--trust` trusts for that invocation without storing. Where the folder cannot be trusted (the home folder), `--trust` is ignored with a line saying why and the run goes on untrusted.
- `doctor` has a `trust` line. `/hooks` and `/permissions` end with the notice, whose reason reads "folder not trusted (/trust)".
- "Don't ask again" in an untrusted folder still writes the rule to `settings.local.json`, and a notice says it holds for that run only until the folder is trusted.

### What it replaced

- The git-tracked heuristic of `prompt/project_trust.rs` (one `git ls-files` per file per settings load) is gone. Whether git tracks a file no longer matters.
- `trustProjectHooks` is still read from the user's settings without error for one version; it enables nothing and produces a notice that points at trusting the folder.
- The per-item approvals stay as they were, behind the gate.

### What got worse, measured against the above

- One question per folder, including folders used daily, and again after a move or a fresh clone.
- `-p` scripts and CI jobs lose project hooks, script tools, MCP servers and `settings.local.json` grants until they pass `--trust`.
- An untracked `settings.local.json` and untracked project hooks, which the stopgap honoured, now wait for trust too.
- In a folder without a repository, a `.<command>/` folder of a parent no longer grants or runs anything.
- A rule saved with "don't ask again" has no effect on later runs in an untrusted folder.
- Evals run in a scratch copy of the fixture, which is trusted only when the project they are run from is.
- Taking trust back does not undo what the process already loaded: a declared provider registered while the folder was trusted stays registered until the process ends. Hooks, tools, servers and settings are read again for every run and follow the change at once.
- While a folder is untrusted, every settings load scans it once more to build the notice (directory listings and JSON reads, no process started).
- The texts that name `/trust` and `--trust` are written in `domain` and the engine, below the terminal client that owns those commands.
