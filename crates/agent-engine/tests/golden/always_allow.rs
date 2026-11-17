use domain::{Decision, PermissionMode};
use serde_json::json;

use crate::models::{call, no_findings, says};
use crate::stage::Stage;

#[test]
fn an_edit_always_allowed_is_remembered_for_the_run_only_and_writes_no_rule() {
    let stage = Stage::new();
    stage.script([
        call("w1", "Write", json!({"file_path": "out.txt", "content": "written\n"})),
        call("w2", "Write", json!({"file_path": "out.txt", "content": "rewritten\n"})),
        says("Done."),
        no_findings(),
    ]);
    let mut request = stage.request("Write the file twice");
    request.policy.permission = PermissionMode::Ask;

    let mut run = stage.start(&request);
    let ask = run.asked();
    run.answer(&ask, Decision::AllowAlways);
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
        Answered ask-<uuid>-w1 AllowAlways
        Edited(Edited { path: "<project>/out.txt", line: 1, removed: 0, added: 1 })
        ToolFinished w1
        ToolResult w1 ok "Created out.txt · Wrote 1 lines\n    1 + written"
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted w2 Write "out.txt" input="{\"content\":\"rewritten\\n\",\"file_path\":\"out.txt\"}"
        Edited(Edited { path: "<project>/out.txt", line: 1, removed: 1, added: 1 })
        ToolFinished w2
        ToolResult w2 ok "Replaced out.txt · Wrote 1 lines\n    1 + rewritten"
        Text "Done."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        AgentSpawned #0 kind=review summary="Reviewing the changes" prompt="# Task\nWrite the file twice\n\n# Changed files\n- out.txt"
        AgentUpdate #0 Running(0) summary=None
        Agent #0 | Usage model=claude-opus-5-5 input=10 output=5 cost=none
        AgentUpdate #0 Done(<seconds>) summary=Some("0 findings")
        Reviewed([])
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Write the file twice"
          reminder "# Runtime reminder\nAsk mode: tool actions require the permissions enforced by the runtime. Respect denials and do not bypass them.\nCollect background agent reports be…
        assistant:
          tool_use w1 Write "{\"content\":\"written\\n\",\"file_path\":\"out.txt\"}"
        user:
          tool_result w1 ok "Created out.txt (1 lines)."
        assistant:
          tool_use w2 Write "{\"content\":\"rewritten\\n\",\"file_path\":\"out.txt\"}"
        user:
          tool_result w2 ok "Replaced out.txt (1 lines)."
        assistant:
          text "Done."
        == prompt: log appended ==
        prompted "Write the file twice"
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
        called model=claude-opus-5-5 stop=ToolUse messages=3
        event Usage
        message assistant
        event ToolStarted w2
        event Edited
        event ToolFinished w2
        event ToolResult w2
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=5
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
        rewritten
        == settings.local.json ==
        (absent)
        "##,
    );
}

#[cfg(unix)]
#[test]
fn a_command_always_allowed_runs_and_keeps_its_rule_in_the_project_s_local_settings() {
    let stage = Stage::new();
    stage.script([
        call("b1", "Bash", json!({"command": "echo one"})),
        call("b2", "Bash", json!({"command": "echo one"})),
        says("Done."),
    ]);
    let mut request = stage.trusted("Echo twice");
    request.policy.permission = PermissionMode::Ask;

    let mut run = stage.start(&request);
    let ask = run.asked();
    run.answer(&ask, Decision::AllowAlways);
    stage.end("prompt", run);

    stage.disk("settings.local.json", &stage.project.join(brand::DIR).join("settings.local.json"));
    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted b1 Bash "echo one" input="{\"command\":\"echo one\"}"
        Asked ask-<uuid>-b1 Permission { tool: "Bash", subject: "echo one", rememberable: true, rule: Some("Bash(echo one:*)") }
        Answered ask-<uuid>-b1 AllowAlways
        ToolProgress b1 "one"
        ToolFinished b1
        ToolResult b1 ok "one"
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted b2 Bash "echo one" input="{\"command\":\"echo one\"}"
        ToolProgress b2 "one"
        ToolFinished b2
        ToolResult b2 ok "one"
        Text "Done."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Echo twice"
          reminder "# Runtime reminder\nAsk mode: tool actions require the permissions enforced by the runtime. Respect denials and do not bypass them.\nCollect background agent reports be…
        assistant:
          tool_use b1 Bash "{\"command\":\"echo one\"}"
        user:
          tool_result b1 ok "one"
        assistant:
          tool_use b2 Bash "{\"command\":\"echo one\"}"
        user:
          tool_result b2 ok "one"
        assistant:
          text "Done."
        == prompt: log appended ==
        prompted "Echo twice"
        event SessionStarted
        opened
        event Checkpoint
        message user
        called model=claude-opus-5-5 stop=ToolUse messages=1
        event Usage
        message assistant
        event ToolStarted b1
        event Asked
        event Answered
        event ToolProgress b1
        event ToolFinished b1
        event ToolResult b1
        message user
        called model=claude-opus-5-5 stop=ToolUse messages=3
        event Usage
        message assistant
        event ToolStarted b2
        event ToolProgress b2
        event ToolFinished b2
        event ToolResult b2
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=5
        event Usage
        message assistant
        event Completed
        == settings.local.json ==
        {
          "permissions": {
            "allow": [
              "Bash(echo one:*)"
            ]
          }
        }
        "##,
    );
}
