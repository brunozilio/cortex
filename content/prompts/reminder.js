const modes = {
  plan: "prompts/mode-plan.js",
  ask: "prompts/mode-ask.js",
  edits: "prompts/mode-edits.js",
  bypass: "prompts/mode-bypass.js",
};

export default function reminder(ctx) {
  const sub = ctx.depth > 0;
  const mode = ctx.mode === "plan" && sub ? ctx.text("prompts/mode-plan-sub-agent.js") : ctx.text(modes[ctx.mode]);
  const role = ctx.text(sub ? "prompts/reminder-sub-agent.js" : "prompts/reminder-main.js");
  let text = `\n\n# Runtime reminder\n${mode}\n${role}`;
  if (ctx.style && !sub) {
    text += `\n${ctx.style}`;
  }
  return text;
}
