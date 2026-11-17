use domain::EngineEvent;
use serde_json::json;

use crate::models::{Gated, call, says};
use crate::stage::{PATIENCE, Stage};

#[test]
fn cancelling_while_the_model_streams_ends_in_cancelled_and_keeps_what_was_said_marked_as_interrupted() {
    let stage = Stage::new();
    let (model, gate) = Gated::streaming("Partial ans");
    stage.model(model);

    let mut run = stage.start(&stage.request("Tell me a story"));
    gate.reached.recv_timeout(PATIENCE).expect("the model is streaming");
    run.cancel();
    gate.release.send(()).expect("the model waits at the gate");
    stage.end("prompt", run);

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Text "Partial ans"
        Cancelled
        == prompt: conversation ==
        user:
          text "Tell me a story"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "Partial ans"
          text "\n\n[interrupted by the user]"
        == prompt: log appended ==
        prompted "Tell me a story"
        event SessionStarted
        opened
        event Checkpoint
        message user
        event TextDelta
        message assistant
        event Cancelled
        "##,
    );
}

#[test]
fn cancelling_while_an_edit_waits_for_permission_ends_in_cancelled_and_saves_the_unrun_call() {
    let stage = Stage::new();
    stage.script([call("w1", "Write", json!({"file_path": "out.txt", "content": "written\n"})), says("never asked")]);
    let mut request = stage.request("Write the file");
    request.policy.permission = domain::PermissionMode::Ask;

    let mut run = stage.start(&request);
    run.asked();
    run.cancel();
    stage.end("prompt", run);

    stage.disk("out.txt", &stage.project.join("out.txt"));
    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted w1 Write "out.txt" input="{\"content\":\"written\\n\",\"file_path\":\"out.txt\"}"
        Asked ask-<uuid>-w1 Permission { tool: "Write", subject: "out.txt", rememberable: true, rule: None }
        ToolFinished w1
        ToolResult w1 failed "Not run: the run was stopped while waiting for the user's answer, so Write did not run and nothing was changed."
        Cancelled
        == prompt: conversation ==
        user:
          text "Write the file"
          reminder "# Runtime reminder\nAsk mode: tool actions require the permissions enforced by the runtime. Respect denials and do not bypass them.\nCollect background agent reports be…
        assistant:
          tool_use w1 Write "{\"content\":\"written\\n\",\"file_path\":\"out.txt\"}"
        user:
          tool_result w1 failed "Not run: the run was stopped while waiting for the user's answer, so Write did not run and nothing was changed."
        == prompt: log appended ==
        prompted "Write the file"
        event SessionStarted
        opened
        event Checkpoint
        message user
        called model=claude-opus-5-5 stop=ToolUse messages=1
        event Usage
        message assistant
        event ToolStarted w1
        event Asked
        event ToolFinished w1
        event ToolResult w1
        message user
        event Cancelled
        == out.txt ==
        (absent)
        "##,
    );
}

#[cfg(unix)]
#[test]
fn cancelling_while_a_shell_command_runs_ends_in_cancelled_and_saves_the_stopped_call() {
    let stage = Stage::new();
    stage.script([call("b1", "Bash", json!({"command": "echo started; sleep 600"})), says("never asked")]);
    let mut request = stage.request("Wait a while");
    request.policy.allowed_tools = vec!["Bash".into()];

    let mut run = stage.start(&request);
    run.until(|event| matches!(event, EngineEvent::ToolProgress { line, .. } if line.contains("started")));
    run.cancel();
    stage.end("prompt", run);

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted b1 Bash "echo started; sleep 600" input="{\"command\":\"echo started; sleep 600\"}"
        ToolProgress b1 "started"
        ToolFinished b1
        ToolResult b1 failed "started\n\nStopped while running: the command may have run in part or in full, and its changes may have been written. Check what it did before running it agai…
        Cancelled
        == prompt: conversation ==
        user:
          text "Wait a while"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          tool_use b1 Bash "{\"command\":\"echo started; sleep 600\"}"
        user:
          tool_result b1 failed "started\n\nStopped while running: the command may have run in part or in full, and its changes may have been written. Check what it did before running it a…
        == prompt: log appended ==
        prompted "Wait a while"
        event SessionStarted
        opened
        event Checkpoint
        message user
        called model=claude-opus-5-5 stop=ToolUse messages=1
        event Usage
        message assistant
        event ToolStarted b1
        event ToolProgress b1
        event ToolFinished b1
        event ToolResult b1
        message user
        event Cancelled
        "##,
    );
}
