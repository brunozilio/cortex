use agent_engine::transcript::{Block, StopReason};

use crate::models::{says, stopping};
use crate::stage::Stage;

#[test]
fn a_refusal_stop_reason_fails_the_run_and_keeps_what_was_said() {
    let stage = Stage::new();
    stage.script([stopping(vec![Block::Text("I cannot help with that.".into())], StopReason::Refusal)]);

    stage.play("prompt", &stage.request("Do the thing"));

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Text "I cannot help with that."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Failure(Other)
        Failed("The model declined to continue with this request.")
        == prompt: conversation ==
        user:
          text "Do the thing"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "I cannot help with that."
        == prompt: log appended ==
        prompted "Do the thing"
        event SessionStarted
        opened
        event Checkpoint
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=Refusal messages=1
        event Usage
        message assistant
        event Failure
        event Failed
        "##,
    );
}

#[test]
fn a_reply_cut_at_the_output_limit_is_nudged_to_continue_until_it_ends() {
    let stage = Stage::new();
    let model = stage.script([
        stopping(vec![Block::Text("The answer starts".into())], StopReason::MaxTokens),
        says(" and ends here."),
    ]);

    stage.play("prompt", &stage.request("Write a lot"));

    assert_eq!(model.asked().len(), 2);
    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Text "The answer starts"
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Text " and ends here."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Write a lot"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "The answer starts"
        user:
          text "Your output reached its token limit. Continue exactly at the cutoff, with no apology and no recap. New text is appended to the same response, so preserve code fences and ma…
        assistant:
          text " and ends here."
        == prompt: log appended ==
        prompted "Write a lot"
        event SessionStarted
        opened
        event Checkpoint
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=MaxTokens messages=1
        event Usage
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
fn a_reply_cut_four_times_in_a_row_stops_the_run_at_the_cap() {
    let stage = Stage::new();
    let cut = || stopping(vec![Block::Text("more ".into())], StopReason::MaxTokens);
    let model = stage.script([cut(), cut(), cut(), cut(), says("never asked")]);

    stage.play("prompt", &stage.request("Write a lot"));

    assert_eq!(model.asked().len(), 4);
    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Text "more "
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Text "more "
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Text "more "
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Text "more "
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Failure(Other)
        Failed("The reply hit the output limit 4 times in a row, so the run stopped.")
        == prompt: conversation ==
        user:
          text "Write a lot"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "more "
        user:
          text "Your output reached its token limit. Continue exactly at the cutoff, with no apology and no recap. New text is appended to the same response, so preserve code fences and ma…
        assistant:
          text "more "
        user:
          text "Your output reached its token limit. Continue exactly at the cutoff, with no apology and no recap. New text is appended to the same response, so preserve code fences and ma…
        assistant:
          text "more "
        user:
          text "Your output reached its token limit. Continue exactly at the cutoff, with no apology and no recap. New text is appended to the same response, so preserve code fences and ma…
        assistant:
          text "more "
        == prompt: log appended ==
        prompted "Write a lot"
        event SessionStarted
        opened
        event Checkpoint
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=MaxTokens messages=1
        event Usage
        message assistant
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=MaxTokens messages=3
        event Usage
        message assistant
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=MaxTokens messages=5
        event Usage
        message assistant
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=MaxTokens messages=7
        event Usage
        message assistant
        event Failure
        event Failed
        "##,
    );
}

#[test]
fn a_reply_cut_while_still_thinking_gets_the_briefer_reasoning_nudge() {
    let stage = Stage::new();
    stage.script([stopping(Vec::new(), StopReason::MaxTokens), says("Short answer.")]);

    stage.play("prompt", &stage.request("Think hard"));

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Text "Short answer."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Think hard"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
          text "Your output reached its token limit while you were still thinking, before any answer. Reason much more briefly and get to the answer or the next tool call."
        assistant:
          text "Short answer."
        == prompt: log appended ==
        prompted "Think hard"
        event SessionStarted
        opened
        event Checkpoint
        message user
        called model=claude-opus-5-5 stop=MaxTokens messages=1
        event Usage
        restarted
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
fn a_reply_that_is_empty_twice_ends_the_run_and_saves_no_assistant_message() {
    let stage = Stage::new();
    let empty = || stopping(Vec::new(), StopReason::EndTurn);
    let model = stage.script([empty(), empty(), says("never asked")]);

    stage.play("prompt", &stage.request("Say something"));

    assert_eq!(model.asked().len(), 2, "an empty reply is asked for once more, then accepted");
    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Say something"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        == prompt: log appended ==
        prompted "Say something"
        event SessionStarted
        opened
        event Checkpoint
        message user
        called model=claude-opus-5-5 stop=EndTurn messages=1
        event Usage
        called model=claude-opus-5-5 stop=EndTurn messages=1
        event Usage
        event Completed
        "##,
    );
}
