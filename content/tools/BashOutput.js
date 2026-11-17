/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "BashOutput",
  input: {
    "additionalProperties": false,
    "properties": {
      "bash_id": {
        "description": "The id Bash returned.",
        "type": "string"
      },
      "filter": {
        "description": "Keep only lines containing this.",
        "type": "string"
      }
    },
    "required": [
      "bash_id"
    ],
    "type": "object"
  },
  description: `Read what a background command (Bash with run_in_background) printed since you last read it, and whether it is still running. Pass \`filter\` (a substring) to keep only the lines that contain it.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
