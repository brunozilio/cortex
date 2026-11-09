//! Reducer rules for side calls: the model's title for a new session, the
//! suggested next prompt, the recap of a session from an earlier launch,
//! `/btw` questions and the output style every prompt carries.

use std::path::PathBuf;

use superfik_core::{
    ChoreAnswer, EngineEvent, EngineKind, ModelOption, OutputStyle, PromptRequest, RunId, SessionCommand, SessionId,
};
use superfik_protocol::{ClientCommand, ServerEvent};

use super::tests::commands;
use super::*;

fn session() -> Session {
    let mut s = Session::new(Some(PathBuf::from("/repo")));
    s.models.push(ModelOption::new(EngineKind::Native, "m", "M"));
    s
}

fn submits(effects: &[Effect]) -> Vec<(RunId, PromptRequest)> {
    commands(effects)
        .into_iter()
        .filter_map(|c| match c {
            ClientCommand::SubmitPrompt { run, request } => Some((*run, request.clone())),
            _ => None,
        })
        .collect()
}

fn of(submitted: &[(RunId, PromptRequest)], command: Option<SessionCommand>) -> Option<(RunId, PromptRequest)> {
    submitted.iter().find(|(_, request)| request.command == command).cloned()
}

fn event(s: &mut Session, run: RunId, event: EngineEvent) -> Vec<Effect> {
    s.apply(Action::Server(ServerEvent::Run { run, event }), 2.0)
}

fn completed() -> EngineEvent {
    EngineEvent::Completed { duration_ms: 10, cost_usd: None }
}

fn send(s: &mut Session, text: &str) -> (RunId, Option<(RunId, PromptRequest)>) {
    s.apply(Action::Edit(EditKey::Insert(text.into())), 0.0);
    let submitted = submits(&s.apply(Action::Submit, 1.0));
    assert_eq!(submitted.len(), 1, "a send submits the prompt alone");
    let (run, _) = of(&submitted, None).expect("the prompt");
    let started = submits(&event(s, run, EngineEvent::SessionStarted(SessionId("abc".into()))));
    (run, of(&started, Some(SessionCommand::Title)))
}

fn answered(s: &mut Session) -> (RunId, PromptRequest) {
    let (run, _) = send(s, "fix the login page");
    event(s, run, EngineEvent::TextDelta("Fixed.".into()));
    of(&submits(&event(s, run, completed())), Some(SessionCommand::Suggest)).expect("a suggestion is asked for")
}

#[test]
fn the_first_prompt_asks_the_model_for_a_title_that_replaces_the_cut_one() {
    let mut s = session();
    let (_, title) = send(&mut s, "please fix the clip rectangles in renderer.rs");
    let (title_run, request) = title.expect("the first prompt names the session");
    assert_eq!(request.prompt, "please fix the clip rectangles in renderer.rs");
    assert_eq!(request.resume, None, "a title needs no engine session");
    assert!(!s.conversation().owns_upkeep(title_run), "it does not hold the measurement's place");
    event(&mut s, title_run, EngineEvent::Chore(ChoreAnswer::Title("Renderer.rs clip rectangles".into())));
    event(&mut s, title_run, completed());
    assert_eq!(s.conversation().title.as_deref(), Some("Renderer.rs clip rectangles"));
    assert!(s.conversation().chores.running.is_empty());
    assert!(s.conversation().turns.iter().all(|turn| turn.response.text.is_empty()), "never in the transcript");
}

#[test]
fn a_title_the_user_changed_meanwhile_stays() {
    let mut s = session();
    let (_, title) = send(&mut s, "fix the login page");
    let (title_run, _) = title.expect("title");
    s.conversation_mut().title = Some("Mine".into());
    event(&mut s, title_run, EngineEvent::Chore(ChoreAnswer::Title("Login page".into())));
    assert_eq!(s.conversation().title.as_deref(), Some("Mine"));
}

#[test]
fn a_follow_up_asks_for_no_title() {
    let mut s = session();
    answered(&mut s);
    assert!(send(&mut s, "and the tests").1.is_none());
}

#[test]
fn a_reply_brings_a_suggestion_that_tab_takes_and_typing_hides() {
    let mut s = session();
    let (run, request) = answered(&mut s);
    assert_eq!(request.resume, Some(SessionId("abc".into())));
    event(&mut s, run, EngineEvent::Chore(ChoreAnswer::Suggestion("run the tests".into())));
    assert_eq!(s.suggestion(), Some("run the tests"));
    assert_eq!(s.completion().as_deref(), Some("run the tests"), "shown where Tab completes");

    s.apply(Action::Edit(EditKey::Insert("x".into())), 3.0);
    assert_eq!(s.suggestion(), None, "typing hides it");
    s.apply(Action::Edit(EditKey::Backspace), 3.0);
    s.apply(Action::AcceptCompletion, 3.0);
    assert_eq!(s.conversation().composer.text(), "run the tests");
    assert_eq!(s.suggestion(), None);
}

#[test]
fn a_suggestion_arriving_after_the_user_typed_is_dropped() {
    let mut s = session();
    let (run, _) = answered(&mut s);
    s.apply(Action::Edit(EditKey::Insert("my own".into())), 3.0);
    event(&mut s, run, EngineEvent::Chore(ChoreAnswer::Suggestion("run the tests".into())));
    s.conversation_mut().composer.take();
    assert_eq!(s.suggestion(), None);
}

#[test]
fn btw_asks_beside_the_run_and_shows_the_answer() {
    let mut s = session();
    let (run, _) = send(&mut s, "fix the login page");
    event(&mut s, run, EngineEvent::SessionStarted(SessionId("abc".into())));
    s.apply(Action::Edit(EditKey::Insert("/btw what is it doing?".into())), 2.0);
    let (side, request) = of(&submits(&s.apply(Action::Submit, 2.0)), Some(SessionCommand::Side)).expect("side");
    assert_eq!(request.prompt, "what is it doing?");
    assert!(s.is_running(), "the run goes on");
    assert!(s.conversation().composer.text().is_empty());
    event(&mut s, side, EngineEvent::Chore(ChoreAnswer::Side("Reading the router.".into())));
    assert_eq!(s.notice, Some(Notice::SideAnswer("Reading the router.".into())));
    assert!(s.notice.as_ref().is_some_and(Notice::is_informative), "news, not a problem");
    assert_eq!(s.conversation().turns.len(), 1, "the question is not a turn");
}

#[test]
fn btw_before_anything_was_said_explains_itself() {
    let mut s = session();
    s.apply(Action::Edit(EditKey::Insert("/btw what now?".into())), 0.0);
    assert!(submits(&s.apply(Action::Submit, 0.0)).is_empty());
    assert_eq!(s.notice, Some(Notice::NothingToAsk));
}

#[test]
fn a_session_from_an_earlier_launch_is_recapped_once_when_shown() {
    let mut s = session();
    let earlier = s.active;
    let (run, _) = send(&mut s, "fix the login page");
    event(&mut s, run, EngineEvent::SessionStarted(SessionId("abc".into())));
    event(&mut s, run, completed());
    s.conversation_mut().chores.recapped = false;
    s.apply(Action::NewConversation, 3.0);
    let recap = of(&submits(&s.apply(Action::SelectConversation(earlier), 3.0)), Some(SessionCommand::Recap));
    let (recap_run, _) = recap.expect("recapped when shown again");
    event(&mut s, recap_run, EngineEvent::Chore(ChoreAnswer::Recap("Moving the login page.".into())));
    assert_eq!(s.notice, Some(Notice::Recap("Moving the login page.".into())));
    s.apply(Action::NewConversation, 4.0);
    assert!(of(&submits(&s.apply(Action::SelectConversation(earlier), 4.0)), Some(SessionCommand::Recap)).is_none());
}

#[test]
fn every_prompt_carries_the_output_style_picked_in_settings() {
    let mut s = session();
    s.apply(Action::ChooseSetting(Setting::OutputStyle(OutputStyle::Concise)), 0.0);
    s.apply(Action::Edit(EditKey::Insert("hello".into())), 0.0);
    let (_, request) = of(&submits(&s.apply(Action::Submit, 1.0)), None).expect("prompt");
    assert_eq!(request.output_style.as_deref(), Some("concise"));
}

#[test]
fn suggestions_turned_off_are_neither_asked_for_nor_shown() {
    let mut s = session();
    let (run, _) = answered(&mut s);
    event(&mut s, run, EngineEvent::Chore(ChoreAnswer::Suggestion("run the tests".into())));
    s.apply(Action::ChooseSetting(Setting::Suggestions(false)), 3.0);
    assert_eq!(s.suggestion(), None, "one already there hides");
    let (next, _) = send(&mut s, "and the docs");
    let after = submits(&event(&mut s, next, completed()));
    assert!(of(&after, Some(SessionCommand::Suggest)).is_none(), "no call is made");
}

#[test]
fn btw_from_the_picker_starts_the_line_for_the_question() {
    let mut s = session();
    s.apply(Action::RunCommand(superfik_core::Command::Btw), 0.0);
    assert_eq!(s.conversation().composer.text(), "/btw ");
}

#[test]
fn long_work_brings_a_heads_up_and_short_work_does_not() {
    let mut s = session();
    let (run, _) = send(&mut s, "migrate the cache");
    for step in 0..8 {
        let call = superfik_core::ToolCall::new(format!("t{step}"), "Edit", "cache.rs");
        event(&mut s, run, EngineEvent::ToolStarted(call));
    }
    let after = submits(&event(&mut s, run, completed()));
    let (look, _) = of(&after, Some(SessionCommand::HeadsUp)).expect("long work is looked at");
    let found = superfik_core::HeadsUp {
        urgent: true,
        learn: "The cache is now shared between users.".into(),
        title: "Cached answers are shared".into(),
        explain: "Keyed by query only.".into(),
    };
    event(&mut s, look, EngineEvent::Chore(ChoreAnswer::HeadsUp(found.clone())));
    assert_eq!(s.notice, Some(Notice::HeadsUp(found)));
    assert!(s.notice.as_ref().is_some_and(Notice::is_informative));

    let (short, _) = send(&mut s, "and the docs");
    let after = submits(&event(&mut s, short, completed()));
    assert!(of(&after, Some(SessionCommand::HeadsUp)).is_none(), "a short reply is not looked at");
}

#[test]
fn rename_sets_the_title_or_asks_the_model_for_one() {
    let mut s = session();
    let (run, _) = send(&mut s, "fix the login page");
    event(&mut s, run, completed());
    s.apply(Action::Edit(EditKey::Insert("/rename Login rework".into())), 3.0);
    assert!(submits(&s.apply(Action::Submit, 3.0)).is_empty());
    assert_eq!(s.conversation().title.as_deref(), Some("Login rework"));

    s.apply(Action::Edit(EditKey::Insert("/rename".into())), 4.0);
    let (named, _) = of(&submits(&s.apply(Action::Submit, 4.0)), Some(SessionCommand::Rename)).expect("asked");
    event(&mut s, named, EngineEvent::Chore(ChoreAnswer::Title("Login page bug".into())));
    assert_eq!(s.conversation().title.as_deref(), Some("Login page bug"), "a rename asked for always takes");
}
