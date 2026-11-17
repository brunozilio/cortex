/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "WebFetch",
  input: {
    "additionalProperties": false,
    "properties": {
      "prompt": {
        "description": "What to find on it.",
        "type": "string"
      },
      "url": {
        "description": "The page's full URL.",
        "type": "string"
      }
    },
    "required": [
      "url",
      "prompt"
    ],
    "type": "object"
  },
  description: `Fetch a web page (http upgraded to https) and read it as text, with what you are looking for in \`prompt\`. The main content comes first and links are numbered with their addresses listed at the end; JSON and feeds come back compacted. Long pages are cut; fetch a narrower page then. A page built only by JavaScript returns an error.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
