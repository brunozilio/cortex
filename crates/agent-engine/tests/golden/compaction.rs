use agent_engine::transcript::Block;
use domain::{ModelOption, SessionCommand};
use serde_json::json;

use crate::models::{measuring, says};
use crate::stage::Stage;

fn small_window(stage: &Stage, prompt: &str) -> domain::PromptRequest {
    let mut request = stage.request(prompt);
    request.session.model = ModelOption::new("claude-opus-5-5", "Opus 5.5").with_window(100_000);
    request
}

fn a_prompt_many_times_its_tool_exchange() -> String {
    format!("Read the notes in notes.txt and tell me what they say.{}", " Leave nothing out.".repeat(60))
}

fn reads_with_a_nearly_full_window() -> crate::models::Scripted {
    let read = Block::ToolUse { id: "r1".into(), name: "Read".into(), input: json!({"file_path": "notes.txt"}) };
    measuring(vec![read], 95_000)
}

#[test]
fn a_nearly_full_context_window_is_compacted_mid_run_in_a_new_session_keeping_the_tool_exchange_and_restarting_the_log()
{
    let stage = Stage::new();
    stage.file("notes.txt", "alpha\nbeta\n");
    let model = stage.script([
        reads_with_a_nearly_full_window(),
        says("<summary>The user asked to read the notes; they say alpha and beta.</summary>"),
        says("Done."),
    ]);

    stage.play("prompt", &small_window(&stage, &a_prompt_many_times_its_tool_exchange()));

    assert_eq!(model.asked().len(), 3);
    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=95000 output=50 cost=none
        ToolStarted r1 Read "notes.txt" input="{\"file_path\":\"notes.txt\"}"
        ToolFinished r1
        ToolResult r1 ok "     1\talpha\n     2\tbeta\n"
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Compacted before=<tokens> after=<tokens> shrank=true summary="The user asked to read the notes; they say alpha and beta."
        Text "Done."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "This session continues an earlier conversation, compacted into this summary:\n\nThe user asked to read the notes; they say alpha and beta.\n\nThe messages after this summar…
          text "If you need specific details from before compaction (exact code, error messages, or content you wrote), read the full transcript at: <tmp>/<brand>-tool-results/<session-1>/…
        assistant:
          tool_use r1 Read "{\"file_path\":\"notes.txt\"}"
        user:
          tool_result r1 ok "     1\talpha\n     2\tbeta\n"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "Done."
        == prompt: log appended ==
        prompted "Read the notes in notes.txt and tell me what they say. Leave nothing out. Leave nothing out. Leave nothing out. Leave nothing out. Leave nothing out. Leave nothing out. L…
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
        called model=claude-opus-5-5 stop=EndTurn messages=2
        event Usage
        event Compacted
        restarted
        message user
        message assistant
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
fn a_nearly_full_context_window_is_compacted_mid_run_and_a_resumed_session_s_log_restarts() {
    let stage = Stage::new();
    stage.file("notes.txt", "alpha\nbeta\n");
    stage.script([says("Hello there.")]);
    stage.script([
        reads_with_a_nearly_full_window(),
        says("<summary>They greeted each other, then the notes were read: alpha and beta.</summary>"),
        says("Done."),
    ]);

    let session = stage.play("first prompt", &small_window(&stage, "hi"));
    let mut second = small_window(&stage, "Read the notes");
    second.session.resume = Some(session);
    stage.play("second prompt", &second);

    stage.assert(
        r##"
        == first prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Text "Hello there."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == first prompt: conversation ==
        user:
          text "hi"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "Hello there."
        == first prompt: log appended ==
        prompted "hi"
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
        Usage model=claude-opus-5-5 input=95000 output=50 cost=none
        ToolStarted r1 Read "notes.txt" input="{\"file_path\":\"notes.txt\"}"
        ToolFinished r1
        ToolResult r1 ok "     1\talpha\n     2\tbeta\n"
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Compacted before=<tokens> after=<tokens> shrank=true summary="They greeted each other, then the notes were read: alpha and beta."
        Text "Done."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == second prompt: conversation ==
        user:
          text "This session continues an earlier conversation, compacted into this summary:\n\nThey greeted each other, then the notes were read: alpha and beta.\n\nThe messages after thi…
          text "If you need specific details from before compaction (exact code, error messages, or content you wrote), read the full transcript at: <tmp>/<brand>-tool-results/<session-1>/…
        assistant:
          tool_use r1 Read "{\"file_path\":\"notes.txt\"}"
        user:
          tool_result r1 ok "     1\talpha\n     2\tbeta\n"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "Done."
        == second prompt: log appended ==
        prompted "Read the notes"
        event SessionStarted
        opened
        event Checkpoint
        message user
        called model=claude-opus-5-5 stop=ToolUse messages=3
        event Usage
        message assistant
        event ToolStarted r1
        event ToolFinished r1
        event ToolResult r1
        message user
        called model=claude-opus-5-5 stop=EndTurn messages=4
        event Usage
        event Compacted
        restarted
        message user
        message assistant
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
fn the_compact_command_summarises_a_kept_session_and_the_log_restarts() {
    let stage = Stage::new();
    stage.script([says("Hello there.")]);
    stage.script([says("<summary>They greeted each other.</summary>")]);
    stage.script([says("Yes.")]);

    let session = stage.play("first prompt", &stage.request("hi"));
    let mut compact = stage.resuming(&session, "");
    compact.session.command = Some(SessionCommand::Compact);
    stage.play("compact command", &compact);
    stage.play("prompt after compaction", &stage.resuming(&session, "Still there?"));

    stage.assert(
        r##"
        == first prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Text "Hello there."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == first prompt: conversation ==
        user:
          text "hi"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "Hello there."
        == first prompt: log appended ==
        prompted "hi"
        event SessionStarted
        opened
        event Checkpoint
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=1
        event Usage
        message assistant
        event Completed
        == compact command: events ==
        SessionStarted <session-1>
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Compacted before=<tokens> after=<tokens> shrank=true summary="They greeted each other."
        Completed duration=<ms> cost=none
        == compact command: conversation ==
        user:
          text "This session continues an earlier conversation, compacted into this summary:\n\nThey greeted each other."
          text "If you need specific details from before compaction (exact code, error messages, or content you wrote), read the full transcript at: <tmp>/<brand>-tool-results/<session-1>/…
        == compact command: log appended ==
        event SessionStarted
        opened
        called model=claude-opus-5-5 stop=EndTurn messages=3
        event Usage
        restarted
        message user
        event Compacted
        event Completed
        == prompt after compaction: events ==
        SessionStarted <session-1>
        Checkpoint(1)
        Text "Yes."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt after compaction: conversation ==
        user:
          text "This session continues an earlier conversation, compacted into this summary:\n\nThey greeted each other."
          text "If you need specific details from before compaction (exact code, error messages, or content you wrote), read the full transcript at: <tmp>/<brand>-tool-results/<session-1>/…
        user:
          text "Still there?"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "Yes."
        == prompt after compaction: log appended ==
        prompted "Still there?"
        event SessionStarted
        opened
        event Checkpoint
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=2
        event Usage
        message assistant
        event Completed
        "##,
    );
}

#[test]
fn the_compact_command_without_a_session_fails_before_anything_starts() {
    let stage = Stage::new();
    let mut compact = stage.request("");
    compact.session.command = Some(SessionCommand::Compact);

    stage.play("compact command", &compact);

    stage.assert(
        r##"
        == compact command: events ==
        Failure(Other)
        Failed("There is no conversation to work on yet.")
        "##,
    );
}
