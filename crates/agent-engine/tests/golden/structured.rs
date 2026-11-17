use serde_json::json;

use crate::models::{call, says};
use crate::stage::Stage;

fn contract(stage: &Stage) {
    stage.settings(&json!({"jsonSchema": {
        "type": "object",
        "properties": {"answer": {"type": "string"}},
        "required": ["answer"],
        "additionalProperties": false
    }}));
}

#[test]
fn a_structured_result_is_emitted_when_the_model_calls_the_output_tool() {
    let stage = Stage::new();
    contract(&stage);
    stage.script([call("s1", "StructuredOutput", json!({"result": {"answer": "42"}})), says("Reported.")]);

    stage.play("prompt", &stage.request("What is the answer?"));

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted s1 StructuredOutput "Structured result" input="{\"result\":{\"answer\":\"42\"}}"
        ToolFinished s1
        ToolResult s1 ok "Structured result accepted."
        StructuredOutput {"answer":"42"}
        Text "Reported."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "What is the answer?"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          tool_use s1 StructuredOutput "{\"result\":{\"answer\":\"42\"}}"
        user:
          tool_result s1 ok "Structured result accepted."
        assistant:
          text "Reported."
        == prompt: log appended ==
        prompted "What is the answer?"
        event SessionStarted
        opened
        event Checkpoint
        message user
        called model=claude-opus-5-5 stop=ToolUse messages=1
        event Usage
        message assistant
        event ToolStarted s1
        event ToolFinished s1
        event ToolResult s1
        event StructuredOutput
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
fn a_model_that_forgets_the_structured_result_is_nudged_until_it_calls_the_tool() {
    let stage = Stage::new();
    contract(&stage);
    stage.script([
        says("The answer is 42."),
        call("s1", "StructuredOutput", json!({"result": {"answer": "42"}})),
        says("Reported."),
    ]);

    stage.play("prompt", &stage.request("What is the answer?"));

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Text "The answer is 42."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted s1 StructuredOutput "Structured result" input="{\"result\":{\"answer\":\"42\"}}"
        ToolFinished s1
        ToolResult s1 ok "Structured result accepted."
        StructuredOutput {"answer":"42"}
        Text "Reported."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "What is the answer?"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "The answer is 42."
        user:
          text "The configured response contract requires calling StructuredOutput with a valid result before completing. Use the tool now."
        assistant:
          tool_use s1 StructuredOutput "{\"result\":{\"answer\":\"42\"}}"
        user:
          tool_result s1 ok "Structured result accepted."
        assistant:
          text "Reported."
        == prompt: log appended ==
        prompted "What is the answer?"
        event SessionStarted
        opened
        event Checkpoint
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=1
        event Usage
        message assistant
        message user
        called model=claude-opus-5-5 stop=ToolUse messages=3
        event Usage
        message assistant
        event ToolStarted s1
        event ToolFinished s1
        event ToolResult s1
        event StructuredOutput
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=5
        event Usage
        message assistant
        event Completed
        "##,
    );
}

#[test]
fn a_structured_result_that_breaks_the_schema_is_refused_and_not_emitted() {
    let stage = Stage::new();
    contract(&stage);
    stage.script([
        call("s1", "StructuredOutput", json!({"result": {"answer": 42}})),
        call("s2", "StructuredOutput", json!({"result": {"answer": "42"}})),
        says("Reported."),
    ]);

    stage.play("prompt", &stage.request("What is the answer?"));

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted s1 StructuredOutput "Structured result" input="{\"result\":{\"answer\":42}}"
        ToolFinished s1
        ToolResult s1 failed "Invalid input for StructuredOutput, so it did not run: /result/answer: 42 is not of type \"string\". Check the tool's parameters and call it again."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        ToolStarted s2 StructuredOutput "Structured result" input="{\"result\":{\"answer\":\"42\"}}"
        ToolFinished s2
        ToolResult s2 ok "Structured result accepted."
        StructuredOutput {"answer":"42"}
        Text "Reported."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "What is the answer?"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          tool_use s1 StructuredOutput "{\"result\":{\"answer\":42}}"
        user:
          tool_result s1 failed "Invalid input for StructuredOutput, so it did not run: /result/answer: 42 is not of type \"string\". Check the tool's parameters and call it again."
        assistant:
          tool_use s2 StructuredOutput "{\"result\":{\"answer\":\"42\"}}"
        user:
          tool_result s2 ok "Structured result accepted."
        assistant:
          text "Reported."
        == prompt: log appended ==
        prompted "What is the answer?"
        event SessionStarted
        opened
        event Checkpoint
        message user
        called model=claude-opus-5-5 stop=ToolUse messages=1
        event Usage
        message assistant
        event ToolStarted s1
        event ToolFinished s1
        event ToolResult s1
        message user
        called model=claude-opus-5-5 stop=ToolUse messages=3
        event Usage
        message assistant
        event ToolStarted s2
        event ToolFinished s2
        event ToolResult s2
        event StructuredOutput
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=5
        event Usage
        message assistant
        event Completed
        "##,
    );
}

#[test]
fn a_model_that_never_gives_the_structured_result_fails_the_run_after_three_nudges() {
    let stage = Stage::new();
    contract(&stage);
    let model = stage.script([says("One."), says("Two."), says("Three."), says("Four."), says("never asked")]);

    stage.play("prompt", &stage.request("What is the answer?"));

    assert_eq!(model.asked().len(), 4);
    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Text "One."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Text "Two."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Text "Three."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Text "Four."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Failure(Other)
        Failed("The model was asked 3 times to call StructuredOutput and never did, so the run stopped without a structured result.")
        == prompt: conversation ==
        user:
          text "What is the answer?"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "One."
        user:
          text "The configured response contract requires calling StructuredOutput with a valid result before completing. Use the tool now."
        assistant:
          text "Two."
        user:
          text "The configured response contract requires calling StructuredOutput with a valid result before completing. Use the tool now."
        assistant:
          text "Three."
        user:
          text "The configured response contract requires calling StructuredOutput with a valid result before completing. Use the tool now."
        assistant:
          text "Four."
        == prompt: log appended ==
        prompted "What is the answer?"
        event SessionStarted
        opened
        event Checkpoint
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=1
        event Usage
        message assistant
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
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=7
        event Usage
        message assistant
        event Failure
        event Failed
        "##,
    );
}
