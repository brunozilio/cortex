/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "ScheduleWakeup",
  input: {
    "additionalProperties": false,
    "properties": {
      "delaySeconds": {
        "type": "number"
      },
      "prompt": {
        "type": "string"
      },
      "reason": {
        "description": "Why this delay, in one sentence.",
        "type": "string"
      }
    },
    "required": [
      "delaySeconds",
      "prompt"
    ],
    "type": "object"
  },
  description: `Send a prompt to this session once, after a delay (60 to 3600 seconds), to pick up work later: check on a long build, a deploy, or repeat a task at your own pace by scheduling the next wakeup each time. It fires only while the session is idle. Each wakeup is a model turn: to wait until a tool's result meets a fixed condition (an issue reaching a status, a file changing), use Watch, which checks without you; to follow a command's output, Monitor.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
