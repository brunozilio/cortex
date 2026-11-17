/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "ExitWorktree",
  input: {
    "additionalProperties": false,
    "properties": {
      "action": {
        "enum": [
          "keep",
          "remove"
        ],
        "type": "string"
      },
      "discard_changes": {
        "type": "boolean"
      }
    },
    "required": [
      "action"
    ],
    "type": "object"
  },
  description: `Leave the worktree EnterWorktree made and go back to the project's checkout. action "keep" leaves the worktree on disk (its path is reported); "remove" deletes it, which is refused while it has uncommitted files or commits of its own unless discard_changes is true.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
