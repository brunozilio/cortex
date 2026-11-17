/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "EnterPlanMode",
  input: {
    "additionalProperties": false,
    "properties": {},
    "type": "object"
  },
  description: `Turn on planning mode before a non-trivial change: a new feature, several valid approaches, changes across many files, or requirements you need to explore first. While planning you only read and search, then present the plan with ExitPlanMode for the user's approval. Skip it for small, obvious changes, for questions and research, and when the user has asked you to just do it.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
