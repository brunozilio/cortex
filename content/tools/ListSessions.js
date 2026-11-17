/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "ListSessions",
  input: {
    "additionalProperties": false,
    "properties": {},
    "type": "object"
  },
  description: `List the user's other {{NAME}} sessions open on this computer: their names and folders. Send one a message with SendMessage (\`to\` its name); its answer reaches you as an event.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
