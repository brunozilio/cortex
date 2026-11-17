/** @type {import("../types").ToolSpec<{ servers: string[] }>} */
export const spec = {
  name: "ListMcpResources",
  input: {
    type: "object",
    properties: { server: { type: "string", description: "The server whose resources to list." } },
    required: ["server"],
    additionalProperties: false,
  },
  description: (ctx) =>
    `List the resources (files, records, documents) MCP servers offer: ${ctx.servers.join(", ")}. Read one with ReadMcpResource.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
