---
title: Instruction files
description: Give {{NAME}} standing instructions for you, your project and parts of your project with {{INSTRUCTIONS}}, {{LOCAL_INSTRUCTIONS}}, AGENTS.md and rule files.
---

Instruction files are plain Markdown that {{NAME}} reads at the start of every prompt and puts into the agent's system prompt. Use them for what the agent should always know: how to build and test the project, conventions, folders to avoid, the way you like to work.

There are three kinds:

- **Instruction files** (`{{INSTRUCTIONS}}`, `{{DIR}}/{{INSTRUCTIONS}}`, `AGENTS.md`, `{{LOCAL_INSTRUCTIONS}}`): always loaded.
- **Rule files** (`{{DIR}}/rules/**/*.md`): loaded always, or only when the agent touches matching paths.
- **Nested instruction files** in subfolders: loaded when the agent works on a file inside that subfolder.

## Quick start

Create `{{INSTRUCTIONS}}` at the root of your repository:

```markdown
# Project notes

- Build with `cargo build`; test with `cargo nextest run --workspace`.
- Never edit files under `generated/`; run `make codegen` instead.
- Use British spelling in user-facing strings.

@docs/architecture.md
```

The next message you send picks it up; nothing needs a restart. The last line imports another file (see [Imports](#imports)).

> [!TIP]
> The built-in `/init` skill analyses the repository and writes a first `{{INSTRUCTIONS}}` for you. If the project already has an `AGENTS.md`, it only adds what that file lacks.

For instructions that are yours alone, for every project, write `~/{{DIR}}/{{INSTRUCTIONS}}`. For private notes about one project that should not be committed, write `{{LOCAL_INSTRUCTIONS}}` next to the project's `{{INSTRUCTIONS}}` and add it to `.gitignore`.

## Which files are read

| File | Scope | Shared with the team |
|---|---|---|
| `~/{{DIR}}/{{INSTRUCTIONS}}` | You, in every project | No |
| `{{INSTRUCTIONS}}` | The project (and every folder above the working folder) | Yes, when committed |
| `{{DIR}}/{{INSTRUCTIONS}}` | Same as `{{INSTRUCTIONS}}`, kept inside the `{{DIR}}` folder | Yes, when committed |
| `AGENTS.md` | Same as `{{INSTRUCTIONS}}` | Yes, when committed |
| `{{LOCAL_INSTRUCTIONS}}` | You, in this project | No (keep it out of git) |
| `~/{{DIR}}/rules/**/*.md` | Your rules, every project | No |
| `{{DIR}}/rules/**/*.md` | The project's rules | Yes, when committed |
| `{{INSTRUCTIONS}}`, `{{LOCAL_INSTRUCTIONS}}` or `AGENTS.md` in a subfolder | That subfolder | Yes, when committed |

### Lookup order

{{NAME}} builds the list in this order, and the agent sees the files in the same order:

1. Your user file, `~/{{DIR}}/{{INSTRUCTIONS}}`.
2. Every folder from the top of the file system down to the working folder. In each folder it looks for, in this order:
   1. `{{INSTRUCTIONS}}`
   2. `{{DIR}}/{{INSTRUCTIONS}}`
   3. `AGENTS.md`
   4. `{{LOCAL_INSTRUCTIONS}}`

So a monorepo can keep general notes at its root and package notes in `packages/api/{{INSTRUCTIONS}}`: starting {{NAME}} in `packages/api` loads both, root first. Files closer to the working folder come later, which lets them refine what the root says.

Every file appears in the system prompt under a heading that names its path and who wrote it:

| Scope | How the agent is told about it |
|---|---|
| user | the user's private instructions for every project |
| project | the project's instructions, kept in the repository |
| local | the user's private instructions for this project |

The agent is told that these instructions take precedence over its default behaviour and must be followed exactly as written.

An empty file (or one that is empty once comments are removed) is skipped.

## AGENTS.md and CLAUDE.md compatibility

{{NAME}} reads `AGENTS.md` in every folder it checks, right after `{{INSTRUCTIONS}}`, so a project that already has one needs no copy. When both exist, both are loaded: put {{NAME}}-specific notes in `{{INSTRUCTIONS}}` and keep shared ones in `AGENTS.md`.

{{NAME}} does **not** read another tool's files. None of these reach a run:

- `CLAUDE.md`, `CLAUDE.local.md`, `.claude/CLAUDE.md` and `~/.claude/CLAUDE.md`
- `.claude/rules/`, `.claude/skills/`, `.claude/agents/`, `.claude/commands/`
- hooks in `.claude/settings.json`
- the project root's `.mcp.json`

To reuse a `CLAUDE.md`, copy what you need into `{{INSTRUCTIONS}}` (the `/init` skill uses it as a source), or import it from `{{INSTRUCTIONS}}`:

```markdown
@CLAUDE.md
```

## Imports

A line can pull another file in with `@path`:

```markdown
See the build notes: @docs/build.md
@"docs/release process.md"
```

- `@` must start the line or follow whitespace.
- A relative path is resolved from the folder of the file that contains it.
- `@~/…` starts from your home folder. It is mostly useful in `~/{{DIR}}/{{INSTRUCTIONS}}`, for example `@~/{{DIR}}/snippets/style.md`, since imports must stay inside a trusted folder (see below).
- Put a path with spaces in double quotes.
- A trailing `,`, `;` or `)` after the path is ignored, so `(see @docs/a.md)` works.
- The imported text is inserted right after the line, under `Imported instructions from <path>:`.
- Imports can import, up to five levels deep.
- Each file is included once per prompt, even if several files import it.

What is **not** an import:

- `@` inside inline code (`` `@not-imported.md` ``) or a fenced code block.
- `@` inside an HTML comment (`<!-- … -->`).

HTML comments are removed from the text the agent sees, so you can leave notes for maintainers that never reach the model. Comments inside code blocks and inline code are kept as written.

### Where imports may point

Imports are confined to a trusted folder:

| File doing the import | It may import files inside |
|---|---|
| `~/{{DIR}}/{{INSTRUCTIONS}}` | `~/{{DIR}}` |
| A project file inside a git repository | The repository (the nearest folder above the working folder that has `.git`) |
| A file in a folder above the repository | That folder |

A file outside its trusted folder, a network path (`//server/…`, `\\server\…`, `/net/…`, `/Network/…`) or a file that cannot be read is not loaded. The agent sees a short note in its place, such as `[Instruction import unavailable: ../private.md: instruction import is outside the trusted directory]`.

### Size limits

| Limit | Value |
|---|---|
| One file | 100,000 bytes |
| All instruction files of a prompt, imports included | 250,000 bytes |
| Import depth | 5 levels |

A file over a limit, or one that is not UTF-8, is replaced by a one-line note such as `[Not loaded: instruction context exceeds its size budget. Tell the user so they can shorten it.]`, so the agent can tell you about it instead of silently missing it.

## Rule files

Rule files live in a `rules` folder and can apply to the whole project or only to some paths.

```text
~/{{DIR}}/rules/            your rules, every project
<project>/{{DIR}}/rules/    the project's rules (subfolders allowed)
```

Every `.md` file under these folders is a rule, up to eight folders deep. Like instruction files, `{{DIR}}/rules` is looked for in every folder from the top of the file system down to the working folder.

### Rules for every task

A rule with no frontmatter applies always:

```markdown
Prefer small, focused commits with a conventional-commit subject.
```

It is added to the reminder the agent gets each turn, under `# Project rule: <path>`.

### Rules for some paths

Add `paths` to the frontmatter to scope a rule to files that match glob patterns:

```markdown
---
paths:
  - "src/**/*.rs"
  - "tests/**/*.rs"
---
Use checked arithmetic. Never call `unwrap()` outside tests.
```

`paths` can also be one comma-separated string; commas inside braces stay in the pattern:

```markdown
---
paths: src/**/*.{ts,tsx}, lib/**/*.ts
---
Components are function components with typed props.
```

Patterns are matched against the path relative to the folder that holds the `{{DIR}}` folder (for your own rules in `~/{{DIR}}/rules`, relative to the working folder). Paths with `..` and symbolic links are resolved first, so `alias/new.rs` matches `src/**` when `alias` links to `src`.

A path-scoped rule loads the first time the agent calls a tool whose `file_path` or `path` matches: that tool call is turned back with the message *New project instructions apply. Read these rules and issue this tool call again*, followed by the rule. The agent then repeats the call with the rule in mind, and the rule stays in its turn reminder for the rest of the conversation.

> [!WARNING]
> A rule whose frontmatter is not valid YAML, or whose `paths` is not a list of strings, fails closed: the next tool call stops the run with an error naming the file. Fix the file and send the prompt again.

## Nested instruction files

A `{{INSTRUCTIONS}}`, `{{LOCAL_INSTRUCTIONS}}` or `AGENTS.md` in a subfolder of the working folder (also inside a subfolder's `{{DIR}}` folder) works like a rule scoped to that subfolder: it loads the first time the agent touches a file inside it.

```text
repo/
├── {{INSTRUCTIONS}}            loaded at the start
└── web/
    ├── {{INSTRUCTIONS}}        loaded when the agent touches web/**
    └── src/
```

{{NAME}} searches up to 32 folders deep and skips `.git`, `node_modules` and `target`. Imports inside nested files are confined to the repository.

## Sub-agents and instruction files

Sub-agents get the same instruction files as the main agent, except:

- the built-in read-only agents (`Explore`, `Plan`) start without them;
- a custom agent with `omitClaudeMd: true` in its frontmatter starts without them (see [Agents](./agents)).

Rules are shared with sub-agents, and an agent that works in its own worktree reads the worktree's files.

## Changing instructions from a hook

A `Brief` hook sees every instruction file and rule before the prompt is built and can replace them, for example to add a team-wide rule from a server. See [Hooks](./hooks#brief).

## Troubleshooting

| Symptom | Cause |
|---|---|
| A `CLAUDE.md` is ignored | {{NAME}} reads `{{INSTRUCTIONS}}` and `AGENTS.md` only. Import it with `@CLAUDE.md`. |
| An import shows *outside the trusted directory* | The imported file is outside the repository (or outside `~/{{DIR}}` for your user file). Move it in. |
| *Not loaded: … size budget* | A file is over 100,000 bytes, or all files together over 250,000. Split or shorten them. |
| A path rule never applies | Its patterns are relative to the folder that holds `{{DIR}}`, not to `{{DIR}}/rules`. |
| A run stops at the first tool call naming a rule | The rule's frontmatter is invalid. |

## See also

- [Settings](./settings) for the files that change how {{NAME}} behaves.
- [Memory](./memory) for what the agent learns on its own.
- [Files and directories](./files-and-directories) for everything under `{{DIR}}`.
