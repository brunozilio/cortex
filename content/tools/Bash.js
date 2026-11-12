export default (ctx) => `Run a shell command in the working directory and return its output (stdout and stderr) and exit code. Each call starts a fresh shell, so \`cd\` and variables do not carry over; prefer absolute paths. The command is stopped after \`timeout\` milliseconds (default ${ctx.default_timeout}, at most ${ctx.max_timeout}). Long output is saved whole to a file you can Read or grep, and you see its start and its end. Use Read, Glob, Grep, Edit and Write for files instead of cat, find, grep, sed or echo. Quote paths with spaces. Its output reaches you, not reliably the user. This computer runs commands with ${ctx.shell}.

Background: set \`run_in_background\` for something that keeps running (a dev server, a watcher, a long build). It returns an id at once; read what it printed with BashOutput and stop it with KillShell. It keeps running across turns until stopped or until the user leaves. Never use \`&\` or \`nohup\` for this.

Git:
- Interactive flags (\`git rebase -i\`, \`git add -i\`) do not work here: there is no terminal.
- Use the \`gh\` CLI for GitHub (pull requests, issues, the API).
- GitHub notifies the owner of any \`@name\` written in a pull request, issue or comment. Write a bare \`@name\` only for a real login you mean to notify; put roles, placeholders and code (\`@reviewer\`, \`@types/node\`) in backticks or drop the \`@\`.
- Commit or push only when the user asks. On the default branch, create a branch first.
- Never force-push, rewrite published history or skip hooks unless the user asks for that.`;
