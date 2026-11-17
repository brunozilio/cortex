/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "REPL",
  access: "read",
  input: {
    "additionalProperties": false,
    "properties": {
      "code": {
        "description": "JavaScript to run. Top-level await works, and what it declares stays for the next call.",
        "type": "string"
      },
      "description": {
        "description": "What the script does, in a few words of active voice, e.g. \"Count TODOs in each crate\".",
        "type": "string"
      },
      "run_in_background": {
        "description": "Run the script as a background task in its own fresh JavaScript context and go on working; collect what it returns with TaskOutput.",
        "type": "boolean"
      },
      "timeout": {
        "description": "Stop the script this many milliseconds after it starts, tool calls included (at most 600000, the default).",
        "type": "integer"
      }
    },
    "required": [
      "code"
    ],
    "type": "object"
  },
  description: `Run JavaScript that calls your other tools as functions, so that loops, filters and chains of calls happen in code instead of in turns. Each of your other tools is a global async function taking that tool's input object, e.g. \`await Grep({ pattern: "TODO", output_mode: "count" })\`; a name that is not an identifier is reached as \`tools["name"]\`. A call resolves to the tool's text output and throws a \`ToolError\` (\`.tool\` names it) when the tool fails or is refused; catch it only where the script can go on without that result. \`\` $\`git log -5\` \`\` runs a shell command through Bash, quoting the values written as \`\${...}\`.

What comes back to you is the value of the script's last expression (strings as they are, anything else as JSON, with the promises in a returned object's keys awaited) and the lines it passed to \`self.log(...)\`. Everything else stays in the script: read files, search, filter and count there, and return only what you need to read. Each part is cut to about 20,000 characters, keeping its start and its end; \`preview(value, n)\` returns the first n characters of a long value with its size. \`console.log\` goes to a ring of the last lines, read back with \`console.tail(n)\`, never to you.

Calls to read-only tools (Read, Grep, Glob, ...) started together with \`Promise.all\` run at the same time; edits, commands and anything that asks permission run one after another. Every call goes through the same permission rules, hooks and prompts as when you call the tool yourself.

The context persists: variables and functions a script declares stay for the next REPL call in this session, so keep results in variables instead of reading them again. A \`const\` cannot be declared twice; use a new name or \`globalThis.name = ...\`. The context is sealed: there is no \`import\`, \`require\`, \`process\` or \`fetch\`; reach files, commands and the web through the tools. Scripts run in strict mode, one at a time.

A script that waits on something long (a build, a test suite, a slow search) can run with \`run_in_background: true\`: it becomes a background task in a fresh context of its own, apart from your variables, and you go on working until you collect its result with TaskOutput (TaskStop ends it).

Limits: the script's own JavaScript may run for 30 seconds (time waiting on tools does not count), the whole script stops after \`timeout\` (10 minutes at most), and one script makes at most 1,000 tool calls.

Use REPL for work that would take many tool calls or that reads more than you need to see: searching across many files, applying the same edit in many places, collecting facts and returning a summary. A single call to one tool is simpler made directly.`,
};
