/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "TaskCreate",
  input: {
    "additionalProperties": false,
    "properties": {
      "activeForm": {
        "description": "The subject as an ongoing action, shown while it is in progress, e.g. \"Running the tests\".",
        "type": "string"
      },
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
  description: `Create a task with a subject and description. Returns its task_id. The list belongs to the session and lasts across its turns; in a team, the lead and every teammate share it.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
