---
title: Quickstart
description: Install {{NAME}}, connect a provider and send your first prompt in a few minutes.
---

This guide takes you from nothing to a working conversation: build and install the `{{COMMAND}}` binary, connect a model provider, and ask the agent about your project.

## Before you begin

You need:

- A Rust toolchain recent enough for the workspace (it declares `rust-version = "1.97"`).
- A terminal. The chat is a full-screen terminal app; there is no GUI.
- An account with an LLM provider: either an API key or a subscription you can sign in to.

Some tools are used when they are present, and nothing breaks without them:

| Tool | Used for |
|---|---|
| `git` | `/diff`, `/diffs`, branch comparisons and worktrees. |
| `pdftotext` | Reading PDFs. |
| `ripgrep` | Optional. The `Grep` tool uses `rg` when it is on `PATH` (faster on large projects, same results) and its built-in search otherwise; `doctor` reports which. |
| Language servers (rust-analyzer, clangd…) | The `LSP` tool and diagnostics after edits. See [Code intelligence](./code-intelligence). |
| `node` / `npx` | Only for MCP servers that need them. |

## Step 1: Install

Build and install the binary from a checkout of the source:

```bash
cargo install --path crates/cli
```

This puts `{{COMMAND}}` on your `PATH` (in Cargo's bin folder). Check it runs:

```bash
{{COMMAND}} --version
```

## Step 2: Check the installation

`{{COMMAND}} doctor` checks that this installation can work in the current folder and exits with a failure status when a check fails:

```bash
cd my-project
{{COMMAND}} doctor
```

The same report is available inside the chat as `/doctor`.

## Step 3: Connect a provider

You can connect from the shell or from inside the chat.

From the shell:

```bash
{{COMMAND}} login
```

It lists the providers, numbered, each with how it connects (`paste an API key`, `sign in with the browser` or `sign in with a code`), and asks you to choose one:

- **API key**: paste the key at the hidden prompt.
- **Browser sign-in**: the browser opens; if it does not, the terminal prints the address to visit.
- **Code sign-in**: the terminal shows a code to enter at an address, and opens the browser there.

When it works you see `Connected <provider> (<account>).`

From the chat, run `/login`: a **Connect a provider** picker opens. Choose with `↑`/`↓` and `Enter`. For an API key, a masked field appears in the panel: paste or type the key, `Enter` connects, `Esc` cancels.

> [!NOTE]
> If no model is available when the chat starts, it says "No model yet. Connect a provider to start:" and opens the login picker for you.

Keys and tokens are sealed in `secrets.json` in the data folder (its key in `secrets.key`, both readable only by you). See [Accounts](./accounts) and [Files and directories](./files-and-directories).

To see what the connected providers offer:

```bash
{{COMMAND}} models
```

## Step 4: Start the chat

Run `{{COMMAND}}` in your project folder:

```bash
cd my-project
{{COMMAND}}
```

The chat opens full screen with the product name and the folder at the top, a few hints (`/resume`, `/model`, `@file`, `!cmd`), the prompt, and the status line showing the model, the permission mode and the effort.

If the project brings things that run code or widen what the agent may do (hooks, MCP servers, script tools, grants in `settings.local.json`…), the chat first asks **Do you trust the files in …?**: **Trust this folder**, **Continue without trusting** or **Exit**. See [Workspace trust](./sandbox-and-safety#workspace-trust).

You can also open the chat with a first prompt already sent:

```bash
{{COMMAND}} "explain this project"
```

> [!TIP]
> Prefer the chat at the bottom of the terminal, with the conversation in your terminal's own scrollback? Start it with `{{COMMAND}} --inline`. See [Inline mode](./chat#inline-mode).

## Step 5: Send your first prompt

Type a request and press `Enter`:

```text
explain this project
```

The agent reads files and answers. While it works you see a line like `Working… (12s · esc to interrupt)`; press `Esc` to stop it.

If the folder is not a git repository, the first message offers to initialize one (**Yes, initialize git**, **No, not now**, **No, and don't offer again**). Git lets you review and undo changes and isolate agents in worktrees.

Now ask for a change:

```text
add a --verbose flag to the CLI and a test for it
```

When the agent wants to edit a file or run a command that is not allowed yet, the panel asks you:

| Answer | Key |
|---|---|
| Yes | `y` |
| Yes, and don't ask again (for that rule in this project, or this session) | `a` |
| No, and tell the agent what to do instead | `n`, or pick it and type a note |

## Step 6: Learn the essentials

| Do this | How |
|---|---|
| Add a new line to the prompt | `Alt+Enter` or `Ctrl+J` (or `Shift+Enter` where the terminal reports it, see `/terminal-setup`) |
| Point the agent at a file | Type `@` and part of the name, then `Tab` |
| Run a shell command yourself | Start the prompt with `!` |
| Attach an image | `Ctrl+V`, or drop the image file on the terminal |
| Change the permission mode | `Shift+Tab` cycles ask → edits → plan → bypass |
| Switch the model | `/model` |
| Undo the last prompt's edits | `Esc` `Esc` on an empty prompt, or `/rewind` |
| See the commands | `/help` |
| Quit | `Ctrl+C` twice, `Ctrl+D` on an empty prompt, or `/exit` |

When you quit after a conversation, the terminal prints `Continue this conversation with: {{COMMAND}} -c`.

## Next steps

- [Chat](./chat): everything the prompt can do.
- [Keyboard shortcuts](./keyboard-shortcuts) and [Slash commands](./slash-commands): the full references.
- [Sessions](./sessions): resume, rewind, fork and export conversations.
- [Permissions](./permissions): decide what runs without asking.
- [Instructions files](./instructions-files): tell the agent about your project in {{INSTRUCTIONS}}.
- [Headless](./headless): use `{{COMMAND}} -p` in scripts and CI.
