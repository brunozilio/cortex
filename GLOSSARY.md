# Superfik Lite

A coding agent you talk to in the terminal, and an SDK to build on it.

**Principal**:
Who is acting on a command. Today always the local owner.

**Project**:
What a set of sessions is about, identified by its git remote, or a folder
with no repository.
_Avoid_: Workspace (for the shared thing), repo

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

**Plugin**:
A folder bringing skills, agents and MCP servers.

**Connection**:
One signed-in account of a plugin.
_Avoid_: Integration
