/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "TaskUpdate",
  access: "read",
  input: {
    "additionalProperties": false,
    "properties": {
      "blocked_by": {
        "items": {
          "type": "string"
        },
        "type": "array"
      },
      "description": {
        "type": "string"
      },
      "status": {
        "enum": [
          "pending",
          "in_progress",
          "completed"
        ]
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
  description: `Update a planning task. Status is pending, in_progress or completed. blocked_by contains task_ids that must be completed first.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
