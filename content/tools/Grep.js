/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "Grep",
  input: {
    "additionalProperties": false,
    "properties": {
      "-A": {
        "description": "Lines after each match (content mode).",
        "type": "integer"
      },
      "-B": {
        "description": "Lines before each match (content mode).",
        "type": "integer"
      },
      "-C": {
        "description": "Lines around each match (content mode).",
        "type": "integer"
      },
      "-i": {
        "description": "Ignore case.",
        "type": "boolean"
      },
      "-n": {
        "description": "Number lines (content mode).",
        "type": "boolean"
      },
      "glob": {
        "description": "Only files whose name matches this glob.",
        "type": "string"
      },
      "head_limit": {
        "description": "Show the first N results (default 200).",
        "type": "integer"
      },
      "multiline": {
        "description": "Let a match span lines; `.` then matches line ends too.",
        "type": "boolean"
      },
      "output_mode": {
        "enum": [
          "content",
          "files_with_matches",
          "count"
        ],
        "type": "string"
      },
      "path": {
        "description": "File or directory to search; the working directory if omitted.",
        "type": "string"
      },
      "pattern": {
        "description": "The regular expression.",
        "type": "string"
      }
    },
    "required": [
      "pattern"
    ],
    "type": "object"
  },
  description: `Search file contents with a regular expression (Rust regex syntax, like ripgrep). Files ignored by git, \`.ignore\` or \`.rgignore\` are skipped; hidden files are searched, the \`.git\` folder is not. \`output_mode\` is \`files_with_matches\` (default: the matching files), \`content\` (the matching lines; \`-n\` numbers them, \`-A\`/\`-B\`/\`-C\` add context) or \`count\`. Narrow with \`path\` and \`glob\` (e.g. \`*.rs\`). \`-i\` ignores case. Each line is matched on its own unless \`multiline\` is set: then a match can span lines, \`.\` matches line ends too, and \`content\` shows every line a match covers. Matching lines come grouped under their file's path; a file shows its first 25 matches, a line its first 300 characters, and the whole answer the first 200 results (\`head_limit\` changes that), each saying how much was left out. Always use this instead of running grep or rg through Bash.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
