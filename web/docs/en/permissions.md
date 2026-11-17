---
title: Permissions
description: Decide what the agent may do without asking, with permission modes, allow/ask/deny rules and extra folders.
---

Every tool call the agent makes goes through a permission check. Reading inside the project is always free; editing files, running commands, fetching pages and reading outside the project depend on the **permission mode** and on your **rules**. When neither settles it, {{NAME}} asks you.

## Answering a permission request

When the agent needs your approval, the panel shows the tool, what it wants to do, and up to three choices:

| Choice | Key | Effect |
|---|---|---|
| Yes | `y`, `1` | Runs this call once. |
| Yes, and don't ask again for `<rule>` in this project | `a`, `2` | Runs it and saves the suggested rule to `permissions.allow` in `{{DIR}}/settings.local.json`. |
| Yes, and don't ask again this session | `a`, `2` | Shown when no rule fits (edits, for example). Remembers the tool for edits, or the exact call for anything else, until the session ends. |
| No, and tell the agent what to do instead | `n`, Esc, or its number | Refuses. Picking it by number or with Enter lets you type a note the agent receives; Esc or `n` refuses without one. |

The "don't ask again" choice is not offered for actions that must be approved every time (see [Always asked](#actions-that-always-ask)).

Suggested rules name the kind of action, not its arguments:

| You were asked about | Suggested rule |
|---|---|
| `cargo test --workspace` | `Bash(cargo test:*)` |
| `gh issue create --title x` | `Bash(gh issue create:*)` |
| `python3 scripts/gen.py --all` | `Bash(python3 scripts/gen.py:*)` |
| `make` | `Bash(make)` |
| Fetching `https://docs.rs/serde/latest` | `WebFetch(domain:docs.rs)` |
| Reading `/home/ana/notes/todo.md` (outside the project) | `Read(//home/ana/notes/**)` |

Commands that cannot be described safely (two commands joined, `$(…)`, a script by path, `FOO=1 make`) get no suggested rule.

## Permission modes

| Mode | Name in `--mode` and `/mode` | Edits | Commands and other actions | Use it when |
|---|---|---|---|---|
| Ask | `ask` (also `default`) | Ask | Ask | You want to approve every change. |
| Edits | `edits` (also `acceptedits`, `accept-edits`) | Allowed | Ask | Default. You trust edits in the project, not commands. |
| Plan | `plan` | Refused | Refused | The agent only reads, searches and proposes a plan for you to approve. |
| Bypass | `bypass` (also `yolo`) | Allowed | Allowed, after a safety check | Unattended work in a disposable environment. |

In every mode, reading and searching inside the project, the [additional folders](#additional-directories), and read-only shell commands (`ls`, `git status`…) run without asking. Reading outside them asks, except in bypass mode.

How to switch:

- **Shift+Tab** in the chat cycles ask → edits → plan → bypass → ask.
- `/mode` opens a picker; `/mode plan` (or `/plan`) switches directly.
- `--mode <name>` starts the chat or a `-p` run in that mode.
- The last mode you picked is remembered for the next chat; `--mode` wins for that start.

> [!WARNING]
> In bypass mode `permissions.allow` and `permissions.ask` no longer matter: everything not denied runs. Before each action that is not a plain read, a safety check reviews it against the conversation and your instruction files and holds back anything it grades as too risky. Deny rules still refuse.

When a plan is ready, the agent asks you to approve it; approving switches the mode so it can carry the plan out. In `-p` runs a plan is printed and not carried out (run again with `--mode edits`).

## Permission rules

Rules live in the `permissions` key of your [settings files](./settings#settings-files):

```json
{
  "permissions": {
    "allow": ["Bash(cargo test:*)", "Edit(src/**)", "WebFetch(domain:docs.rs)"],
    "ask": ["Bash(git push:*)"],
    "deny": ["Bash(rm:*)", "Read(*.env)", "mcp__github__delete_repo"]
  }
}
```

- `allow` runs without asking. Only your own files can allow: allow rules in a checked-in `{{DIR}}/settings.json` are ignored.
- `ask` always asks, even when the mode would let the call through.
- `deny` refuses, in every mode, and tells the agent not to retry.

### Precedence

1. A **deny** rule that matches refuses the call.
2. In bypass mode, and for edits or commands in plan mode, the mode decides.
3. An **ask** rule that matches asks (even for reads).
4. An **allow** rule that matches runs the call.
5. Otherwise the mode decides.

Rules from all files, and from `--allowedTools`/`--disallowedTools`, form one list; their order does not matter. `/permissions` (also `/allowed-tools`) lists them as deny, ask, allow, followed by the extra folders.

### Rule syntax

A rule is a tool name, optionally followed by a specifier in parentheses: `Tool` or `Tool(specifier)`.

| Rule | Matches |
|---|---|
| `Bash` | Every command. |
| `Bash(git status)` | Exactly `git status` (extra spaces are ignored). `git status -s` does not match. |
| `Bash(git diff:*)` | `git diff` and anything starting with `git diff ` (word boundary: not `git difftool`). |
| `Bash(npm run * --silent)` | `*` matches anything: `npm run build --silent`. |
| `Edit` | Every edit. `Edit` rules also cover `Write` and `NotebookEdit`. |
| `Edit(src/**)` | Edits under `src/` in the project. |
| `Read(*.env)` | A file name anywhere in the project. `Read` rules also cover `Glob`, `Grep` and `LSP`. |
| `Read(//etc/hosts)` | An absolute path (two leading slashes). |
| `Read(~/notes/*.md)` | A path in your home folder. |
| `WebFetch(domain:docs.rs)` | Fetching pages on that host. |
| `mcp__jira` | Every tool of the `jira` MCP server. |
| `mcp__github__delete_repo` | One tool of one server. |
| `mcp__docs__resources(file:///a.md)` | Reading that resource of the `docs` server. |
| `Tool(subject)` | Any other tool: the specifier must equal the call's subject. |

#### Paths

Path specifiers match like `.gitignore`:

- `//path` is absolute, `~/path` starts at your home folder, `./path` starts at the project.
- A pattern with a `/` in it starts at the project (`src/**`, `config/prod.env`).
- A pattern without `/` matches that name at any depth in the project (`*.env`, `secrets`).
- `*` and `?` stay inside one folder; `**` crosses folders.
- A rule that matches a folder covers everything inside it.
- Deny and ask rules ignore case (`Read(.env)` also refuses `.ENV`); allow rules do not.
- A `Glob` or `Grep` over a folder is refused when a deny rule covers something inside it.

#### Commands

Shell rules read the command the way the shell will run it:

- **Compound commands** (`&&`, `;`, `|`): every part must be allowed. Read-only parts (`ls`, `cat`, `git status`…) and `cd` into a folder inside the project go along without a rule. One denied part refuses the whole command.
- **Hidden commands** (`$(…)`, backticks, `<(…)`, `bash -c '…'`, `eval`, `xargs`) and **redirections to files** are never allowed by an allow rule. When deny or ask rules with a specifier exist, such a command is asked about.
- **Deny rules see through disguises**: `sudo rm`, `env rm`, `VAR=1 rm`, `\rm`, `"rm"`, `/bin/rm`, `(rm x)`, `{ rm x; }` all match `Bash(rm:*)`.
- **Allow rules read the command as written**: `LD_PRELOAD=/tmp/x.so git status` does not match `Bash(git status:*)`.

#### Invalid rules

A rule without its closing `)`, with an empty specifier (`Bash()`), or that does not start with a tool name stops the run with an error naming the list, for example `permissions.deny: rule "Bash(rm" misses its )`.

## Actions that always ask

Some actions are asked about every time, whatever the mode and rules (except a deny rule, which refuses them), and cannot be remembered:

- Pushing to the default branch, or force-pushing, deleting or rewriting remote history.
- `cargo publish`, `npm publish`, `pnpm publish`, `yarn publish`.
- `gh pr merge`, `gh release create|delete|edit`, `gh api`.
- `docker push`, `podman push`.
- Fetching an address that never appeared in the conversation, once the session has read content from outside (web pages, MCP tools).
- Running a command outside the sandbox, and installing a suspicious package. See [Sandbox and safety](./sandbox-and-safety).

Commands that would print secrets (`env`, `printenv`, `cat .env`, reading SSH keys…) are refused outright.

## Command-line flags

| Flag | Effect |
|---|---|
| `--mode <ask\|edits\|plan\|bypass>` | Starts in this mode. |
| `--allowedTools <rules>` (also `--allowed-tools`) | Adds allow rules for this run. Repeatable; rules may be separated by spaces or commas outside parentheses: `--allowedTools "Bash(git diff:*) Edit"`. |
| `--disallowedTools <rules>` (also `--disallowed-tools`) | Adds deny rules for this run. |
| `--add-dir <path>` | Adds a folder the agent reads like the project. Repeatable. |

```bash
{{COMMAND}} -p "fix the failing test" --mode edits --allowedTools "Bash(cargo test:*)"
{{COMMAND}} --disallowedTools "Bash(git push:*),WebFetch"
```

In `-p` runs nobody can answer a question, so any call that would ask is refused and noted (`denied: the agent asked for permission (use --mode or --allowedTools)`). See [Headless](./headless).

## Suggested rules

With the `suggested-rules` feature (on by default), `/rules` looks at your answers in recent conversations in this folder and offers rules for what you keep answering the same way: an allow rule after 5 or more approvals and no refusals, a deny rule after 3 or more refusals and no approvals. Rules you already have are not offered.

```text
/rules           lists the suggestions, numbered
/rules keep 2    saves suggestion 2 to {{DIR}}/settings.local.json
```

## Additional directories

By default the agent reads freely only inside the project. To give it another folder (a sibling repository, shared notes):

- `/add-dir <path>` for the current conversation; `/add-dir` alone lists the added folders.
- `--add-dir <path>` when starting.
- `permissions.additionalDirectories` in your own settings files, for good. `~/` starts at your home folder; a relative path starts at the project.

Paths typed in `/add-dir` and `--add-dir` must be existing folders; `~/` is expanded and relative paths start at the project. An added folder is read like the project and, when the sandbox is on, sandboxed commands may write to it.

## Hooks and permissions

A `PreToolUse` hook can answer `allow`, `ask` or `deny` for a call; `deny` refuses it, `ask` asks you, and `allow` skips the question unless an ask rule matched. A `PermissionRequest` hook runs whenever {{NAME}} is about to ask you and can answer in your place. See [Hooks](./hooks).
