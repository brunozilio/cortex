import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import gate, { gates, guards, stages } from "../hooks/gate.js";
import resume from "../hooks/resume.js";

const cwd = "/work";

function project(files = {}, branch = "") {
  return {
    readFile(path) {
      if (!(path in files)) throw new Error(`${path}: not found`);
      return files[path];
    },
    exec(command) {
      if (command === "git branch --show-current") return { code: branch ? 0 : 1, stdout: `${branch}\n`, stderr: "" };
      if (command === "ls .flow") {
        const names = Object.keys(files)
          .filter((path) => path.startsWith(`${cwd}/.flow/`))
          .map((path) => path.slice(`${cwd}/.flow/`.length));
        return { code: 0, stdout: names.join("\n"), stderr: "" };
      }
      return { code: 1, stdout: "", stderr: "unknown" };
    },
  };
}

const state = (issue, stage, more = {}) => JSON.stringify({ issue, stage, ...more });
const at = (issue) => `${cwd}/.flow/${issue}.json`;
const write = (issue, content) => ({ tool_name: "Write", cwd, tool_input: { file_path: at(issue), content } });
const bash = (command) => ({ tool_name: "Bash", cwd, tool_input: { command } });
const decision = (result) => result && result.hookSpecificOutput.permissionDecision;
const reason = (result) => result.hookSpecificOutput.permissionDecisionReason;

test("every stage has a gate and asks by default", () => {
  assert.deepEqual(Object.keys(gates), stages);
  assert.ok(stages.every((stage) => gates[stage] === "ask"));
});

test("starting a flow and moving one stage forward ask for approval", () => {
  const started = gate(write("PAY-12", state("PAY-12", "refine", { approval: "Start refining" })), project());
  assert.equal(decision(started), "ask");
  assert.equal(reason(started), "Approve PAY-12: start -> refine? Start refining");
  const files = { [at("PAY-12")]: state("PAY-12", "refine") };
  const moved = gate(write("PAY-12", state("PAY-12", "spec")), project(files));
  assert.equal(decision(moved), "ask");
  assert.match(reason(moved), /PAY-12: refine -> spec/);
});

test("an update inside the same stage is left to the usual permissions", () => {
  const files = { [at("PAY-12")]: state("PAY-12", "spec") };
  assert.equal(gate(write("PAY-12", state("PAY-12", "spec", { title: "Refunds" })), project(files)), undefined);
});

test("a stage cannot be skipped, and going back asks", () => {
  const files = { [at("PAY-12")]: state("PAY-12", "spec") };
  const skipped = gate(write("PAY-12", state("PAY-12", "implement")), project(files));
  assert.equal(decision(skipped), "deny");
  assert.match(reason(skipped), /after "spec" comes "plan"/);
  const back = gate(write("PAY-12", state("PAY-12", "refine")), project(files));
  assert.equal(decision(back), "ask");
});

test("a state file must be whole, valid and about its own issue", () => {
  const edit = { tool_name: "Edit", cwd, tool_input: { file_path: at("PAY-12"), old_string: "a", new_string: "b" } };
  assert.equal(decision(gate(edit, project())), "deny");
  assert.equal(decision(gate(write("PAY-12", "{"), project())), "deny");
  assert.equal(decision(gate(write("PAY-12", state("PAY-13", "refine")), project())), "deny");
  assert.equal(decision(gate(write("PAY-12", state("PAY-12", "ship")), project())), "deny");
});

test("other files are none of the gate's business", () => {
  const other = { tool_name: "Write", cwd, tool_input: { file_path: `${cwd}/src/App.java`, content: "class App {}" } };
  assert.equal(gate(other, project()), undefined);
  assert.equal(gate(bash("mvn verify"), project()), undefined);
  assert.equal(gate({ tool_name: "Bash", cwd }, project()), undefined);
});

test("a gate may be a function of the change, or allow outright", () => {
  const kept = gates.spec;
  try {
    gates.spec = "allow";
    const files = { [at("PAY-12")]: state("PAY-12", "refine") };
    assert.equal(decision(gate(write("PAY-12", state("PAY-12", "spec")), project(files))), "allow");
    gates.spec = (change) => (change.state.size === "small" ? "allow" : "deny");
    assert.equal(decision(gate(write("PAY-12", state("PAY-12", "spec", { size: "small" })), project(files))), "allow");
    assert.equal(decision(gate(write("PAY-12", state("PAY-12", "spec", { size: "large" })), project(files))), "deny");
    gates.spec = () => "whatever";
    assert.equal(decision(gate(write("PAY-12", state("PAY-12", "spec")), project(files))), "ask");
  } finally {
    gates.spec = kept;
  }
});

test("merging is refused until the merge stage was approved", () => {
  const branch = "feature/PAY-12-refunds";
  const early = project({ [at("PAY-12")]: state("PAY-12", "review") }, branch);
  const refused = gate(bash("gh pr merge 41 --squash"), early);
  assert.equal(decision(refused), "deny");
  assert.match(reason(refused), /needs stage "merge" approved, and PAY-12 is at "review"/);
  const approved = project({ [at("PAY-12")]: state("PAY-12", "merge") }, branch);
  assert.equal(gate(bash("gh pr merge 41 --squash"), approved), undefined);
  assert.equal(decision(gate(bash("gh pr create --fill"), project({ [at("PAY-12")]: state("PAY-12", "implement") }, branch))), "deny");
  assert.equal(gate(bash("gh pr create --fill"), project({ [at("PAY-12")]: state("PAY-12", "pr") }, branch)), undefined);
});

test("a guarded action with no flow behind it asks", () => {
  assert.equal(decision(gate(bash("gh pr merge 41"), project())), "ask");
  assert.equal(decision(gate(bash("gh pr merge 41"), project({}, "feature/PAY-12-refunds"))), "ask");
});

test("without a branch the only open flow is the one meant", () => {
  const files = { [at("PAY-12")]: state("PAY-12", "qa"), [at("PAY-9")]: state("PAY-9", "done") };
  assert.equal(gate(bash("gh pr merge 41"), project(files)), undefined);
  const two = { ...files, [at("PAY-20")]: state("PAY-20", "spec") };
  assert.equal(decision(gate(bash("gh pr merge 41"), project(two))), "ask");
});

test("Jira and Zephyr follow the flow, and infrastructure always asks", () => {
  const jira = (tool, input) => ({ tool_name: `mcp__plugin_dev-flow_atlassian__${tool}`, cwd, tool_input: input });
  assert.equal(decision(gate(jira("createJiraIssue", { summary: "Refunds" }), project())), "ask");
  const refining = project({ [at("PAY-12")]: state("PAY-12", "refine") });
  const moving = jira("transitionJiraIssue", { issueIdOrKey: "PAY-12", transition: { id: "31" } });
  assert.equal(decision(gate(moving, refining)), "deny");
  assert.equal(gate(moving, project({ [at("PAY-12")]: state("PAY-12", "implement") })), undefined);
  assert.equal(gate(jira("getJiraIssue", { issueIdOrKey: "PAY-12" }), refining), undefined);
  const post = 'curl -s -X POST https://api.zephyrscale.smartbear.com/v2/testcases -H "Authorization: Bearer $ZEPHYR_TOKEN" -d @case.json';
  assert.equal(decision(gate(bash(post), refining)), "deny");
  assert.equal(gate(bash(post), project({ [at("PAY-12")]: state("PAY-12", "plan") })), undefined);
  const get = 'curl -s "https://api.zephyrscale.smartbear.com/v2/testcases?projectKey=PAY"';
  assert.equal(gate(bash(get), refining), undefined);
  assert.equal(decision(gate(bash("terraform apply -auto-approve"), project())), "ask");
  assert.equal(gate(bash("terraform plan -out tf.plan"), project()), undefined);
});

test("every guard names a known stage or a decision", () => {
  for (const guard of guards) {
    assert.ok(guard.decision ? ["ask", "deny"].includes(guard.decision) : stages.includes(guard.stage), guard.name);
  }
});

test("a new session is told which flows are open", () => {
  const files = { [at("PAY-12")]: state("PAY-12", "qa", { title: "Refunds" }), [at("PAY-9")]: state("PAY-9", "done") };
  const told = resume({ cwd }, project(files)).hookSpecificOutput.additionalContext;
  assert.match(told, /PAY-12 is at stage "qa" \(Refunds\)/);
  assert.doesNotMatch(told, /PAY-9/);
  assert.equal(resume({ cwd }, project()), undefined);
});

test("the hooks file registers both scripts on events the app knows", () => {
  const { hooks } = JSON.parse(readFileSync(new URL("../hooks/hooks.json", import.meta.url), "utf8"));
  assert.deepEqual(Object.keys(hooks), ["PreToolUse", "SessionStart"]);
  for (const groups of Object.values(hooks)) {
    for (const handler of groups.flatMap((group) => group.hooks)) {
      assert.equal(handler.type, "script");
      const file = handler.command.replace(/^\$\{[A-Z_]+_PLUGIN_ROOT\}\//, "../");
      assert.ok(readFileSync(new URL(file, import.meta.url), "utf8").includes("export default"), handler.command);
    }
  }
  assert.ok(new RegExp(hooks.PreToolUse[0].matcher).test("mcp__plugin_dev-flow_atlassian__transitionJiraIssue"));
});
