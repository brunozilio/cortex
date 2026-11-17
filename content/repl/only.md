## Your tools are in here

In this session REPL is how you use tools: apart from REPL, asking the user questions and the plan-mode and worktree switches, every tool is called from a script, never directly. Read files, search, edit, run commands, fetch pages and call MCP servers as the functions declared below; each returns the tool's text output, as the tool would give it to you, and throws a `ToolError` when it fails or is refused.

Besides them: `` $`git status` `` runs a shell command through Bash (values written as `${...}` are quoted for the shell) and throws when the command exits with an error. Put everything that does not need your judgement in between into one script, and come back when the next step depends on what a script shows you.