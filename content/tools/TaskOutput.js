/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "TaskOutput",
  input: {
    "additionalProperties": false,
    "properties": {
      "block": {
        "type": "boolean"
      },
      "task_id": {
        "type": "string"
      }
    },
    "required": [
      "task_id"
    ],
    "type": "object"
  },
  description: `Collect a background agent's report. block defaults to true; false polls without waiting.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
