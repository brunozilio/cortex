export default (ctx) => {
  let text = `Starts a sub-agent for a task of several steps. Each kind of agent has its own abilities and tools. Leaving out \`subagent_type\` starts a fresh general-purpose agent; \`fork\` forks you, inheriting your whole conversation and your model.

When to use it: a fresh agent costs more than it looks. It knows only what you put in \`prompt\`, and you see only the report it sends back: both handoffs drop detail, and neither of you can tell what the other missed. You cannot watch it work, only wait or cancel. Its mistakes come back in the same confident tone as its findings, and an agent handed your hypothesis tends to return it confirmed. Several at once spend tokens in a burst the user did not ask for.

Reach for it when you have independent work to run in parallel, when the user asks for a side task that should not block you, or when answering means reading across many files: delegate that and you keep the conclusion, not the file dumps. Do the work yourself when it is a handful of tool calls or a lookup whose target you already know. Delegate a review only when you want a reading not anchored on yours, and then give it the code, not your conclusion. Once you delegated something, do not also do it yourself; wait for the result. When in doubt, do not start one.

Brief it like the peer it is: state the goal and what you already ruled out, point it at the files and documents worth reading instead of retyping them, and keep the scope explicit and narrow. If you cannot write a clear brief, you do not understand the task well enough to hand it off. A fork needs no background, since it has yours: give it only its directive.

- Its report is not shown to the user: pass on what matters.
- \`run_in_background\` runs it while you go on; collect its report with TaskOutput. Never make up or predict a pending agent's result; if the user asks before it arrives, say it is still running.
- Each kind's model, effort and tools come from its definition (\`{{DIR}}/agents/*.md\` frontmatter).`;
  text += "\n\nAgents available through \`subagent_type\`:\n- general-purpose: researching complex questions, searching for code and carrying out tasks of several steps (the default).";
  for (const agent of ctx.agents) {
    text += `\n- ${agent.name}: ${agent.description}`;
  }
  return text;
};
