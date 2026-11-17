use agent_engine::model::ModelError;

use crate::models::says;
use crate::stage::Stage;

#[test]
fn an_overloaded_model_with_no_fallback_fails_the_run_as_unavailable() {
    let stage = Stage::new();
    let model = stage.script([Err(ModelError::Overloaded("busy".into())), says("never asked")]);

    stage.play("prompt", &stage.request("hi"));

    assert_eq!(model.asked().len(), 1, "the engine itself does not retry; the provider clients do");
    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Failure(Unavailable)
        Failed("rate limited: busy")
        == prompt: conversation ==
        user:
          text "hi"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        == prompt: log appended ==
        prompted "hi"
        event SessionStarted
        opened
        event Checkpoint
        message user
        event Failure
        event Failed
        "##,
    );
}

#[test]
fn an_overloaded_model_falls_back_to_the_next_model_and_answers_once() {
    let stage = Stage::new();
    let model = stage.script([Err(ModelError::Overloaded("busy".into())), says("From the fallback.")]);
    let mut request = stage.request("hi");
    request.session.fallbacks = vec!["claude-sonnet-5".into()];

    stage.play("prompt", &request);

    let asked: Vec<String> = model.asked().iter().map(|turn| turn.model.clone()).collect();
    assert_eq!(asked, ["claude-opus-5-5", "claude-sonnet-5"]);
    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        FellBack("claude-sonnet-5")
        Text "From the fallback."
        Usage model=claude-sonnet-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == prompt: conversation ==
        user:
          text "hi"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "From the fallback."
        == prompt: log appended ==
        prompted "hi"
        event SessionStarted
        opened
        event Checkpoint
        message user
        event FellBack
        event TextDelta
        called model=claude-sonnet-5 stop=EndTurn messages=1
        event Usage
        message assistant
        event Completed
        "##,
    );
}

#[test]
fn a_conversation_too_long_for_the_model_is_compacted_and_the_turn_asked_again() {
    let stage = Stage::new();
    stage.script([says("Hello there.")]);
    let model = stage.script([
        Err(ModelError::TooLong("prompt is too long".into())),
        says("<summary>They greeted each other.</summary>"),
        says("Shorter now."),
    ]);

    let session = stage.play("first prompt", &stage.request("hi"));
    stage.play("second prompt", &stage.resuming(&session, "And now?"));

    assert_eq!(model.asked().len(), 3);
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
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Compacted before=<tokens> after=<tokens> shrank=true summary="They greeted each other."
        Text "Shorter now."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == second prompt: conversation ==
        user:
          text "This session continues an earlier conversation, compacted into this summary:\n\nThey greeted each other."
          text "If you need specific details from before compaction (exact code, error messages, or content you wrote), read the full transcript at: <tmp>/<brand>-tool-results/<session-1>/…
        user:
          text "And now?"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "Shorter now."
        == second prompt: log appended ==
        prompted "And now?"
        event SessionStarted
        opened
        event Checkpoint
        message user
        called model=claude-opus-5-5 stop=EndTurn messages=4
        event Usage
        event Compacted
        restarted
        message user
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
fn a_provider_error_that_cannot_be_retried_fails_the_run_and_keeps_the_prompt() {
    let stage = Stage::new();
    stage.script([Err(ModelError::Provider("the provider rejected the request".into()))]);

    stage.play("prompt", &stage.request("hi"));

    stage.assert(
        r##"
        == prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Failure(Other)
        Failed("the provider rejected the request")
        == prompt: conversation ==
        user:
          text "hi"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        == prompt: log appended ==
        prompted "hi"
        event SessionStarted
        opened
        event Checkpoint
        message user
        event Failure
        event Failed
        "##,
    );
}

#[test]
fn a_run_without_an_account_is_diagnosed_as_not_logged_in_and_creates_no_session() {
    let stage = Stage::new();

    stage.play("prompt", &stage.request("hi"));
    let kept = std::fs::read_dir(&stage.store).expect("the store").flatten();
    stage.note("files in the session store", kept.map(|file| file.file_name().to_string_lossy().into_owned()));

    stage.assert(
        r##"
        == prompt: events ==
        Diagnosed(NotLoggedIn)
        Failure(Other)
        Failed("not signed in: no account connected")
        == files in the session store ==
        "##,
    );
}
