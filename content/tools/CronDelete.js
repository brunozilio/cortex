/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "CronDelete",
  input: {
    "additionalProperties": false,
    "properties": {
      "id": {
        "type": "string"
      }
    },
    "required": [
      "id"
    ],
    "type": "object"
  },
  description: `Remove a scheduled prompt by its id (CronList shows them).`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
