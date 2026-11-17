use domain::{Decision, PermissionMode};
use serde_json::json;

use crate::models::{Held, call, no_findings, says};
use crate::stage::{PATIENCE, Stage};

#[test]
fn a_message_steered_during_a_tool_call_is_not_delivered_with_its_results_and_can_still_be_taken_back() {
    let stage = Stage::new();
    let (model, gate) = Held::before_turn(
        1,
        [call("w1", "Write", json!({"file_path": "out.txt", "content": "written\n"})), says("Written."), no_findings()],
    );
    stage.model(model);
    let mut request = stage.request("Write the file");
    request.permission = PermissionMode::Ask;

    let mut run = stage.start(&request);
    let ask = run.asked();
    run.steer("Also add tests");
    run.answer(&ask, Decision::Allow);
    gate.reached.recv_timeout(PATIENCE).expect("the model is asked again with the tool's results");
    run.unsteer();
    gate.release.send(()).expect("the model waits at the gate");
    stage.end("prompt", run);

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted w1 Write "out.txt" input="{\"content\":\"written\\n\",\"file_path\":\"out.txt\"}"
        Asked ask-<uuid>-w1 Permission { tool: "Write", subject: "out.txt", rememberable: true, rule: None }
        Answered ask-<uuid>-w1 Allow
        Edited(Edited { path: "<project>/out.txt", line: 1, removed: 0, added: 1 })
        ToolFinished w1
        ToolResult w1 ok "Created out.txt · Wrote 1 lines\n    1 + written"
        Unsteered(["Also add tests"])
        Text "Written."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        AgentSpawned #0 kind=review summary="Reviewing the changes" prompt="# Task\nWrite the file\n\n# Changed files\n- out.txt"
        AgentUpdate #0 Running(0) summary=None
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        AgentUpdate #0 Done(<seconds>) summary=Some("0 findings")
        Reviewed([])
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Write the file"
          reminder "# Runtime reminder\nAsk mode: tool actions require the permissions enforced by the runtime. Respect denials and do not bypass them.\nCollect background agent reports be…
        assistant:
          tool_use w1 Write "{\"content\":\"written\\n\",\"file_path\":\"out.txt\"}"
        user:
          tool_result w1 ok "Created out.txt (1 lines)."
        assistant:
          text "Written."
        == prompt: log appended ==
        prompted "Write the file"
        event SessionStarted
        opened
        event Checkpoint
        called model=claude-opus-5-5 stop=ToolUse messages=1
        event Usage
        event ToolStarted w1
        event Asked
        event Answered
        event Edited
        event ToolFinished w1
        event ToolResult w1
        event Unsteered
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=3
        event Usage
        event AgentSpawned
        event AgentUpdate
        called model=claude-opus-5-5 stop=EndTurn messages=1
        event Usage
        event AgentUpdate
        event Reviewed
        message user
        message assistant
        message user
        message assistant
        event Completed
        "##,
    );
}
