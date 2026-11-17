use domain::{Decision, PermissionMode};
use serde_json::json;

use crate::models::{call, says};
use crate::stage::Stage;

fn stage_with_a_project_hook() -> Stage {
    let stage = Stage::new();
    stage.file("notes.txt", "alpha\n");
    let folder = stage.project.join(brand::DIR);
    std::fs::create_dir_all(&folder).expect("project folder");
    let hooks = json!({"hooks": {"PreToolUse": [
        {"matcher": "Read", "hooks": [{"type": "command", "command": "echo 'the project says no' >&2; exit 2"}]}
    ]}});
    std::fs::write(folder.join("settings.json"), hooks.to_string()).expect("project settings");
    stage.script([call("r1", "Read", json!({"file_path": "notes.txt"})), says("Done.")]);
    stage
}

#[test]
fn a_rule_kept_in_an_untrusted_folder_says_that_it_waits_for_trust() {
    let stage = Stage::new();
    stage.script([call("b1", "Bash", json!({"command": "echo one"})), says("Done.")]);
    let mut request = stage.request("Echo");
    request.policy.permission = PermissionMode::Ask;

    let mut run = stage.start(&request);
    let ask = run.asked();
    run.answer(&ask, Decision::AllowAlways);
    stage.end("prompt", run);

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted b1 Bash "echo one" input="{\"command\":\"echo one\"}"
        Asked ask-<uuid>-b1 Permission { tool: "Bash", subject: "echo one", rememberable: true, rule: Some("Bash(echo one:*)") }
        Answered ask-<uuid>-b1 AllowAlways
        Notice("Kept Bash(echo one:*) in .<brand>/settings.local.json. This folder is not trusted, so the rule holds for this run only; it applies to later ones once you trust the folder (…
        ToolProgress b1 "one"
        ToolFinished b1
        ToolResult b1 ok "one"
        Text "Done."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Echo"
          reminder "# Runtime reminder\nAsk mode: tool actions require the permissions enforced by the runtime. Respect denials and do not bypass them.\nCollect background agent reports be…
        assistant:
          tool_use b1 Bash "{\"command\":\"echo one\"}"
        user:
          tool_result b1 ok "one"
        assistant:
          text "Done."
        == prompt: log appended ==
        prompted "Echo"
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
        event Notice
        event ToolProgress b1
        event ToolFinished b1
        event ToolResult b1
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=3
        event Usage
        message assistant
        event Completed
        "##,
    );
}

#[test]
fn an_untrusted_project_s_hook_does_not_run_and_the_call_goes_through() {
    let stage = stage_with_a_project_hook();

    stage.play("prompt", &stage.request("Read the notes"));

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted r1 Read "notes.txt" input="{\"file_path\":\"notes.txt\"}"
        ToolFinished r1
        ToolResult r1 ok "     1\talpha\n"
        Text "Done."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Read the notes"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          tool_use r1 Read "{\"file_path\":\"notes.txt\"}"
        user:
          tool_result r1 ok "     1\talpha\n"
        assistant:
          text "Done."
        == prompt: log appended ==
        prompted "Read the notes"
        event SessionStarted
        opened
        event Checkpoint
        message user
        called model=claude-opus-5-5 stop=ToolUse messages=1
        event Usage
        message assistant
        event ToolStarted r1
        event ToolFinished r1
        event ToolResult r1
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=3
        event Usage
        message assistant
        event Completed
        "##,
    );
}

#[test]
fn a_trusted_project_s_hook_blocks_the_call_and_the_model_reads_why() {
    let stage = stage_with_a_project_hook();

    stage.play("prompt", &stage.trusted("Read the notes"));

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted r1 Read "notes.txt" input="{\"file_path\":\"notes.txt\"}"
        ToolFinished r1
        ToolResult r1 failed "Hook blocked Read: the project says no"
        Text "Done."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Read the notes"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          tool_use r1 Read "{\"file_path\":\"notes.txt\"}"
        user:
          tool_result r1 failed "Hook blocked Read: the project says no"
        assistant:
          text "Done."
        == prompt: log appended ==
        prompted "Read the notes"
        event SessionStarted
        opened
        event Checkpoint
        message user
        called model=claude-opus-5-5 stop=ToolUse messages=1
        event Usage
        message assistant
        event ToolStarted r1
        event ToolFinished r1
        event ToolResult r1
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=3
        event Usage
        message assistant
        event Completed
        "##,
    );
}
