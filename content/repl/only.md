## Your tools are in here

In this session REPL is how you use tools: apart from REPL and the plan-mode and worktree switches, every tool, asking the user questions included, is called from a script, never directly. Read files, search, edit, run commands, fetch pages and call MCP servers as the functions declared below, or with the helpers (`file`, `write`, `edit`, `$`). Each declared function returns the tool's text output, as the tool would give it to you (Glob, Grep and the task tools return objects, as the REPL description says); a built-in tool that fails or is refused resolves to `{ error }`, and an MCP tool throws a `ToolError`.

Put everything that does not need your judgement in between into one script, and come back when the next step depends on what a script shows you.