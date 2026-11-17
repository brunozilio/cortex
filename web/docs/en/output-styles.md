---
title: Output styles
description: Change how the agent talks and works with the built-in output styles, or write your own.
---

An output style changes how the agent communicates: how much it says, whether it explains itself, whether it asks you to write code. The work it does stays as thorough; only the way it reports and collaborates changes.

```text
/output-style concise
```

## The built-in styles

| Style | In one line |
|---|---|
| `default` | {{NAME}}'s usual way: works, then reports. |
| `concise` | Leads with the result and skips preamble and narration. |
| `explanatory` | Explains its choices and the codebase's patterns as it works. |
| `learning` | Pauses to have the user write small, meaningful pieces of code. |
| `proactive` | Acts at once, interrupts little and prefers doing to planning. |

### default

No style is added to the system prompt: the agent follows its standard instructions for delivering work and writing answers.

### concise

The agent keeps responses short and direct while doing the work just as thoroughly:

- Its first sentence answers what happened or what the answer is; no "Let me…" preamble and no closing recap.
- It reports outcomes, decisions and what you must act on, not each step it took.
- Simple questions get one to three sentences of plain prose; headings, tables and lists only when they carry real structure.
- Caveats only when they change what you should do next.
- Full detail when you ask for an explanation.
- Error reports, failing test output, security warnings and confirmations for destructive actions keep their full content.

A short reminder of the style is repeated to the agent every turn.

### explanatory

Besides doing the work, the agent teaches you about the codebase. Before and after writing code it adds a brief insight about the choices made, framed like this:

```text
★ Insight ─────────────────────────────────────
[two or three key points]
─────────────────────────────────────────────────
```

Insights stay in the conversation, never in the code, and favour what is specific to this codebase over general programming knowledge.

### learning

The agent helps you learn by doing. When it would write twenty lines or more involving a design decision, business logic with more than one valid approach, or a key algorithm or interface, it asks you to write a piece of two to ten lines instead:

1. It adds a `TODO(human)` marker to the code (only one at a time).
2. It asks under **Learn by doing**, with the **Context**, **Your task** (the file and the marker) and **Guidance** on the trade-offs.
3. It stops and waits for your code.

After your contribution it shares one insight connecting your code to the wider system. It also adds the same `★ Insight` notes as the explanatory style.

### proactive

The agent works continuously and autonomously:

- It starts implementing at once and makes reasonable assumptions on low-risk work.
- It prefers an assumption to a question for routine decisions.
- It does not stop to plan unless you ask for a plan.
- It treats your corrections as ordinary input.
- It still asks before destroying data or changing shared or production systems, and only posts to chats, channels or tickets, or shares secrets, when you told it to.

A short reminder of the style is repeated to the agent every turn.

> [!NOTE]
> The style shapes how the agent talks; your [permission mode](./permissions) still decides what it may do without asking. `proactive` in `ask` mode still asks before edits and commands.

## Set the style

### In the chat

`/output-style` opens a picker with the built-in styles and the current one highlighted; `/output-style <name>` sets one directly:

```text
/output-style
/output-style explanatory
/output-style default
```

The chat answers `Output style: explanatory, from the next message.` The choice applies from your next message and is remembered for your next chats. `/config` also lists **Output style** with its current value.

### In settings

The `outputStyle` setting picks a style for every session that reads those settings, including `{{COMMAND}} -p`:

```json
{
  "outputStyle": "concise"
}
```

Put it in `~/{{DIR}}/settings.json` (yours), `{{DIR}}/settings.json` (the project's, checked in) or `{{DIR}}/settings.local.json` (yours, for this project). Or set it from the shell:

```bash
{{COMMAND}} config set outputStyle concise              # ~/{{DIR}}/settings.json
{{COMMAND}} config set outputStyle concise --project    # {{DIR}}/settings.local.json
{{COMMAND}} config unset outputStyle
```

See [Settings](./settings).

### Which one wins

1. A built-in style chosen in the chat (with `/output-style`) applies to that chat. `/output-style default` turns styles off for it, whatever the settings say.
2. Otherwise the `outputStyle` setting applies.
3. A name the chat does not know as built-in leaves the choice to the setting.

Built-in names are matched in any case (`Concise` works).

## Custom styles

Write your own style as a Markdown file named after the style in an `output-styles` folder:

```text
~/{{DIR}}/output-styles/review.md        your styles
<project>/{{DIR}}/output-styles/review.md  the project's styles
```

{{NAME}} looks in `~/{{DIR}}` and in the `{{DIR}}` folder of the project and of each folder above it; the one closest to the project wins.

The body is the instruction added to the system prompt. Optional frontmatter tunes it:

```markdown
---
keep-coding-instructions: true
turn-reminder: Report findings as a numbered list, most severe first.
---
Review code instead of writing it. For each problem, give the file and line,
what is wrong, and a suggested fix. Do not edit files unless asked.
```

| Key | Effect |
|---|---|
| `keep-coding-instructions` | `true` keeps the standard instructions for delivering work and writing answers alongside your style. Anything else, or leaving it out, drops them, so the style fully replaces how the agent reports. |
| `turn-reminder` | A short text repeated to the agent every turn to keep the style in mind. |

Select it with the setting:

```json
{
  "outputStyle": "review"
}
```

> [!IMPORTANT]
> A custom style name may contain only letters, digits, `-` and `_`. A name with anything else, or a style file that cannot be found, is an error in the settings (`invalid outputStyle name`, `output style review was not found`).

The `/output-style` picker lists only the built-in styles; choose a custom style through `outputStyle`.

## What a style does not change

- Sub-agents' system prompts leave the style's instructions out; they are written for the main agent.
- The status line command receives the chat's current style as `output_style` in its JSON input; see [Status line](./statusline).
