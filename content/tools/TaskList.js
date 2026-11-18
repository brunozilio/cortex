/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "TaskList",
  input: {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  description: `List the session's tasks (shared by the team, with owners and what blocks what) and the background agents of this run.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
