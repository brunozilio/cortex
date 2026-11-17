/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "TaskStop",
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
  description: `Cancel a background agent and wait for its tools to stop.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
