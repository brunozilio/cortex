const scopes = {
  user: "the user's private instructions for every project",
  project: "the project's instructions, kept in the repository",
  local: "the user's private instructions for this project",
};

export default function system(ctx) {
  const sections = [ctx.text("prompts/role.md"), ctx.text("prompts/harness.md")];
  if (ctx.tools.some((tool) => tool.startsWith(`mcp__${ctx.brand.COMMAND}_memory__`))) {
    sections.push(ctx.text("prompts/memory.md"));
  }
  sections.push(ctx.text("prompts/context.md"));
  if (!ctx.output_style || ctx.output_style.keep_coding) {
    sections.push(ctx.text("prompts/delivery.md"), ctx.text("prompts/writing.md"));
  }
  if (ctx.tools.includes("Advisor")) {
    sections.push(ctx.text("prompts/advisor.md"));
  }
  let prompt = sections.join("\n\n");
  prompt += `\n\nPrompt contract: ${ctx.contract}`;
  if (ctx.output_style) {
    prompt += `\n\n# Output style: ${ctx.output_style.name}\n${ctx.output_style.instructions}`;
  }
  if (ctx.skills) {
    prompt += `\n\n${ctx.text("prompts/skills.md")}\n${ctx.skills}`;
  }
  if (ctx.instructions.length > 0) {
    prompt += `\n\n${ctx.text("prompts/instructions.md")}`;
    for (const file of ctx.instructions) {
      prompt += `\n\nContents of ${file.path} (${scopes[file.scope]}):\n\n${file.text.trim()}`;
    }
  }
  prompt += "\n\n# Environment\n";
  prompt += `- Working directory: ${ctx.cwd}\n`;
  if (ctx.added_dirs.length > 0) {
    prompt += `- Additional working directories: ${ctx.added_dirs.join(", ")}\n`;
  }
  prompt += `- Git repository: ${ctx.git ? "yes" : "no"}\n`;
  prompt += `- Platform: ${ctx.platform}\n`;
  prompt += `- Today: ${ctx.date}\n`;
  if (ctx.model) {
    prompt += `- Model: ${ctx.model}\n`;
  }
  if (ctx.git) {
    prompt += ctx.include("prompts/git.js");
  }
  return prompt;
}
