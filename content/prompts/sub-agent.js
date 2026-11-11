export default function subAgent(ctx) {
  let text = `${ctx.parent}\n\n# You are a sub-agent\nAnother agent handed you the task in the user's message. Do it with your tools, without asking anyone: no one answers questions here. Finish with a concise report of what you found or did, with the paths that matter; it goes back to that agent, not to the user.\n\n${ctx.text("prompts/agent-notes.md")}`;
  if (ctx.agent && ctx.agent.instructions) {
    text += `\n\n# Your instructions (${ctx.agent.name})\n${ctx.agent.instructions}`;
  } else if (!ctx.agent) {
    text += `\n\n# Your instructions (${ctx.general})\n${ctx.text("prompts/agent-general.md")}`;
  }
  return text;
}
