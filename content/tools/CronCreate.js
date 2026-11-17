/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "CronCreate",
  input: {
    "additionalProperties": false,
    "properties": {
      "cron": {
        "type": "string"
      },
      "prompt": {
        "description": "What to send when it fires.",
        "type": "string"
      },
      "recurring": {
        "description": "Every match (default) or only the next.",
        "type": "boolean"
      }
    },
    "required": [
      "cron",
      "prompt"
    ],
    "type": "object"
  },
  description: `Schedule a prompt to be sent to this session later, on a cron expression in the user's local time (minute hour day-of-month month day-of-week, e.g. "*/10 * * * *", "0 9 * * 1-5"). It fires only while the session is open and idle. Recurring prompts end after 7 days. Use it when the user asks for something periodic or at a set time that needs your judgement each time. Each firing is a model turn: to be told when a tool's result meets a fixed condition ("let me know when the ticket is done"), use Watch, which checks without you.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
