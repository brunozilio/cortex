/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "Skill",
  input: {
    "properties": {
      "args": {
        "description": "Optional arguments for the skill.",
        "type": "string"
      },
      "skill": {
        "description": "The skill's name, without a leading slash.",
        "type": "string"
      }
    },
    "required": [
      "skill"
    ],
    "type": "object"
  },
  description: `Loads a skill's instructions by its exact name from the list of available skills. When a skill covers the task, call this first and follow what it returns.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
