/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "LSP",
  input: {
    "additionalProperties": false,
    "properties": {
      "character": {
        "description": "Column of the symbol, from 1.",
        "type": "integer"
      },
      "file_path": {
        "description": "The file the question is about.",
        "type": "string"
      },
      "line": {
        "description": "Line of the symbol, from 1.",
        "type": "integer"
      },
      "operation": {
        "enum": [
          "goToDefinition",
          "findReferences",
          "hover",
          "documentSymbol",
          "workspaceSymbol",
          "goToImplementation",
          "incomingCalls",
          "outgoingCalls",
          "diagnostics",
          "workspaceDiagnostics"
        ],
        "type": "string"
      },
      "query": {
        "description": "The name to find (workspaceSymbol).",
        "type": "string"
      }
    },
    "required": [
      "operation",
      "file_path"
    ],
    "type": "object"
  },
  description: `Ask the file's language server about code: \`goToDefinition\`, \`findReferences\`, \`goToImplementation\`, \`hover\` (type and docs), \`incomingCalls\`/\`outgoingCalls\` (who calls a function, what it calls) at a position; \`documentSymbol\` (a file's outline) or \`workspaceSymbol\` (find a symbol by name across the project, with \`query\`). \`line\` and \`character\` count from 1, as Read shows them; point at the symbol's name. Prefer this over Grep to find where a symbol is defined or used: it follows the language's rules, not the text. To find a function or type by name, or to list a project's declarations, ask \`workspaceSymbol\` instead of a Grep for the language's keywords. Before reading a long file, ask \`documentSymbol\` and Read only the part you need; a short file is quicker read whole. \`diagnostics\` gives the file's current errors and warnings as the server sees them (the file is sent as it is on disk first); \`workspaceDiagnostics\` gives every error and warning the file's server has reported for the project so far, not only for that file. Both answer as one line per problem, errors first. Ask them after a change you are unsure of, instead of running a full build. The first question to a project can wait while its server starts.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
