/** @type {import("../types").ToolSpec<{ agents: { name: string, description: string }[] }>} */
export const spec = {
  name: "Agent",
  input: {
    "properties": {
      "description": {
        "description": "A short description of the task, 3 to 5 words.",
        "type": "string"
      },
      "isolation": {
        "description": "worktree: work in a git worktree of its own, kept when it changed something. Only for an agent that edits files; outside a git repository with a commit the agent runs without it.",
        "enum": [
          "worktree"
        ]
      },
      "max_turns": {
        "maximum": 200,
        "minimum": 1,
        "type": "integer"
      },
      "model": {
        "description": "A model for this agent: an id or short name your own provider serves. One it does not serve is ignored and the agent's own or yours is used, as when left out.",
        "type": "string"
      },
      "name": {
        "description": "Start a teammate under this name instead of a one-off sub-agent: it runs on its own in your team, keeps working across your turns, reports to you with SendMessage and waits idle for your next message. Letters, digits, `_` and `-`.",
        "type": "string"
      },
      "team_name": {
        "description": "Ignored: your session has one team.",
        "type": "string"
      },
      "prompt": {
        "description": "The task for the agent to carry out.",
        "type": "string"
      },
      "run_in_background": {
        "description": "Run concurrently and return a task_id. Collect its report with TaskOutput.",
        "type": "boolean"
      },
      "subagent_type": {
        "description": "The kind of agent; general-purpose when left out.",
        "type": "string"
      }
    },
    "required": [
      "description",
      "prompt"
    ],
    "type": "object"
  },
  description: (ctx) => {
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
  },
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
