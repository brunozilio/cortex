/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "SendMessage",
  access: "read",
  input: {
    "properties": {
      "message": {
        "description": "What to tell or ask the agent.",
        "type": "string"
      },
      "to": {
        "description": "The agent_id an Agent report gave (agent-1), or another session's name (ListSessions).",
        "type": "string"
      }
    },
    "required": [
      "to",
      "message"
    ],
    "type": "object"
  },
  description: `Continues a sub-agent you started earlier in this request, with its conversation intact: it remembers its task, what it read and what it reported. \`to\` is the agent_id at the end of its report. Use it for a follow-up, a correction or the next step of the same work, instead of starting a fresh agent and briefing it again. The agent answers with a new report, which, like the first, is not shown to the user.

- An agent still running in the background cannot take a message: collect it with TaskOutput first.
- Agents last for this request only; start a new one for later requests.
- \`to\` can also name another of the user's open sessions (ListSessions lists them): the message waits in its inbox and reaches its agent; any answer comes back to you as an event.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
