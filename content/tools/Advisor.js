/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "Advisor",
  input: {
    "additionalProperties": false,
    "properties": {},
    "type": "object"
  },
  description: `Consult a stronger reviewer who sees your whole conversation. No parameters: your history (the task, every tool call and result) is forwarded.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
