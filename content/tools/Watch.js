/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "Watch",
  input: {
    "additionalProperties": false,
    "properties": {
      "contains": {
        "description": "End when the value contains this text (case ignored).",
        "type": "string"
      },
      "description": {
        "description": "What it waits for, in a few words.",
        "type": "string"
      },
      "equals": {
        "anyOf": [
          {
            "type": "string"
          },
          {
            "items": {
              "type": "string"
            },
            "type": "array"
          }
        ],
        "description": "End when the value equals this (case ignored); a list ends on any of them."
      },
      "every_seconds": {
        "description": "Seconds between checks (default 300).",
        "maximum": 86400,
        "minimum": 60,
        "type": "integer"
      },
      "input": {
        "description": "The arguments for that tool, as you would call it.",
        "type": "object"
      },
      "path": {
        "description": "Dot path into the JSON the tool returns, e.g. data.fields.status.name; without it the whole result is compared.",
        "type": "string"
      },
      "tool": {
        "description": "The read-only tool to call on each check, e.g. an MCP tool such as mcp__server__get_issue.",
        "type": "string"
      }
    },
    "required": [
      "tool"
    ],
    "type": "object"
  },
  description: `Wait for something to change without spending model turns: the app calls a read-only tool on a fixed pace (an MCP tool such as an issue tracker's "get issue", or Read, Grep, Glob) and compares one value of its result, and you hear back once, as an event, when the condition is met or when the watch gives up (three failed checks in a row, or 7 days). Between checks nothing reaches you, so it costs nothing while it waits.

Choose by what the check needs:
- A fixed condition on what a tool returns ("tell me when KAN-1 is done", "when this file shows READY"): Watch. Point \`path\` at the field (a dot path into the JSON, e.g. \`data.fields.status.name\` or \`fields.statusCategory.key\`), and give \`equals\` (one value or a list, case ignored) or \`contains\`; with neither, any change of the value ends the watch. Prefer a stable field (a status category or key) over a translated label, or list every label that counts.
- A shell command's output (a log, a build, \`gh pr checks\`): Monitor.
- A check that needs judgement each time (reading and deciding, summarising, acting): ScheduleWakeup or CronCreate, which run you on each check.

The first check runs now, as an ordinary call of that tool, so it follows the permission mode and rules like one: a rule or an earlier "always allow" for that tool covers the watch, a deny rule refuses it, and otherwise the user is asked, seeing what the watch waits for. If the condition already holds, nothing is watched. Load an MCP tool with ToolSearch before watching it. Stop a watch with KillShell and its id.

Once a watch is started, end your turn: its event arrives as a new message even while the chat is idle. Do not add ScheduleWakeup, CronCreate or a second watch for the same thing, and leave whatever the user asked to happen afterwards ("then commit", "then deploy") until the event says the condition was met.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
