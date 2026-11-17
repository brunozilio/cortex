# 0004 Workspace trust

Status: Accepted, 2026-11-16

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
