/** @type {import("../types").ToolSpec<{ default_timeout: number, max_timeout: number, OUTPUT: number, shell: string }>} */
export const spec = {
  name: "Bash",
  input: {
    "additionalProperties": false,
    "properties": {
      "command": {
        "description": "The command to run.",
        "type": "string"
      },
      "description": {
        "description": "What the command does, in plain active words the user reads, often without seeing the command: do not echo its text, flags or paths. Five to ten words for simple commands (`git status` → \"Show working tree status\"); more context for pipes or obscure flags (`git reset --hard origin/main` → \"Discard all local changes and match remote main\"). Never call a command \"complex\" or \"risky\": say what it does.",
        "type": "string"
      },
      "run_in_background": {
        "description": "Leave it running and return its id at once.",
        "type": "boolean"
      },
      "timeout": {
        "description": "Milliseconds before it is stopped.",
        "type": "integer"
      },
      "unsandboxed": {
        "description": "Run outside the sandbox. Only after the sandbox refused a command the task truly needs; the user is always asked.",
        "type": "boolean"
      }
    },
    "required": [
      "command"
    ],
    "type": "object"
  },
  description: (ctx) => `Run a shell command in the working directory and return its output (stdout and stderr) and exit code. Each call starts a fresh shell, so \`cd\` and variables do not carry over; prefer absolute paths. The command is stopped after \`timeout\` milliseconds (default ${ctx.default_timeout}, at most ${ctx.max_timeout}). Output past ${ctx.OUTPUT} characters (unless the user set another limit) is cut: you see its start and its end, under a first line saying how much was cut and the file that holds all of it, which you can Read or Grep. Use Read, Glob, Grep, Edit and Write for files instead of cat, find, grep, sed or echo. Quote paths with spaces. Its output reaches you, not reliably the user. This computer runs commands with ${ctx.shell}.

Background: set \`run_in_background\` for something that keeps running (a dev server, a watcher, a long build). It returns an id at once; read what it printed with BashOutput and stop it with KillShell. It keeps running across turns until stopped or until the user leaves. Never use \`&\` or \`nohup\` for this.

Git:
- Interactive flags (\`git rebase -i\`, \`git add -i\`) do not work here: there is no terminal.
- Use the \`gh\` CLI for GitHub (pull requests, issues, the API).
- GitHub notifies the owner of any \`@name\` written in a pull request, issue or comment. Write a bare \`@name\` only for a real login you mean to notify; put roles, placeholders and code (\`@reviewer\`, \`@types/node\`) in backticks or drop the \`@\`.
- Commit or push only when the user asks. On the default branch, create a branch first.
- Never force-push, rewrite published history or skip hooks unless the user asks for that.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
