const scopes = {
  user: "the user's private instructions for every project",
  project: "the project's instructions, kept in the repository",
  local: "the user's private instructions for this project",
};

export default function system(ctx) {
  const sections = [ctx.text("prompts/role.js"), ctx.text("prompts/harness.js")];
  if (ctx.tools.some((tool) => tool.startsWith(`mcp__${ctx.brand.COMMAND}_memory__`))) {
    sections.push(ctx.text("prompts/memory.js"));
  }
  sections.push(ctx.text("prompts/context.js"));
  if (!ctx.output_style || ctx.output_style.keep_coding) {
    sections.push(ctx.text("prompts/delivery.js"), ctx.text("prompts/writing.js"));
  }
  if (ctx.tools.includes("Advisor")) {
    sections.push(ctx.text("prompts/advisor.js"));
  }
  let prompt = sections.join("\n\n");
  prompt += `\n\nPrompt contract: ${ctx.contract}`;
  if (ctx.output_style) {
    prompt += `\n\n# Output style: ${ctx.output_style.name}\n${ctx.output_style.instructions}`;
  }
  if (ctx.skills) {
    prompt += `\n\n${ctx.text("prompts/skills.js")}\n${ctx.skills}`;
  }
  if (ctx.instructions.length > 0) {
    prompt += `\n\n${ctx.text("prompts/instructions.js")}`;
    for (const file of ctx.instructions) {
      prompt += `\n\nContents of ${file.path} (${scopes[file.scope]}):\n\n${file.text.trim()}`;
    }
  }
  if (ctx.mcp_instructions.length > 0) {
    prompt +=
      "\n\n# MCP server instructions\n\nThe connected MCP servers below describe how to use their tools and what they send you.";
    for (const served of ctx.mcp_instructions) {
      prompt += `\n\n## ${served.server}\n${served.text}`;
    }
  }
  prompt += "\n\n# Environment\n";
  prompt += `- Working directory: ${ctx.cwd}\n`;
  if (ctx.added_dirs.length > 0) {
    prompt += `- Additional working directories: ${ctx.added_dirs.join(", ")}\n`;
  }
  prompt += `- Git repository: ${ctx.git ? "yes" : "no"}\n`;
  prompt += `- Platform: ${ctx.platform}\n`;
  prompt += ctx.time_zone ? `- Today: ${ctx.date} (time zone ${ctx.time_zone})\n` : `- Today: ${ctx.date}\n`;
  if (ctx.top_level && ctx.top_level.length === 0) {
    prompt +=
      "- The working directory was empty when the conversation started (no files or folders except hidden ones starting with `.`), so there is nothing in it to list, search or read: do not call Glob, Grep or Read on it, nor start an agent to explore it, before you have created files. This is not updated as you work.\n";
  } else if (ctx.top_level) {
    prompt += `- Top level of the working directory when the conversation started (hidden entries left out, not updated as you work): ${ctx.top_level.join(", ")}\n`;
  }
  if (ctx.model) {
    prompt += `- Model: ${ctx.model}\n`;
  }
  if (ctx.language_servers.length > 0) {
    const servers = ctx.language_servers.map((server) => `${server.name} (${server.extensions.join(", ")})`);
    prompt += `- Language servers (the LSP tool; prefer it to Grep for definitions, references, callers and symbols, and its documentSymbol outline to reading a long file whole): ${servers.join(", ")}\n`;
  }
  if (ctx.git) {
    prompt += ctx.include("prompts/git.js");
  }
  return prompt;
}
