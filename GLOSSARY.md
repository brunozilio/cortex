# Glossary

A coding agent you talk to in the terminal, and an SDK to build on it.

**Principal**:
Who is acting on a command. Today always the local owner.

**Project**:
The folder a session works in; memory is kept per project.
_Avoid_: Workspace, repo

**Session**:
One conversation with the agent: its turns and context, resumable.
_Avoid_: Chat, thread

**Run**:
One execution of an engine for a prompt or a session command, ending in
exactly one terminal event.

**Memory**:
What an agent keeps about a project between sessions: episodic, semantic or
procedural.

**Provider**:
Where models come from: an API key or a subscription (Anthropic, OpenAI,
Claude, ChatGPT, GitHub Copilot, Kiro), or local MLX models.
_Avoid_: Plugin (for providers)

**Local model**:
An open model run on this computer by MLX (Apple silicon), named by its
Hugging Face id (`mlx-community/Qwen3-8B-4bit`). `<command> pull` downloads
it into `<local data>/models/mlx/<org>/<name>`, `<command> rm` deletes it.

**Extensions**:
What an SDK host adds to every run: folders of skills, commands and agents,
and MCP servers (HTTP, stdio or in-process).
_Avoid_: Plugin (that is an installed package, below)

**Plugin**:
A folder with `.<command>-plugin/plugin.json`, put in `~/.<command>/plugins/`:
skills, commands, agents, hooks and MCP servers under one name (ADR 0019).

**Verification**:
The criteria a run's work must meet before it completes, set by the user or
taken from the last plan, and the judge agent that checks them (ADR 0020).
_Avoid_: Eval, test (those are what the judge may run)

**Verdict**:
The judge's grade of one attempt: passed, a score from 0 to 1, and one check
per criterion with its evidence.

**Language server**:
A process that understands one language (rust-analyzer, clangd…) and
answers the `LSP` tool's questions and an edit's diagnostics (ADR 0023).
Kept running per server and project root between runs.
_Avoid_: LSP (that is the protocol and the tool's name)
