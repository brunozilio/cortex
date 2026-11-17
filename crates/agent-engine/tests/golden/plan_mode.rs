use domain::{Decision, PermissionMode};
use serde_json::json;

use crate::models::{call, no_findings, says};
use crate::stage::Stage;

#[test]
fn plan_mode_refuses_an_edit_then_an_approved_plan_changes_mode_and_the_edit_runs() {
    let stage = Stage::new();
    let write = |id| call(id, "Write", json!({"file_path": "out.txt", "content": "planned\n"}));
    stage.script([
        write("w1"),
        call("p1", "ExitPlanMode", json!({"plan": "1. Write out.txt"})),
        write("w2"),
        says("Implemented."),
        no_findings(),
    ]);
    let mut request = stage.request("Plan, then write the file");
    request.policy.permission = PermissionMode::Plan;

    let mut run = stage.start(&request);
    let ask = run.asked();
    run.answer(&ask, Decision::Approve(PermissionMode::AcceptEdits));
    stage.end("prompt", run);

    stage.disk("out.txt", &stage.project.join("out.txt"));
    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted w1 Write "out.txt" input="{\"content\":\"planned\\n\",\"file_path\":\"out.txt\"}"
        Refused { tool: "Write", reason: "Write is not available while planning: only read and search now, then present the plan." }
        ToolFinished w1
        ToolResult w1 failed "Write is not available while planning: only read and search now, then present the plan."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted p1 ExitPlanMode "" input="{\"plan\":\"1. Write out.txt\"}"
        Asked ask-<uuid>-p1 Plan { plan: "1. Write out.txt" }
        Answered ask-<uuid>-p1 Approve(AcceptEdits)
        ModeChanged(AcceptEdits)
        ToolFinished p1
        ToolResult p1 ok "The user approved your plan and planning is over (edits run freely, commands ask first). Start implementing it now, following the plan. Track the steps with the t…
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted w2 Write "out.txt" input="{\"content\":\"planned\\n\",\"file_path\":\"out.txt\"}"
        Edited(Edited { path: "<project>/out.txt", line: 1, removed: 0, added: 1 })
        ToolFinished w2
        ToolResult w2 ok "Created out.txt · Wrote 1 lines\n    1 + planned"
        Text "Implemented."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        AgentSpawned #0 kind=review summary="Reviewing the changes" prompt="# Task\nPlan, then write the file\n\n# Changed files\n- out.txt"
        AgentUpdate #0 Running(0) summary=None
        Agent #0 | Usage model=claude-opus-5-5 input=10 output=5 cost=none
        AgentUpdate #0 Done(<seconds>) summary=Some("0 findings")
        Reviewed([])
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Plan, then write the file"
          reminder "# Runtime reminder\nPlanning mode is on. The user wants a plan before anything changes: do not edit files, run commands that change anything, commit or change any confi…
        assistant:
          tool_use w1 Write "{\"content\":\"planned\\n\",\"file_path\":\"out.txt\"}"
        user:
          tool_result w1 failed "Write is not available while planning: only read and search now, then present the plan."
        assistant:
          tool_use p1 ExitPlanMode "{\"plan\":\"1. Write out.txt\"}"
        user:
          tool_result p1 ok "The user approved your plan and planning is over (edits run freely, commands ask first). Start implementing it now, following the plan. Track the steps with th…
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          tool_use w2 Write "{\"content\":\"planned\\n\",\"file_path\":\"out.txt\"}"
        user:
          tool_result w2 ok "Created out.txt (1 lines)."
        assistant:
          text "Implemented."
        == prompt: log appended ==
        prompted "Plan, then write the file"
        event SessionStarted
        opened
        event Checkpoint
        message user
        called model=claude-opus-5-5 stop=ToolUse messages=1
        event Usage
        message assistant
        event ToolStarted w1
        event Refused
        event ToolFinished w1
        event ToolResult w1
        message user
        called model=claude-opus-5-5 stop=ToolUse messages=3
        event Usage
        message assistant
        event ToolStarted p1
        event Asked
        event Answered
        event ModeChanged
        event ToolFinished p1
        event ToolResult p1
        message user
        amended keep=4
        message user
        called model=claude-opus-5-5 stop=ToolUse messages=5
        event Usage
        message assistant
        event ToolStarted w2
        event Edited
        event ToolFinished w2
        event ToolResult w2
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=7
        event Usage
        message assistant
        event AgentSpawned
        event AgentUpdate
        called model=claude-opus-5-5 stop=EndTurn messages=1 agent=#0
        event Agent #0 | Usage
        event AgentUpdate
        event Reviewed
        event Completed
        == out.txt ==
        planned
        "##,
    );
}

#[test]
fn a_plan_sent_back_with_a_note_keeps_plan_mode_and_tells_the_model_why() {
    let stage = Stage::new();
    stage.script([call("p1", "ExitPlanMode", json!({"plan": "1. Delete everything"})), says("I will rethink.")]);
    let mut request = stage.request("Plan the cleanup");
    request.policy.permission = PermissionMode::Plan;

    let mut run = stage.start(&request);
    let ask = run.asked();
    run.answer(&ask, Decision::DenyWith("too drastic".into()));
    stage.end("prompt", run);

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted p1 ExitPlanMode "" input="{\"plan\":\"1. Delete everything\"}"
        Asked ask-<uuid>-p1 Plan { plan: "1. Delete everything" }
        Answered ask-<uuid>-p1 DenyWith("too drastic")
        ToolFinished p1
        ToolResult p1 failed "The user wants to keep planning and did not approve the plan. Do not implement anything. They said: too drastic"
        Text "I will rethink."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Plan the cleanup"
          reminder "# Runtime reminder\nPlanning mode is on. The user wants a plan before anything changes: do not edit files, run commands that change anything, commit or change any confi…
        assistant:
          tool_use p1 ExitPlanMode "{\"plan\":\"1. Delete everything\"}"
        user:
          tool_result p1 failed "The user wants to keep planning and did not approve the plan. Do not implement anything. They said: too drastic"
        assistant:
          text "I will rethink."
        == prompt: log appended ==
        prompted "Plan the cleanup"
        event SessionStarted
        opened
        event Checkpoint
        message user
        called model=claude-opus-5-5 stop=ToolUse messages=1
        event Usage
        message assistant
        event ToolStarted p1
        event Asked
        event Answered
        event ToolFinished p1
        event ToolResult p1
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=3
        event Usage
        message assistant
        event Completed
        "##,
    );
}
