/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "TaskUpdate",
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
      "owner": {
        "description": "Who works on it: your own name to claim it, a teammate's name to give it to them (they are told), or empty to hand it back.",
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
  description: `Update a task. Status is pending, in_progress or completed; a task cannot start or end while a task in its blocked_by is open. blocked_by lists the task_ids that must be completed first (each task shows the ones it blocks). owner claims a task: a task someone else holds cannot be claimed, except by the lead, and a teammate that stops hands its open tasks back.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
