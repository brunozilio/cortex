use std::sync::Arc;

use serde_json::json;

use crate::models::{ByTask, call, says};
use crate::stage::{Order, Stage};

const TASK: &str = "Read notes.txt and report what it says";

#[test]
fn a_sub_agent_s_events_are_wrapped_between_its_spawn_and_its_report_to_the_parent() {
    let stage = Stage::new();
    stage.file("notes.txt", "alpha\n");
    stage.script([
        call("a1", "Agent", json!({"description": "Read the notes", "prompt": TASK})),
        call("r1", "Read", json!({"file_path": "notes.txt"})),
        says("The notes say alpha."),
        says("The sub-agent found: alpha."),
    ]);

    stage.play("prompt", &stage.request("Delegate reading the notes"));

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted a1 Agent "Read the notes" input="{\"description\":\"Read the notes\",\"prompt\":\"Read notes.txt and report what it says\"}"
        AgentSpawned #0 kind=general-purpose summary="Read the notes" prompt="Read notes.txt and report what it says"
        AgentUpdate #0 Running(0) summary=None
        Agent #0 | Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Agent #0 | ToolStarted r1 Read "notes.txt" input="{\"file_path\":\"notes.txt\"}"
        Agent #0 | ToolFinished r1
        Agent #0 | ToolResult r1 ok "     1\talpha\n"
        Agent #0 | Text "The notes say alpha."
        Agent #0 | Usage model=claude-opus-5-5 input=10 output=5 cost=none
        AgentUpdate #0 Done(<seconds>) summary=Some("Read the notes")
        ToolFinished a1
        ToolResult a1 ok "The notes say alpha.\n\nagent_id: agent-1 (SendMessage continues it)"
        Text "The sub-agent found: alpha."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Delegate reading the notes"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          tool_use a1 Agent "{\"description\":\"Read the notes\",\"prompt\":\"Read notes.txt and report what it says\"}"
        user:
          tool_result a1 ok "The notes say alpha.\n\nagent_id: agent-1 (SendMessage continues it)"
        assistant:
          text "The sub-agent found: alpha."
        == prompt: log appended ==
        prompted "Delegate reading the notes"
        event SessionStarted
        opened
        event Checkpoint
        message user
        called model=claude-opus-5-5 stop=ToolUse messages=1
        event Usage
        message assistant
        event ToolStarted a1
        event AgentSpawned
        event AgentUpdate
        called model=claude-opus-5-5 stop=ToolUse messages=1 agent=#0
        event Agent #0 | Usage
        event Agent #0 | ToolStarted r1
        event Agent #0 | ToolFinished r1
        event Agent #0 | ToolResult r1
        event Agent #0 | TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=3 agent=#0
        event Agent #0 | Usage
        event AgentUpdate
        event ToolFinished a1
        event ToolResult a1
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
fn a_background_sub_agent_is_collected_before_the_run_completes() {
    let stage = Stage::new();
    stage.file("notes.txt", "alpha\n");
    let model = ByTask::new([
        call("a1", "Agent", json!({"description": "Read the notes", "prompt": TASK, "run_in_background": true})),
        says("Started it; finishing."),
        says("The background agent found: alpha."),
    ])
    .task(TASK, [call("r1", "Read", json!({"file_path": "notes.txt"})), says("The notes say alpha.")]);
    stage.model(Arc::new(model));

    let request = stage.request("Delegate reading the notes in the background");
    stage.end_ordered("prompt", stage.start(&request), Order::PerLane);

    stage.assert(
        r##"
        == prompt: events ==
        [main lane]
        SessionStarted <session-1>
        Checkpoint(0)
        ToolStarted a1 Agent "Read the notes" input="{\"description\":\"Read the notes\",\"prompt\":\"Read notes.txt and report what it says\",\"run_in_background\":true}"
        ToolFinished a1
        ToolResult a1 ok "{\"instructions\":\"Use TaskOutput to collect the report before completing the request.\",\"status\":\"running\",\"task_id\":\"task-1\"}"
        Text "Started it; finishing."
        Text "The background agent found: alpha."
        Completed duration=<ms> cost=none
        [agent #0 lane]
        AgentSpawned #0 kind=general-purpose summary="Read the notes" prompt="Read notes.txt and report what it says"
        AgentUpdate #0 Running(0) summary=None
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted r1 Read "notes.txt" input="{\"file_path\":\"notes.txt\"}"
        ToolFinished r1
        ToolResult r1 ok "     1\talpha\n"
        Text "The notes say alpha."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        AgentUpdate #0 Done(<seconds>) summary=Some("Read the notes")
        [main lane usage]
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        == prompt: conversation ==
        user:
          text "Delegate reading the notes in the background"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          tool_use a1 Agent "{\"description\":\"Read the notes\",\"prompt\":\"Read notes.txt and report what it says\",\"run_in_background\":true}"
        user:
          tool_result a1 ok "{\"instructions\":\"Use TaskOutput to collect the report before completing the request.\",\"status\":\"running\",\"task_id\":\"task-1\"}"
        assistant:
          text "Started it; finishing."
        user:
          text "Background agents completed. Incorporate their reports into your answer:\nThe notes say alpha.\n\nagent_id: agent-1 (SendMessage continues it)"
        assistant:
          text "The background agent found: alpha."
        == prompt: log appended ==
        [main lane]
        prompted "Delegate reading the notes in the background"
        event SessionStarted
        opened
        event Checkpoint
        message user
        called model=claude-opus-5-5 stop=ToolUse messages=1
        event Usage
        message assistant
        event ToolStarted a1
        event ToolFinished a1
        event ToolResult a1
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=3
        event Usage
        message assistant
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=5
        event Usage
        message assistant
        event Completed
        [agent #0 lane]
        event AgentSpawned
        event AgentUpdate
        called model=claude-opus-5-5 stop=ToolUse messages=1 agent=#0
        event Agent #0 | Usage
        event Agent #0 | ToolStarted r1
        event Agent #0 | ToolFinished r1
        event Agent #0 | ToolResult r1
        event Agent #0 | TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=3 agent=#0
        event Agent #0 | Usage
        event AgentUpdate
        "##,
    );
}
