use serde_json::json;

use crate::hooks::command;
use crate::models::says;
use crate::stage::Stage;

#[test]
fn a_stop_hook_that_blocks_once_sends_the_model_back_to_work_then_lets_it_finish() {
    let stage = Stage::new();
    let once = "input=$(cat); case \"$input\" in *'\"stop_hook_active\":false'*) \
                echo '{\"decision\":\"block\",\"reason\":\"Verify once before finishing\"}';; *) echo '{}';; esac";
    stage.settings(&json!({"hooks": {"Stop": command(once)}}));
    stage.script([says("Finished."), says("Verified, finished.")]);

    stage.play("prompt", &stage.request("Do the work"));

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Text "Finished."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Text "Verified, finished."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Do the work"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "Finished."
        user:
          text "Completion blocked by hook: Verify once before finishing"
        assistant:
          text "Verified, finished."
        == prompt: log appended ==
        prompted "Do the work"
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
        event Completed
        "##,
    );
}

#[test]
fn a_stop_hook_that_always_blocks_is_overruled_after_three_blocks_with_a_notice() {
    let stage = Stage::new();
    stage.settings(&json!({"hooks": {"Stop": command("echo 'never done' >&2; exit 2")}}));
    let model = stage.script([says("One."), says("Two."), says("Three."), says("Four."), says("never asked")]);

    stage.play("prompt", &stage.request("Do the work"));

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
        Notice("The Stop hook blocked completion 3 times in a row and blocked it again, so the turn ended anyway. Its reason: never done")
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "Do the work"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "One."
        user:
          text "Completion blocked by hook: never done"
        assistant:
          text "Two."
        user:
          text "Completion blocked by hook: never done"
        assistant:
          text "Three."
        user:
          text "Completion blocked by hook: never done"
        assistant:
          text "Four."
        == prompt: log appended ==
        prompted "Do the work"
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
        event Notice
        event Completed
        "##,
    );
}
