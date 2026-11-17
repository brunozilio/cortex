/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "Edit",
  input: {
    "additionalProperties": false,
    "properties": {
      "file_path": {
        "description": "The file to change.",
        "type": "string"
      },
      "new_string": {
        "description": "The text to put in its place.",
        "type": "string"
      },
      "old_string": {
        "description": "The exact text to replace.",
        "type": "string"
      },
      "replace_all": {
        "default": false,
        "description": "Replace every occurrence.",
        "type": "boolean"
      }
    },
    "required": [
      "file_path",
      "old_string",
      "new_string"
    ],
    "type": "object"
  },
  description: `Replace \`old_string\` with \`new_string\` in a file read earlier with Read. \`old_string\` must match the file exactly, indentation included (without Read's line-number prefix), and appear only once unless \`replace_all\` is true; include surrounding lines to make it unique. To create a file, use Write.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
