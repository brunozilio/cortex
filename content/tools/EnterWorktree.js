/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "EnterWorktree",
  input: {
    "additionalProperties": false,
    "properties": {},
    "type": "object"
  },
  description: `Move this session into a new git worktree of the project (a separate checkout of the current commit, HEAD, under {{DIR}}/worktrees; uncommitted changes stay behind), to work on a change apart from the user's checkout. From the next step on, files and commands are in the worktree, and later prompts of the session too, until ExitWorktree. Use it only when the user asks for a worktree or for working apart.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
