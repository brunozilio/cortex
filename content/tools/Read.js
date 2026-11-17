/** @type {import("../types").ToolSpec<{ DEFAULT_LINES: number, LINE_CHARS: number, MAX_CHARS: number }>} */
export const spec = {
  name: "Read",
  input: {
    "additionalProperties": false,
    "properties": {
      "file_path": {
        "description": "The file to read.",
        "type": "string"
      },
      "limit": {
        "description": "How many lines to read.",
        "type": "integer"
      },
      "offset": {
        "description": "First line to read, from 1.",
        "type": "integer"
      },
      "pages": {
        "description": "A PDF's pages, like \"3\" or \"1-5\".",
        "type": "string"
      }
    },
    "required": [
      "file_path"
    ],
    "type": "object"
  },
  description: (ctx) => `Read a text file, a PNG, JPEG, GIF or WebP image, a PDF (as text; \`pages\` like "1-5" for a long one) or a Jupyter notebook (its cells and outputs) from the machine. Images are sent visually to the model. Give an absolute path, or one relative to the working directory. Returns up to ${ctx.DEFAULT_LINES} lines from the start, each prefixed with its line number and a tab; pass \`offset\` (first line, from 1) and \`limit\` to read a part of a long file. Lines longer than ${ctx.LINE_CHARS} characters are cut, and a read stops at the last whole line before ${ctx.MAX_CHARS} characters, saying where to go on. Reading lines of an unchanged file that an earlier read already returned gives a short note instead of the text. When you know which part of a file you need, read only that part. Read a file before editing or overwriting it, but do not read it again to check an edit: Edit and Write fail when a change does not apply. To find files, use Glob; to search inside them, Grep. Several reads can be requested at once.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
