/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "TaskList",
  input: {
    "additionalProperties": false,
    "properties": {},
    "required": [],
    "type": "object"
  },
  description: `List planning tasks and background agents for this run.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
