use serde_json::json;

use crate::models::{call, calls, says};
use crate::stage::{Order, Stage};

#[test]
fn a_plain_answer_starts_a_session_and_a_second_prompt_resumes_it() {
    let stage = Stage::new();
    stage.script([says("Hello.")]);
    stage.script([says("Still here.")]);

    let session = stage.play("first prompt", &stage.request("Say hello"));
    stage.play("second prompt", &stage.resuming(&session, "Are you there?"));

    stage.assert(
        r##"
        == first prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Text "Hello."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == first prompt: conversation ==
        user:
          text "Say hello"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "Hello."
        == first prompt: log appended ==
        prompted "Say hello"
        event SessionStarted
        opened
        event Checkpoint
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=1
        event Usage
        message assistant
        event Completed
        == second prompt: events ==
        SessionStarted <session-1>
        Checkpoint(1)
        Text "Still here."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == second prompt: conversation ==
        user:
          text "Say hello"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "Hello."
        user:
          text "Are you there?"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "Still here."
        == second prompt: log appended ==
        prompted "Are you there?"
        event SessionStarted
        opened
        event Checkpoint
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
fn a_tool_turn_starts_finishes_and_reports_the_tool_before_the_answer() {
    let stage = Stage::new();
    stage.file("notes.txt", "alpha\nbeta\n");
    stage.script([call("r1", "Read", json!({"file_path": "notes.txt"})), says("It says alpha and beta.")]);

    stage.play("prompt", &stage.request("Read the notes"));

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted r1 Read "notes.txt" input="{\"file_path\":\"notes.txt\"}"
        ToolFinished r1
        ToolResult r1 ok "     1\talpha\n     2\tbeta\n"
        Text "It says alpha and beta."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Read the notes"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          tool_use r1 Read "{\"file_path\":\"notes.txt\"}"
        user:
          tool_result r1 ok "     1\talpha\n     2\tbeta\n"
        assistant:
          text "It says alpha and beta."
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
fn a_tool_turn_keeps_the_conversation_in_one_file_and_the_trace_in_another() {
    let stage = Stage::new();
    stage.file("notes.txt", "alpha\n");
    stage.script([call("r1", "Read", json!({"file_path": "notes.txt"})), says("It says alpha.")]);

    stage.prepare(&stage.request("Read the notes"));
    stage.files("after the run");

    stage.assert(
        r##"
        == after the run: conversation file ==
        message user (numbered)
        message assistant (numbered)
        message user (numbered)
        message assistant (numbered)
        == after the run: trace file ==
        prompted (numbered)
        event SessionStarted (numbered)
        opened (numbered)
        event Checkpoint (numbered)
        called (numbered)
        event Usage (numbered)
        event ToolStarted (numbered)
        event ToolFinished (numbered)
        event ToolResult (numbered)
        event TextDelta (numbered)
        called (numbered)
        event Usage (numbered)
        event Completed (numbered)
        "##,
    );
}

#[test]
fn two_read_only_calls_in_one_reply_run_as_a_batch_and_answer_in_call_order() {
    let stage = Stage::new();
    stage.file("a.txt", "first file\n");
    stage.file("b.txt", "second file\n");
    stage.script([
        calls([("r1", "Read", json!({"file_path": "a.txt"})), ("r2", "Read", json!({"file_path": "b.txt"}))]),
        says("Both read."),
    ]);

    let request = stage.request("Read both files");
    stage.end_ordered("prompt", stage.start(&request), Order::PerCall);

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted r1 Read "a.txt" input="{\"file_path\":\"a.txt\"}"
        ToolFinished r1
        ToolResult r1 ok "     1\tfirst file\n"
        ToolStarted r2 Read "b.txt" input="{\"file_path\":\"b.txt\"}"
        ToolFinished r2
        ToolResult r2 ok "     1\tsecond file\n"
        Text "Both read."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Read both files"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          tool_use r1 Read "{\"file_path\":\"a.txt\"}"
          tool_use r2 Read "{\"file_path\":\"b.txt\"}"
        user:
          tool_result r1 ok "     1\tfirst file\n"
          tool_result r2 ok "     1\tsecond file\n"
        assistant:
          text "Both read."
        == prompt: log appended ==
        prompted "Read both files"
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
        event ToolStarted r2
        event ToolFinished r2
        event ToolResult r2
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=3
        event Usage
        message assistant
        event Completed
        "##,
    );
}
