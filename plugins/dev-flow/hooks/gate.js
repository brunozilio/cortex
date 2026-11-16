export const event = "PreToolUse";
export const matcher = "^(Write|Edit|Bash|mcp__.*)$";
export const timeout = 30;

export const stages = ["refine", "spec", "plan", "implement", "pr", "review", "merge", "qa", "done"];

export const gates = {
  refine: "ask",
  spec: "ask",
  plan: "ask",
  implement: "ask",
  pr: "ask",
  review: "ask",
  merge: "ask",
  qa: "ask",
  done: "ask",
};

export const guards = [
  { name: "Creating the Jira story", tool: /createJiraIssue$/, decision: "ask" },
  { name: "Moving the Jira issue", tool: /transitionJiraIssue$/, stage: "implement" },
  { name: "Opening the pull request", tool: /^Bash$/, command: /\bgh\s+pr\s+create\b/, stage: "pr" },
  { name: "Merging the pull request", tool: /^Bash$/, command: /\bgh\s+pr\s+merge\b/, stage: "merge" },
  {
    name: "Writing to Zephyr",
    tool: /^Bash$/,
    command: /zephyrscale\.smartbear\.com[\s\S]*(\s-X\s*(POST|PUT|DELETE)|\s(-d|--data|--data-raw|-F|--form)\s)|(\s-X\s*(POST|PUT|DELETE)|\s(-d|--data|--data-raw|-F|--form)\s)[\s\S]*zephyrscale\.smartbear\.com/,
    stage: "plan",
  },
  { name: "Changing infrastructure", tool: /^Bash$/, command: /\bterraform\s+(apply|destroy)\b/, decision: "ask" },
];

const ISSUE = /[A-Z][A-Z0-9]+-\d+/;
const STATE = /(?:^|\/)\.flow\/([A-Z][A-Z0-9]+-\d+)\.json$/;

const answer = (permissionDecision, permissionDecisionReason) => ({
  hookSpecificOutput: { permissionDecision, permissionDecisionReason },
});

function read(ctx, path) {
  try {
    return JSON.parse(ctx.readFile(path));
  } catch (error) {
    return null;
  }
}

function run(ctx, command) {
  try {
    const result = ctx.exec(command, { timeout: 10 });
    return result.code === 0 ? result.stdout.trim() : "";
  } catch (error) {
    return "";
  }
}

function decided(gate, change) {
  const decision = typeof gate === "function" ? gate(change) : gate;
  return ["allow", "deny"].includes(decision) ? decision : "ask";
}

function stateChange(input, ctx) {
  const path = String(input.tool_input.file_path ?? "");
  const found = STATE.exec(path);
  if (!found) return undefined;
  const issue = found[1];
  if (input.tool_name === "Edit") {
    return answer("deny", `Replace ${path} whole with Write, so the gate reads the stage it moves to.`);
  }
  let next;
  try {
    next = JSON.parse(String(input.tool_input.content ?? ""));
  } catch (error) {
    return answer("deny", `${path} must hold JSON: ${error}`);
  }
  if (next.issue !== issue) return answer("deny", `${path} must say "issue": "${issue}".`);
  const to = stages.indexOf(next.stage);
  if (to < 0) return answer("deny", `Unknown stage "${next.stage}"; the stages are ${stages.join(", ")}.`);
  const current = read(ctx, path);
  const from = current ? stages.indexOf(current.stage) : -1;
  if (current && from === to) return undefined;
  if (current && to > from + 1) {
    return answer("deny", `One stage at a time: after "${current.stage}" comes "${stages[from + 1]}".`);
  }
  const change = { issue, from: current ? current.stage : null, to: next.stage, state: next, previous: current, ctx };
  const decision = decided(gates[next.stage], change);
  const moved = `${issue}: ${change.from ?? "start"} -> ${change.to}`;
  if (decision === "deny") return answer("deny", `The gate refuses ${moved}.`);
  return answer(decision, `Approve ${moved}? ${next.approval ?? ""}`.trim());
}

function issueOf(input, ctx) {
  const call = input.tool_input;
  for (const field of ["issueIdOrKey", "issueKey", "key"]) {
    const found = ISSUE.exec(String(call[field] ?? ""));
    if (found) return found[0];
  }
  const branch = ISSUE.exec(run(ctx, "git branch --show-current"));
  if (branch) return branch[0];
  const open = run(ctx, "ls .flow")
    .split("\n")
    .map((name) => /^([A-Z][A-Z0-9]+-\d+)\.json$/.exec(name))
    .filter(Boolean)
    .map((found) => found[1])
    .filter((issue) => (read(ctx, `${input.cwd}/.flow/${issue}.json`) ?? {}).stage !== "done");
  return open.length === 1 ? open[0] : null;
}

function guarded(input, ctx) {
  const command = String(input.tool_input.command ?? "");
  const guard = guards.find(
    (candidate) => candidate.tool.test(input.tool_name) && (!candidate.command || candidate.command.test(command)),
  );
  if (!guard) return undefined;
  if (guard.decision) return answer(guard.decision, `${guard.name} always needs this decision.`);
  const issue = issueOf(input, ctx);
  if (!issue) return answer("ask", `${guard.name}: no flow found for this action.`);
  const state = read(ctx, `${input.cwd}/.flow/${issue}.json`);
  if (!state) return answer("ask", `${guard.name}: ${issue} has no flow state.`);
  if (stages.indexOf(state.stage) >= stages.indexOf(guard.stage)) return undefined;
  return answer(
    "deny",
    `${guard.name} needs stage "${guard.stage}" approved, and ${issue} is at "${state.stage}". Finish this stage and move .flow/${issue}.json forward first.`,
  );
}

export default function gate(input, ctx) {
  if (!input.tool_input) return undefined;
  if (input.tool_name === "Write" || input.tool_name === "Edit") return stateChange(input, ctx);
  return guarded(input, ctx);
}
