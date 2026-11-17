/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "CronList",
  input: {
    "additionalProperties": false,
    "properties": {},
    "type": "object"
  },
  description: `List the prompts scheduled for this session (CronCreate, ScheduleWakeup).`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
