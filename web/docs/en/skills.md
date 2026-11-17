---
title: Skills and custom commands
description: Package instructions, scripts and reference files as skills the agent loads when a task calls for them, or as commands you type with a slash.
---

A **skill** is a folder with a `SKILL.md` file: instructions for one kind of task, plus any scripts, templates or reference files it needs. The agent sees the list of skills with their descriptions and loads one when a task matches, or when you type `/<skill-name>`.

A **custom command** is a single Markdown file you run by typing its name after a slash. Skills and commands share one list and work the same way once loaded.

## Quick start

Create `{{DIR}}/skills/release/SKILL.md` in your project:

```markdown
---
name: release
description: Cut a release of this project. Use when the user asks to release, publish or tag a version.
---

# Release

1. Make sure `main` is clean and up to date.
2. Bump the version in `Cargo.toml` to $ARGUMENTS.
3. Update `CHANGELOG.md` from the commits since the last tag.
4. Commit, tag `v$ARGUMENTS` and push the tag.
```

Then either ask in plain words ("release 1.4.0", and the agent loads the skill because the description matches), or type:

```text
/release 1.4.0
```

`/skills` lists every skill and command found for this folder.

> [!TIP]
> `/skills new <what it should do>` asks the agent to write a skill for you. It loads the built-in `skill-creator` skill and writes `{{DIR}}/skills/<name>/SKILL.md`.

## Where skills live

| Location | Scope |
|---|---|
| `{{DIR}}/skills/<name>/SKILL.md` in the working folder or any folder above it | The project |
| `~/{{DIR}}/skills/<name>/SKILL.md` | You, every project |
| `<plugin>/skills/<name>/SKILL.md` | An installed plugin, named `<plugin>:<name>` |
| Built-in skills | Every project |

Custom commands live next to the skills folder:

| Location | Scope |
|---|---|
| `{{DIR}}/commands/**/*.md` in the working folder or any folder above it | The project |
| `~/{{DIR}}/commands/**/*.md` | You, every project |
| `<plugin>/commands/**/*.md` | An installed plugin, named `<plugin>:<name>` |

When two have the same name, the first one found wins, in this order: the working folder, then each folder above it, then yours, then plugins, then the built-in ones. So a project can replace one of your skills, and you can replace a built-in one, by using its name.

> [!NOTE]
> Skills and commands in another tool's folders, such as `.claude/skills/` or `.claude/commands/`, are not read.

Skills are found again at the start of every prompt; `/reload` shows the new count right away.

## SKILL.md format

```markdown
---
name: pdf-forms
description: Fill in PDF forms from data. Use when the user asks to fill, complete or populate a PDF form.
disable-model-invocation: false
---

# Filling PDF forms

Run `scripts/fill.py <form.pdf> <data.json>` from this skill's folder…
```

| Frontmatter field | Meaning |
|---|---|
| `name` | The skill's name, and the `/name` you type. Defaults to the folder's name. |
| `description` | What the skill does and when to use it. The agent decides from this text whether to load the skill. Defaults to the first line of the body that is not empty and not a heading. |
| `disable-model-invocation` | `true` keeps the skill out of the agent's list: only you can run it, by typing `/name`. |

Other frontmatter fields are ignored. A multi-line description can use YAML's `|` or `>` followed by indented lines.

The body is what the agent reads when it loads the skill. In it:

- `$ARGUMENTS` is replaced by what follows the command (`/release 1.4.0` gives `1.4.0`).
- When arguments were given, `Arguments: <arguments>` is also added at the end.
- The agent is told the skill's folder (`Skill release (base directory for its files: …)`), so the body can refer to files next to `SKILL.md` by relative path.

### Supporting files

Keep the body short and put details in files the agent reads only when needed:

```text
{{DIR}}/skills/pdf-forms/
├── SKILL.md
├── reference/fields.md
└── scripts/fill.py
```

## Custom commands

A command is one Markdown file; its name is its path inside `commands/` without the extension, with `:` between folders:

| File | Command |
|---|---|
| `{{DIR}}/commands/audit.md` | `/audit` |
| `{{DIR}}/commands/db/migrate.md` | `/db:migrate` |
| `<plugin acme>/commands/hello.md` | `/acme:hello` |

```markdown
---
description: Audit the dependencies for known vulnerabilities
---
Run `cargo audit` and summarise each finding for $ARGUMENTS with a suggested fix.
```

Commands accept `description` and `disable-model-invocation` like skills; the description defaults to the first line of the body. Commands are offered to the agent too, unless `disable-model-invocation: true`.

## How skills are invoked

### By you

Type `/<name>` followed by any arguments. The skill's body (or the command's) is loaded and sent with your message, before hooks such as `UserPromptSubmit` see it. This works for every skill and command, including those with `disable-model-invocation: true`.

### By the agent

The system prompt lists the skills the agent may use, with their descriptions, and tells it to load one with the `Skill` tool when you type `/<name>` or when a listed skill covers the task. The agent calls:

```json
{ "skill": "release", "args": "1.4.0" }
```

The listing has a budget of about 8,000 characters: each description is cut to at most 1,536 characters, shorter when there are many skills, and with very many skills only the names are listed. Put the words that decide when to use a skill at the start of its description.

A skill with `disable-model-invocation: true` is not listed, and the agent gets *requires explicit user invocation* if it tries to load it.

### Skills preloaded into an agent

A custom agent can list skills in its `skills` frontmatter: they are loaded into its system prompt when it starts. See [Agents](./agents).

## JavaScript skills

A skill folder can hold `SKILL.js` instead of `SKILL.md`. It runs in {{NAME}}'s built-in script runtime and returns the text the agent receives:

```javascript
export const name = "changelog";
export const description = "Show the commits since the last tag. Use before writing release notes.";
export const modelInvocation = true;
export const capabilities = ["exec"];

export default function (args, ctx) {
  const tag = ctx.exec("git describe --tags --abbrev=0").stdout.trim();
  return ctx.exec(`git log --oneline ${tag}..HEAD`).stdout;
}
```

| Export | Meaning |
|---|---|
| `name` | The name; defaults to the folder's |
| `description` | Like `description` in `SKILL.md` |
| `modelInvocation` | `false` works like `disable-model-invocation: true` |
| `capabilities` | The host functions the script calls: `readFile`, `writeFile`, `exec`, `fetch`. Empty by default. |
| `default` | Called with the arguments (a string) and a context; returns the text. It has 60 seconds. |

A `.js` file in a `commands/` folder works the same way as a command. A call to a host function the script did not list in `capabilities` fails; a listed one is judged by your permissions like the matching tool (`ctx.exec` like `Bash`), asking you where they say so (see [Capabilities](./sandbox-and-safety#capabilities)). The `scripts` setting stays the outer limit (see [Settings](./settings)).

> [!WARNING]
> Breaking change: a script skill or command that calls `ctx.readFile`, `ctx.writeFile`, `ctx.exec` or `ctx.fetch` without listing it in `export const capabilities` now fails with an error saying what to add. A project's `SKILL.js` is replaced by one of yours or a plugin's with the same name.

> [!IMPORTANT]
> A project's `SKILL.js` and `.js` commands are offered only after you trust the project's folder; its Markdown skills and commands load either way. The same holds for a project's script tools in `{{DIR}}/tools/`. See [Workspace trust](./sandbox-and-safety#workspace-trust).

## Script tools

A script tool is a new tool the agent can call, written in JavaScript. Put it in `~/{{DIR}}/tools/` (yours, every project) or in the working folder's `{{DIR}}/tools/` (the project's, once you trust the folder), as `<name>.js` or `<name>/tool.js`:

```javascript
export const spec = {
  name: "Release",
  description: "Tags the release. Use when the user asks to tag a version.",
  input: { type: "object", properties: { version: { type: "string" } }, required: ["version"] },
  access: "execute",
  capabilities: ["exec"],
};

export function run(input, ctx) {
  return ctx.exec(`git tag v${input.version}`).stdout;
}
```

| `spec` field | Meaning |
|---|---|
| `name` | The tool's name. Required. |
| `description` | What the agent reads to decide when to call it |
| `input` | The JSON Schema of its input; any object when left out |
| `access` | What a call does, for permissions: `read`, `edit` or `execute` (the default). Only your own tools set it; a project's tools always count as `execute`. See [Permissions](./permissions). |
| `capabilities` | The host functions it calls: `readFile`, `writeFile`, `exec`, `fetch`. Empty by default. |

The function `run` (or `execute`) gets the input and a context, and returns text or a value, sent as JSON; a thrown error is a failed call. A call to a host function not listed in `capabilities` fails; a listed one is judged by your permissions like the matching tool (see [Capabilities](./sandbox-and-safety#capabilities)). A script tool may run for 60 seconds.

A tool of yours with the name of a built-in tool replaces it; a project's tool with a name that is already taken is left out. A file that cannot be read, or whose `spec` has no `name`, is left out with a warning in the log.

## Built-in skills

{{NAME}} ships these skills, which you can replace with your own of the same name:

| Skill | What it does |
|---|---|
| `code-review` | Reviews the current diff, a PR, branch or path for bugs at a chosen effort level |
| `review` | Reviews the current changes at medium effort |
| `security-review` | Reviews the pending changes for security issues |
| `simplify` | Looks for reuse, simplification and efficiency cleanups in changed code and applies them |
| `verify` | Checks that a change works by running the affected flow (only when you type `/verify`) |
| `run` | Launches and drives the project's app to see a change working |
| `init` | Writes a first `{{INSTRUCTIONS}}` for the repository |
| `configure` | Changes {{NAME}}'s own settings, MCP servers and hooks |
| `debug` | Helps diagnose a problem with {{NAME}} itself (only when you type `/debug`) |
| `skill-creator` | Creates and improves skills |
| `follow-pr` | Follows a pull request to merge, waking on failing checks and new reviews |

### Skills behind a feature flag

`follow-pr` is available while the `experimental-pr-follow` feature is on (the default). Turning it off in your settings removes the skill:

```json
{ "features": { "experimental-pr-follow": false } }
```

See [Experimental features](./experimental-features).

## Changing skills from a hook

A `Brief` hook receives every skill found (name, description, path, body and whether the model may invoke it) and can add, rewrite, hide or remove skills before the prompt is built. See [Hooks](./hooks#brief).

## Troubleshooting

| Symptom | What to check |
|---|---|
| The agent never uses a skill | Its description does not say when to use it, or `disable-model-invocation` is `true`. |
| `/name` is sent as plain text | No skill or command has that name here; check `/skills`. |
| A skill seems to be ignored | Another one with the same name is found first (the working folder comes first). |
| A skill from `.claude/skills` is missing | Move it to `{{DIR}}/skills`. |

## See also

- [Slash commands](./slash-commands) for the chat's own commands.
- [Agents](./agents) to preload skills into an agent.
- [Plugins](./plugins) to share skills and commands.
