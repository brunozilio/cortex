/** @type {import("../types").ToolSpec<{ LIMIT: number }>} */
export const spec = {
  name: "Glob",
  input: {
    "additionalProperties": false,
    "properties": {
      "path": {
        "description": "The directory to search; the working directory if omitted.",
        "type": "string"
      },
      "pattern": {
        "description": "The glob to match paths against.",
        "type": "string"
      }
    },
    "required": [
      "pattern"
    ],
    "type": "object"
  },
  description: (ctx) => `Find files by name with a glob pattern such as \`**/*.rs\` or \`src/**/mod.rs\`, searching the working directory or \`path\`. Files ignored by git are skipped. Returns up to ${ctx.LIMIT} paths, most recently modified first. Use Grep to search file contents.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
