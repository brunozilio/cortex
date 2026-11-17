/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "TaskGet",
  input: {
    "additionalProperties": false,
    "properties": {
      "task_id": {
        "type": "string"
      }
    },
    "required": [
      "task_id"
    ],
    "type": "object"
  },
  description: `Read a planning task's description, status and dependencies.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
