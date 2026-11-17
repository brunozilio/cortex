use std::path::PathBuf;

use serde_json::{Value, json};

use crate::models::{call, says};
use crate::stage::Stage;

pub fn command(command: &str) -> Value {
    json!([{"hooks": [{"type": "command", "command": command}]}])
}

fn journal(stage: &Stage, events: &[&str]) -> PathBuf {
    let journal = stage.home.join("hooks.journal");
    let (store, file) = (stage.store.display(), journal.display());
    let hooks: serde_json::Map<String, Value> = events
        .iter()
        .map(|event| {
            let count = |kind: &str| format!("$(cat {store}/*.jsonl 2>/dev/null | grep -c '\"kind\":\"{kind}\"')");
            let line = format!(
                "printf '%s events_logged=%s messages_saved=%s payload=' {event} \"{}\" \"{}\" >> {file}; \
                 cat >> {file}; echo >> {file}",
                count("event"),
                count("message")
            );
            ((*event).to_owned(), command(&line))
        })
        .collect();
    stage.settings(&json!({ "hooks": hooks }));
    journal
}

#[test]
fn a_pre_tool_use_hook_that_exits_2_blocks_the_call_and_the_model_reads_why() {
    let stage = Stage::new();
    stage.file("notes.txt", "alpha\n");
    stage.settings(&json!({"hooks": {"PreToolUse": [
        {"matcher": "Read", "hooks": [{"type": "command", "command": "echo 'reading is off today' >&2; exit 2"}]}
    ]}}));
    stage.script([call("r1", "Read", json!({"file_path": "notes.txt"})), says("I could not read it.")]);

    stage.play("prompt", &stage.request("Read the notes"));

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted r1 Read "notes.txt" input="{\"file_path\":\"notes.txt\"}"
        ToolFinished r1
        ToolResult r1 failed "Hook blocked Read: reading is off today"
        Text "I could not read it."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Read the notes"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          tool_use r1 Read "{\"file_path\":\"notes.txt\"}"
        user:
          tool_result r1 failed "Hook blocked Read: reading is off today"
        assistant:
          text "I could not read it."
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
fn a_user_prompt_submit_hook_adds_its_output_to_the_prompt_as_context() {
    let stage = Stage::new();
    stage.settings(&json!({"hooks": {"UserPromptSubmit": command("echo 'Team convention: tabs.'")}}));
    let model = stage.script([says("Noted.")]);

    stage.play("prompt", &stage.request("Format the file"));

    let sent = model.asked()[0].messages.last().map(|message| message.text()).unwrap_or_default();
    assert_eq!(sent, "Format the fileTeam convention: tabs.", "the context is a text block after the prompt");
    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Text "Noted."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Format the file"
          text "Team convention: tabs."
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "Noted."
        == prompt: log appended ==
        prompted "Format the file"
        event SessionStarted
        opened
        event Checkpoint
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=1
        event Usage
        message assistant
        event Completed
        "##,
    );
}

#[test]
fn session_start_fires_once_per_session_and_run_start_and_run_end_once_per_run_around_the_turn_s_other_hooks() {
    let stage = Stage::new();
    stage.file("notes.txt", "alpha\n");
    let journal = journal(
        &stage,
        &["SessionStart", "RunStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "Stop", "RunEnd", "SessionEnd"],
    );
    stage.script([call("r1", "Read", json!({"file_path": "notes.txt"})), says("Read.")]);
    stage.script([says("Again.")]);

    let session = stage.play("first prompt", &stage.request("Read the notes"));
    stage.play("second prompt", &stage.resuming(&session, "And again"));

    stage.disk("hooks journal", &journal);
    stage.assert(
        r##"
        == first prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted r1 Read "notes.txt" input="{\"file_path\":\"notes.txt\"}"
        ToolFinished r1
        ToolResult r1 ok "     1\talpha\n"
        Text "Read."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == first prompt: conversation ==
        user:
          text "Read the notes"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          tool_use r1 Read "{\"file_path\":\"notes.txt\"}"
        user:
          tool_result r1 ok "     1\talpha\n"
        assistant:
          text "Read."
        == first prompt: log appended ==
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
        == second prompt: events ==
        SessionStarted <session-1>
        Checkpoint(1)
        Text "Again."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == second prompt: conversation ==
        user:
          text "Read the notes"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          tool_use r1 Read "{\"file_path\":\"notes.txt\"}"
        user:
          tool_result r1 ok "     1\talpha\n"
        assistant:
          text "Read."
        user:
          text "And again"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "Again."
        == second prompt: log appended ==
        prompted "And again"
        event SessionStarted
        opened
        event Checkpoint
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=5
        event Usage
        message assistant
        event Completed
        == hooks journal ==
        SessionStart events_logged=0 messages_saved=0 payload={"agent_type":"main","cwd":"<project>","hook_event_name":"SessionStart","model":"claude-opus-5-5","session_id":"<session-1>","source":"startup"}
        RunStart events_logged=0 messages_saved=0 payload={"agent_type":"main","cwd":"<project>","hook_event_name":"RunStart","model":"claude-opus-5-5","session_id":"<session-1>","source":"startup"}
        UserPromptSubmit events_logged=0 messages_saved=0 payload={"agent_type":"main","cwd":"<project>","hook_event_name":"UserPromptSubmit","prompt":"Read the notes","session_id":"<session-1>"}
        PreToolUse events_logged=3 messages_saved=2 payload={"agent_type":"main","cwd":"<project>","hook_event_name":"PreToolUse","session_id":"<session-1>","tool_input":{"file_path":"notes.txt"},"tool_name":"Read","tool_use_id":"r1"}
        PostToolUse events_logged=3 messages_saved=2 payload={"agent_type":"main","cwd":"<project>","hook_event_name":"PostToolUse","session_id":"<session-1>","tool_input":{"file_path":"notes.txt"},"tool_name":"Read","tool_response":"     1\talpha\n","tool_use_id":"r1"}
        Stop events_logged=8 messages_saved=4 payload={"agent_type":"main","cwd":"<project>","hook_event_name":"Stop","last_assistant_message":"Read.","session_id":"<session-1>","stop_hook_active":false}
        RunEnd events_logged=8 messages_saved=4 payload={"agent_type":"main","cwd":"<project>","hook_event_name":"RunEnd","session_id":"<session-1>"}
        RunStart events_logged=9 messages_saved=4 payload={"agent_type":"main","cwd":"<project>","hook_event_name":"RunStart","model":"claude-opus-5-5","session_id":"<session-1>","source":"resume"}
        UserPromptSubmit events_logged=9 messages_saved=4 payload={"agent_type":"main","cwd":"<project>","hook_event_name":"UserPromptSubmit","prompt":"And again","session_id":"<session-1>"}
        Stop events_logged=13 messages_saved=6 payload={"agent_type":"main","cwd":"<project>","hook_event_name":"Stop","last_assistant_message":"Again.","session_id":"<session-1>","stop_hook_active":false}
        RunEnd events_logged=13 messages_saved=6 payload={"agent_type":"main","cwd":"<project>","hook_event_name":"RunEnd","session_id":"<session-1>"}
        "##,
    );
}
