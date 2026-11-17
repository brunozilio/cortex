/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "PushNotification",
  input: {
    "additionalProperties": false,
    "properties": {
      "message": {
        "type": "string"
      },
      "title": {
        "description": "{{NAME}} by default.",
        "type": "string"
      }
    },
    "required": [
      "message"
    ],
    "type": "object"
  },
  description: `Show the user a desktop notification: when a long task finishes, when you need a decision, or when something they asked you to watch happens and they may be away. Keep it to one short sentence; do not use it for routine progress.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
