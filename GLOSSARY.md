# Superfik Lite

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
Claude, ChatGPT), or local MLX models.
_Avoid_: Plugin (for providers)

**Extensions**:
What an SDK host adds to every run: folders of skills, commands and agents,
and MCP servers (HTTP, stdio or in-process).
_Avoid_: Plugin (that is an installed package, below)

**Plugin**:
An installed package in Claude Code's plugin layout: skills, commands,
agents, hooks and MCP servers under one name (ADR 0019).

**Marketplace**:
A git repository listing plugins (`.claude-plugin/marketplace.json`).
