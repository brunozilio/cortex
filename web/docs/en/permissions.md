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

The "don't ask again" choice is offered only when your answer stops the next question. It is not offered for questions that come back every time anyway: risky commands (pushes, publishes…), an address that never appeared in the conversation (see [Always asked](#actions-that-always-ask)), running a command outside the sandbox, installing a flagged package, a call an `ask` rule matched, and a call a hook asked about.

In a folder you have not trusted, a rule saved to `{{DIR}}/settings.local.json` holds for that run only; it applies to later runs once you trust the folder, and the chat says so when it saves the rule. See [Workspace trust](./sandbox-and-safety#workspace-trust).

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
| Edits | `edits` (also `acceptEdits`, `accept-edits`) | Allowed | Ask | Default. You trust edits in the project, not commands. |
| Plan | `plan` | Refused | Refused | The agent only reads, searches and proposes a plan for you to approve. |
| Bypass | `bypass` (also `bypassPermissions`, `bypass-permissions`, `yolo`) | Allowed | Allowed, after a safety check | Unattended work in a disposable environment. |

The name in the second column is the one the app shows and writes. The others in brackets are read as the same mode, in any letter case, wherever a mode is given: `--mode`, `/mode`, the saved `mode` setting and a sub-agent's `permissionMode`.

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

### Paths

Path specifiers match like `.gitignore`:

- `//path` is absolute, `~/path` starts at your home folder, `./path` starts at the project.
- A pattern with a `/` in it starts at the project (`src/**`, `config/prod.env`).
- A pattern without `/` matches that name at any depth in the project (`*.env`, `secrets`).
- `*` and `?` stay inside one folder; `**` crosses folders.
- A rule that matches a folder covers everything inside it.
- Deny and ask rules ignore case (`Read(.env)` also refuses `.ENV`); allow rules do not.
- A `Glob` or `Grep` over a folder is refused when a deny rule covers something inside it.

### Commands

Shell rules read the command the way the shell will run it. The line is parsed into every simple command it runs, including those inside `if`, `for`, `while`, `case`, functions, subshells, `{ …; }`, `$(…)`, backticks, `<(…)` and the command `find -exec` runs:

- **Compound commands** (`&&`, `;`, `|`, newlines, control structures): every part must be allowed. Read-only parts (`ls`, `cat`, `git status`…) and `cd` into a folder inside the project go along without a rule. One denied part refuses the whole command.
- **Hidden commands** (`$(…)`, backticks, `<(…)`, `bash -c '…'`, `eval`, a program named by a variable) and **redirections to files** are never allowed by an allow rule. The commands inside a substitution are parts of their own, so a deny rule naming one refuses the whole line, and so is the code of `bash -c '…'`, `sh -c "…"` and `eval …`: `bash -c 'rm -rf x'` is refused by `Bash(rm:*)`. A program the rules cannot see (`$(echo rm) x`, `$X x`, `` `echo rm` x ``, a wildcard such as `/bin/r? x` or `{rm,x}`, a shell reading its commands from a pipe like `echo … | sh`) is asked about **every time, in every mode including bypass**, while a deny rule with a specifier exists; with nobody to ask it is refused. Program names are compared ignoring case (`RM`, `/bin/RM`).
- **Deny rules see through disguises**: `sudo rm`, `env rm`, `VAR=1 rm`, `\rm`, `"rm"`, `/bin/rm`, `(rm x)`, `{ rm x; }`, `if true; then rm x; fi`, `f() { rm x; }`, `ls | xargs rm`, `busybox rm`, `find . -exec rm {} \;` all match `Bash(rm:*)`. The wrappers taken apart are `sudo`, `doas`, `env`, `exec`, `nice`, `timeout`, `nohup`, `time`, `command`, `builtin`, `stdbuf`, `ionice`, `chrt`, `xargs`, `busybox`, `setsid`, `chroot`, `flock`, `watch`, `script`, `caffeinate`, `xcrun`, `taskset`, `su`, `runuser`, `sg`, `pkexec`, `unbuffer`, `arch` and `strace`; the command they are given as text (`su -c '…'`, `flock f -c '…'`, `script -c '…'`, `watch '…'`, `sg group '…'`, `env -S '…'`) is ruled like the line itself.
- **Redirections do not hide a program**: rules are matched on each command with its redirections taken out, so `2>/dev/null rm -rf y`, `<x rm -rf y`, `rm</dev/null -rf y` and `curl<<<x …` match `Bash(rm:*)` and `Bash(curl:*)`. A file read through an input redirection (`<secrets/k cat`, `cat < secrets/k`) is judged by the `Read` rules, whatever the command.
- **A denied program handed to an unknown one asks**: when a program the rules do not know is given a word naming a denied program (`frobnicate rm -rf x`, `python3 -c 'import os; os.system("rm -rf x")'`, `osascript -e '… rm …'`), the line is asked about every time, bypass included. `echo`, `printf`, the read-only programs and a subcommand right after `docker`, `npm`, `cargo` and the like (`docker rm`) are left out.
- **Allow rules read the command as written**: `LD_PRELOAD=/tmp/x.so git status` does not match `Bash(git status:*)`.
- **A line that cannot be parsed with confidence** (an unclosed quote, arithmetic such as `$((…))` holding a command, `coproc`, a heredoc whose body runs a substitution) is never allowed by a rule with a specifier and is asked about when deny or ask rules with a specifier exist.

Deny rules are a best-effort list: a command built while it runs (a script file, an interpreter's inline code, the value of a variable) is invisible to them. The [sandbox](./sandbox-and-safety) is what stops a command from writing where it should not.

### Read-only commands

A shell command runs without asking, in every mode, only when the whole line is a plain list or pipeline (no control structure, function, subshell, group, `!` or substitution) and every command in it:

- is a listed reading program (`ls`, `cat`, `head`, `grep`, `rg`, `find`, `git status`, `git diff`, `git log`…) used only with flags listed for it;
- has no `VAR=value` prefix and no wrapper (`sudo ls` asks);
- redirects only to `/dev/null` or between its own outputs (`2>&1`), never to a file;
- has no word the shell would change: no `$`, backtick or backslash outside single quotes, no unquoted `*`, `?`, `[`, `{` or `<`, and no `~` that names a home folder. Quoted patterns are plain words: `find . -name '*.rs'` and `grep 'a.*b' notes.txt` are read-only, `find . -name *.rs` is not. `git diff HEAD~1` is read-only;
- names no path outside the project (`/…`, `..`), including through a symbolic link: `cat ./link` where `link` leads out of the project asks like `Read` does, and `Read` rules judge where the link leads as well as the name written.

### Tools and their effects

Each built-in tool says, from its input, what the call will do, and the decision is made from that:

| Tool | Treated as |
|---|---|
| `Read`, `LSP` | reading that file (asks outside the project); `LSP` starts no server in a folder you have not trusted |
| `Glob`, `Grep` | reading the folder they search |
| `Write`, `Edit`, `NotebookEdit` | writing that file (asks outside the project and the added folders, in `.git/` or in `{{DIR}}/`; there only a rule that names the path, such as `Edit(.git/hooks/**)`, allows it, not a bare `Edit`) |
| `Bash`, `Monitor` | running the command (read-only commands run freely) |
| `WebFetch`, `WebSearch` | reaching the network (`WebFetch(domain:…)` rules) |
| `CronCreate`, `ScheduleWakeup` | starting work later: asks |
| `EnterWorktree`, `ExitWorktree` | changing the session's checkout: asks (refused in plan mode) |
| `Agent`, `Watch`, `PushNotification`, task and plan tools | no permission needed; a sub-agent is held to its own mode |
| `AskUserQuestion`, `Skill`, `ToolSearch`, `Advisor`, `BashOutput`, `CronList`, `ListSessions` | no permission needed |
| `REPL` | no permission for the script itself; each tool its script calls is judged like that tool |
| `KillShell`, `CronDelete`, `TaskStop` | no permission needed for the agent's own work; `KillShell` on anything else asks |
| `SendMessage` to another session | asks unless a rule allows `SendMessage` |
| `ListMcpResources`, `ReadMcpResource` | follow that server's rules (`mcp__docs`, `mcp__docs__resources(…)`) |
| MCP tools | asks unless a rule allows the server or the tool |

Your own script tools in `~/{{DIR}}/tools/*.js` say what they do with `access` in their `spec`: `"read"` runs freely, `"edit"` is treated like an edit, `"execute"` (or no `access`) asks. A project's script tools always ask. `access` only covers the call itself: each host function the script uses (`readFile`, `writeFile`, `exec`, `fetch`) must also be listed in `capabilities` in the `spec`, and each of those calls is judged like `Read`, `Write`, `Bash` or `WebFetch` (see [Capabilities](./sandbox-and-safety#capabilities)).

### Invalid rules

A rule without its closing `)`, with an empty specifier (`Bash()`), or that does not start with a tool name stops the run with an error naming the list, for example ``permissions.deny: rule "Bash(rm" misses its `)` ``.

## Actions that always ask

Some actions are asked about every time, whatever the mode and rules (except a deny rule, which refuses them):

- Pushing to the default branch, or force-pushing, deleting or rewriting remote history.
- `cargo publish`, `npm publish`, `pnpm publish`, `yarn publish`.
- `gh pr merge`, `gh release create|delete|edit`, `gh api`.
- `docker push`, `podman push`, `docker image push`.
- The same commands with global options before the subcommand (`git -C . push -f`, `npm --prefix . publish`).
- A shell command whose program the rules cannot see, while a deny rule for `Bash` with a specifier exists (see above).
- Fetching an address that never appeared in the conversation, once the session has read content from outside (web pages, MCP tools).
- Running a command outside the sandbox, installing a suspicious package, and installing a language server (unless an allow rule such as `LSP(install:pyright)` names it). See [Sandbox and safety](./sandbox-and-safety).

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

Paths typed in `/add-dir` and `--add-dir` must be existing folders; `~/` is expanded and relative paths start at the project. An added folder is read and edited like the project and, when the sandbox is on, sandboxed commands may write to it.

## Hooks and permissions

A `PreToolUse` hook can answer `allow`, `ask` or `deny` for a call; `deny` refuses it, `ask` asks you, and `allow` skips the question unless an ask rule matched. A `PermissionRequest` hook runs whenever {{NAME}} is about to ask you and can answer in your place. See [Hooks](./hooks).
