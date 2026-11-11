const modes = {
  plan: "prompts/mode-plan.md",
  ask: "prompts/mode-ask.md",
  edits: "prompts/mode-edits.md",
  bypass: "prompts/mode-bypass.md",
};

export default function reminder(ctx) {
  const sub = ctx.depth > 0;
  const mode = ctx.mode === "plan" && sub ? ctx.text("prompts/mode-plan-sub-agent.md") : ctx.text(modes[ctx.mode]);
  const role = ctx.text(sub ? "prompts/reminder-sub-agent.md" : "prompts/reminder-main.md");
  let text = `\n\n# Runtime reminder\n${mode}\n${role}`;
  if (ctx.style && !sub) {
    text += `\n${ctx.style}`;
  }
  return text;
}
