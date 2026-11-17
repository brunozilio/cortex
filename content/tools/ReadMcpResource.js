/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "ReadMcpResource",
  input: {
    type: "object",
    properties: { server: { type: "string" }, uri: { type: "string" } },
    required: ["server", "uri"],
    additionalProperties: false,
  },
  description: `Read a resource of an MCP server by its URI (ListMcpResources lists them).`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
