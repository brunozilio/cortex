/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "TaskCreate",
  access: "read",
  input: {
    "additionalProperties": false,
    "properties": {
      "description": {
        "type": "string"
      },
      "subject": {
        "type": "string"
      }
    },
    "required": [
      "subject"
    ],
    "type": "object"
  },
  description: `Create a planning task with a subject and description. Returns a run-local task_id.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
