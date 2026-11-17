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
  description: `Run JavaScript that calls your other tools as functions, so that loops, filters and chains of calls happen in code instead of in turns. Each of your other tools is a global async function taking that tool's input object, e.g. \`await Grep({ pattern: "TODO", output_mode: "count" })\`; a name that is not an identifier is reached as \`tools["name"]\`. A call resolves to the tool's text output. When a built-in tool fails or is refused, the call resolves to \`{ error: "why" }\` instead; an MCP tool's call throws a \`ToolError\` (\`.tool\` names it). Check for \`error\` and catch only where the script can go on without that result, and never treat a failure as success.

Helpers for the common cases, each going through the matching tool (and its permission rules and hooks) and throwing a \`ToolError\` when it fails:
- \`await file(path).text()\`, \`.json()\`, \`.lines()\`, \`.exists()\`: the file's whole text, as it is on disk (no line numbers, nothing cut).
- \`await write(path, text)\` writes a file; \`await edit(path, oldText, newText, { all: true })\` replaces text that must match exactly once, or every match with \`all\`.
- \`\` await $\`git log -5\` \`\` runs a shell command through Bash (values written as \`\${...}\` are quoted for the shell) and throws when it exits with an error; \`\` (await $\`cmd\`.nothrow()).exitCode \`\` checks it instead.

What comes back to you is the value of the script's last expression (strings as they are, anything else as JSON, with the promises in a returned object's keys awaited) and the lines it passed to \`self.log(...)\`. Everything else stays in the script: read files, search, filter and count there, and return only what you need to read. Each part is cut to about 20,000 characters, keeping its start and its end; \`preview(value, n)\` returns the first n characters of a long value with its size. \`console.log\` goes to a ring of the last lines, read back with \`console.tail(n)\`, never to you. Calls refused while a script ran are listed after its output.

Calls to read-only tools (Read, Grep, Glob, ...) started together with \`Promise.all\` run at the same time; edits, commands and anything that asks permission run one after another. Every call goes through the same permission rules, hooks and prompts as when you call the tool yourself.

The context persists: variables and functions a script declares stay for the next REPL call in this session, so keep results in variables instead of reading them again. A \`const\` cannot be declared twice; use a new name or \`globalThis.name = ...\`. The context is sealed: there is no \`import\`, \`require\`, \`process\` or \`fetch\`; reach files, commands and the web through the tools. Scripts run in strict mode.

A script that is still running after about 30 seconds is dispatched: the call returns \`(dispatched #N)\` with a task id, and the script goes on beside the scripts you send next, which share its variables. Its output comes back through TaskOutput, or by itself before your turn ends, so put the work that waits (a build, a test suite) in one script and go on with other things; send a script that depends on a dispatched one's result only after that result is back.

Limits: the script's own JavaScript may run for 30 seconds without a break (time waiting on tools does not count), the whole script stops after \`timeout\` (10 minutes at most), and one script makes at most 1,000 tool calls.

Use REPL for work that would take many tool calls or that reads more than you need to see: searching across many files, applying the same edit in many places, collecting facts and returning a summary. A single call to one tool is simpler made directly.`,
};
