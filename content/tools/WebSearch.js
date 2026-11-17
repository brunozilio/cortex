/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "WebSearch",
  input: {
    "additionalProperties": false,
    "properties": {
      "query": {
        "description": "What to search for.",
        "type": "string"
      }
    },
    "required": [
      "query"
    ],
    "type": "object"
  },
  description: `Search the web for current information. The search runs at the model provider and its results arrive in your context with their sources. Fetch a page with WebFetch to read it in full.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
