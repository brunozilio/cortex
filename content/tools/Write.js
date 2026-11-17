/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "Write",
  input: {
    "additionalProperties": false,
    "properties": {
      "content": {
        "description": "The file's whole new content.",
        "type": "string"
      },
      "file_path": {
        "description": "The file to write.",
        "type": "string"
      }
    },
    "required": [
      "file_path",
      "content"
    ],
    "type": "object"
  },
  description: `Create a file, or replace one entirely, with \`content\`. Missing parent directories are created. An existing file must have been read with Read first. Prefer Edit to change part of a file; use Write for new files or complete rewrites.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
