/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "NotebookEdit",
  input: {
    "additionalProperties": false,
    "properties": {
      "cell_number": {
        "description": "The cell, from 0.",
        "type": "integer"
      },
      "cell_type": {
        "enum": [
          "code",
          "markdown"
        ]
      },
      "edit_mode": {
        "enum": [
          "replace",
          "insert",
          "delete"
        ]
      },
      "new_source": {
        "type": "string"
      },
      "notebook_path": {
        "type": "string"
      }
    },
    "required": [
      "notebook_path"
    ],
    "type": "object"
  },
  description: `Change one cell of a Jupyter notebook (.ipynb): \`replace\` its source (the default), \`insert\` a new cell after it (or first, without \`cell_number\`), or \`delete\` it. Cells are numbered from 0 as Read shows them. Read the notebook first.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
