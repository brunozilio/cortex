use domain::{Decision, PermissionMode};
use serde_json::json;

use crate::models::{call, no_findings, says};
use crate::stage::Stage;

fn writing(stage: &Stage) -> domain::PromptRequest {
    stage.script([
        call("w1", "Write", json!({"file_path": "out.txt", "content": "written\n"})),
        says("Done."),
        no_findings(),
    ]);
    let mut request = stage.request("Write the file");
    request.policy.permission = PermissionMode::Ask;
    request
}

#[test]
fn an_edit_in_ask_mode_waits_for_the_user_and_runs_once_allowed() {
    let stage = Stage::new();
    let request = writing(&stage);

    let mut run = stage.start(&request);
    let ask = run.asked();
    run.answer(&ask, Decision::Allow);
    stage.end("prompt", run);

    stage.disk("out.txt", &stage.project.join("out.txt"));
    stage.disk("settings.local.json", &stage.project.join(brand::DIR).join("settings.local.json"));
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
        Text "Done."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        AgentSpawned #0 kind=review summary="Reviewing the changes" prompt="# Task\nWrite the file\n\n# Changed files\n- out.txt"
        AgentUpdate #0 Running(0) summary=None
        Agent #0 | Usage model=claude-opus-5-5 input=10 output=5 cost=none
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
          text "Done."
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
        event Answered
        event Edited
        event ToolFinished w1
        event ToolResult w1
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=3
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
        written
        == settings.local.json ==
        (absent)
        "##,
    );
}

#[test]
fn an_edit_the_user_denies_never_runs_and_the_model_reads_the_refusal() {
    let stage = Stage::new();
    let request = writing(&stage);

    let mut run = stage.start(&request);
    let ask = run.asked();
    run.answer(&ask, Decision::Deny);
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
        Answered ask-<uuid>-w1 Deny
        Refused { tool: "Write", reason: "The user did not allow Write here. Do not retry it; ask them or take another approach." }
        ToolFinished w1
        ToolResult w1 failed "The user did not allow Write here. Do not retry it; ask them or take another approach."
        Text "Done."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Write the file"
          reminder "# Runtime reminder\nAsk mode: tool actions require the permissions enforced by the runtime. Respect denials and do not bypass them.\nCollect background agent reports be…
        assistant:
          tool_use w1 Write "{\"content\":\"written\\n\",\"file_path\":\"out.txt\"}"
        user:
          tool_result w1 failed "The user did not allow Write here. Do not retry it; ask them or take another approach."
        assistant:
          text "Done."
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
        event Answered
        event Refused
        event ToolFinished w1
        event ToolResult w1
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=3
        event Usage
        message assistant
        event Completed
        == out.txt ==
        (absent)
        "##,
    );
}

#[test]
fn an_edit_denied_with_a_note_carries_the_note_to_the_model() {
    let stage = Stage::new();
    let request = writing(&stage);

    let mut run = stage.start(&request);
    let ask = run.asked();
    run.answer(&ask, Decision::DenyWith("use notes.txt instead".into()));
    stage.end("prompt", run);

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted w1 Write "out.txt" input="{\"content\":\"written\\n\",\"file_path\":\"out.txt\"}"
        Asked ask-<uuid>-w1 Permission { tool: "Write", subject: "out.txt", rememberable: true, rule: None }
        Answered ask-<uuid>-w1 DenyWith("use notes.txt instead")
        Refused { tool: "Write", reason: "The user did not allow Write here and said what to do instead: use notes.txt instead" }
        ToolFinished w1
        ToolResult w1 failed "The user did not allow Write here and said what to do instead: use notes.txt instead"
        Text "Done."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Write the file"
          reminder "# Runtime reminder\nAsk mode: tool actions require the permissions enforced by the runtime. Respect denials and do not bypass them.\nCollect background agent reports be…
        assistant:
          tool_use w1 Write "{\"content\":\"written\\n\",\"file_path\":\"out.txt\"}"
        user:
          tool_result w1 failed "The user did not allow Write here and said what to do instead: use notes.txt instead"
        assistant:
          text "Done."
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
        event Answered
        event Refused
        event ToolFinished w1
        event ToolResult w1
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=3
        event Usage
        message assistant
        event Completed
        "##,
    );
}
