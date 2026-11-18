---
title: Introduction
description: What {{NAME}} is, what it can do, and how this documentation is organised.
---

{{NAME}} is one terminal coding agent for several model providers, with a trust boundary that holds when the input is hostile: the same chat, rules and sessions whichever model runs, and a clear line on what the agent may do without asking. It is for one developer, on their own machine and their own phone; it is not a hosted or multi-user service, an embeddable SDK or an editor. You run `{{COMMAND}}` in a project folder, describe what you want in plain words, and the agent reads the code, edits files, runs commands and reports back, asking you before it does anything you have not allowed.

```bash
cd my-project
{{COMMAND}}
```

You work in a terminal chat: there is no GUI. A local page in your browser only shows the [trace](./trace-console) and these docs, and can hold a terminal attached to a session. The same agent can also answer once and print the result (`{{COMMAND}} -p "..."`) for scripts and CI.

## What you can do with it

- **Work on code in conversation.** Ask questions about a codebase, have the agent fix a bug, add a feature or write tests, and follow along as it reads, edits and runs commands. See [Using the chat](./chat).
- **Stay in control of what it does.** Four permission modes (ask, edits, plan, bypass), allow/ask/deny rules and an optional sandbox decide what runs without asking. See [Permissions](./permissions) and [Sandbox and safety](./sandbox-and-safety).
- **Plan before coding.** In plan mode the agent only reads and proposes a plan; you approve it before any edit. See [Slash commands](./slash-commands#mode).
- **Go back in time.** Every prompt is a checkpoint: rewind files and conversation, retry a prompt, branch or fork a conversation, resume an earlier one. See [Sessions](./sessions).
- **Choose models and providers.** Connect a provider with an API key or a subscription, switch models mid-conversation, set a model per role and a fallback chain. See [Models and providers](./models-and-providers) and [Accounts](./accounts).
- **Give it project knowledge.** Instruction files ([{{INSTRUCTIONS}}](./instructions-files)), [memory](./memory) that persists between sessions, and [code intelligence](./code-intelligence) from language servers.
- **Extend it.** Add [skills](./skills) and custom commands (run as `/name`), [sub-agents](./agents), [hooks](./hooks), [MCP servers](./mcp), [channels](./channels) and [plugins](./plugins), or draw your own [status line](./statusline).
- **Check its work.** [Verification and review](./verification-and-review) judges runs against criteria; [budgets](./budget-and-cost) cap spending.
- **Automate.** Run headless with `-p` ([Headless](./headless)), schedule prompts ([Schedules](./schedules)), score the agent on your own cases ([Evals](./evals)).
- **Watch it work.** The [trace console](./trace-console) shows every session's runs, spans, cost and waits live in the browser.
- **Take it with you.** [Remote control](./remote-control) puts the console and a terminal on your phone, through a relay you own, end-to-end encrypted.

## How the chat looks

The screen has four parts, from top to bottom:

| Part | What it shows |
|---|---|
| Conversation | Your prompts, the agent's answers (rendered Markdown with highlighted code), its tool calls, diffs, plans and notices. |
| Panel | Whatever needs you now: a question or permission request, a picker, completions, or the agent's task list. |
| Prompt | Where you type. A `>` mark, or a pink `!` in shell mode. |
| Status line | Conversation title, model, permission mode, effort, context left, cost and feature markers. |

## How these docs are organised

| Section | Pages |
|---|---|
| Getting started | This page and the [Quickstart](./quickstart). |
| Using the chat | [Chat](./chat), [Keyboard shortcuts](./keyboard-shortcuts), [Slash commands](./slash-commands), [Sessions](./sessions), [Output styles](./output-styles). |
| Models and accounts | [Models and providers](./models-and-providers), [Accounts](./accounts). |
| Configuration | [Settings](./settings), [Permissions](./permissions), [Sandbox and safety](./sandbox-and-safety), [Environment variables](./environment-variables), [Files and directories](./files-and-directories), [Instructions files](./instructions-files). |
| Extending | [Skills](./skills), [Agents](./agents), [Hooks](./hooks), [MCP](./mcp), [Channels](./channels), [Plugins](./plugins), [Status line](./statusline). |
| Features | [Experimental features](./experimental-features), [Memory](./memory), [Budget and cost](./budget-and-cost), [Branches and git](./branches-and-git), [Verification and review](./verification-and-review), [Learning](./learning), [Schedules](./schedules), [Code intelligence](./code-intelligence), [Trace console](./trace-console), [Remote control](./remote-control). |
| Automation | [Headless](./headless), [Evals](./evals). |
| Reference | [CLI reference](./cli-reference), [Troubleshooting](./troubleshooting). |

> [!TIP]
> Inside the chat, `/help` lists every command and the main keys, `/help <command>` explains one, and `/docs` opens this documentation in your browser (`/docs <topic>` searches it).

## Where to go next

- New here? Follow the [Quickstart](./quickstart).
- Already running? Learn the [chat](./chat) and its [keyboard shortcuts](./keyboard-shortcuts).
- Looking for a command? See the [slash command reference](./slash-commands).
