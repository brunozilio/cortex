//! Several conversations in one workspace: each run streams into the
//! conversation that started it, wherever it sits on the timeline.

use std::path::PathBuf;

use superfik_core::{Activity, EngineEvent, EngineProblem, RunId, SessionId};
use superfik_protocol::{ClientCommand, ServerEvent};

use super::*;

const CLOCK: WallClock = WallClock { unix_at_zero: 1_793_282_400, utc_offset_secs: 3600 };
const DAY: f64 = 86_400.0;

fn session() -> Session {
    let mut s = Session::new(Some(PathBuf::from("/repo")));
    s.clock = CLOCK;
    s
}

fn submit(s: &mut Session, text: &str, now: f64) -> RunId {
    s.apply(Action::Edit(EditKey::Insert(text.into())), now);
    let effects = s.apply(Action::Submit, now);
    match super::tests::commands(&effects).as_slice() {
        [ClientCommand::SubmitPrompt { run, .. }] => *run,
        other => panic!("unexpected {other:?}"),
    }
}

fn event(s: &mut Session, run: RunId, event: EngineEvent, now: f64) {
    s.apply(Action::Server(ServerEvent::Run { run, event }), now);
}

fn finish(s: &mut Session, run: RunId, now: f64) {
    event(s, run, EngineEvent::Completed { duration_ms: 10, cost_usd: None }, now);
}

fn titles(s: &Session, now: f64) -> Vec<(String, Vec<String>)> {
    s.timeline(now).into_iter().map(|g| (g.label, g.items.into_iter().map(|i| i.title).collect())).collect()
}

#[test]
fn starts_with_one_empty_conversation_listed_as_new() {
    let s = session();
    assert_eq!(s.conversations.len(), 1);
    assert!(s.conversation().is_empty());
    assert_eq!(titles(&s, 0.0), vec![("Today".to_owned(), vec!["New session".to_owned()])]);
}

#[test]
fn new_conversation_inherits_model_and_effort_and_drops_the_untouched_one_it_leaves() {
    let mut s = session();
    let first = s.active;
    s.apply(Action::NewConversation, 0.0);
    assert_eq!(s.conversations.len(), 1, "the untouched conversation left behind is dropped");
    assert_ne!(s.active, first, "but the user still gets a fresh one");

    s.apply(Action::SelectModel(2), 0.0);
    s.apply(Action::SetEffort(superfik_core::Effort::High), 0.0);
    submit(&mut s, "first", 1.0);
    s.apply(Action::NewConversation, 2.0);
    assert_eq!(s.conversations.len(), 2);
    assert_ne!(s.active, first);
    assert!(s.conversation().is_empty());
    assert_eq!(s.model().label, "Sonnet 5.5");
    assert_eq!(s.effort(), superfik_core::Effort::High);
    assert!(!s.is_running(), "the new conversation is idle");
    assert!(s.any_running(), "the first one still runs");
}

#[test]
fn first_prompt_titles_and_dates_the_conversation() {
    let mut s = session();
    let run = submit(&mut s, "/motion Make the sky move\nsecond line", 5.0);
    let c = s.conversation();
    assert_eq!(c.title.as_deref(), Some("Make the sky move"));
    assert_eq!(c.started_unix, Some(CLOCK.unix_at_zero + 5));
    finish(&mut s, run, 6.0);
    submit(&mut s, "and again", 7.0);
    assert_eq!(s.conversation().title.as_deref(), Some("Make the sky move"), "follow-ups keep the title");
}

#[test]
fn each_run_streams_into_its_own_conversation() {
    let mut s = session();
    let a = s.active;
    let run_a = submit(&mut s, "sky", 1.0);
    s.apply(Action::NewConversation, 2.0);
    let b = s.active;
    let run_b = submit(&mut s, "moon", 3.0);
    assert!(s.any_running() && s.is_running());

    event(&mut s, run_a, EngineEvent::SessionStarted(SessionId("sa".into())), 3.5);
    event(&mut s, run_a, EngineEvent::TextDelta("stars".into()), 4.0);
    event(&mut s, run_b, EngineEvent::TextDelta("crater".into()), 4.1);
    finish(&mut s, run_a, 5.0);

    let find = |s: &Session, id| s.conversations.iter().find(|c| c.id == id).cloned().expect("conversation");
    let conv_a = find(&s, a);
    let conv_b = find(&s, b);
    assert_eq!(conv_a.turns[0].response.text, "stars");
    assert_eq!(conv_a.turns[0].response.activity, Activity::Done);
    assert_eq!(conv_a.engine_session, Some(SessionId("sa".into())));
    assert!(!conv_a.is_running());
    assert_eq!(conv_b.turns[0].response.text, "crater");
    assert!(conv_b.is_running());
    assert_eq!(conv_b.engine_session, None);

    s.apply(Action::SelectConversation(a), 6.0);
    assert!(!s.is_running());
    s.apply(Action::Edit(EditKey::Insert("more".into())), 6.0);
    let effects = s.apply(Action::Submit, 6.0);
    let [ClientCommand::CancelRun { .. }, ClientCommand::SubmitPrompt { request, .. }] =
        super::tests::commands(&effects).as_slice()
    else {
        panic!("expected a submit")
    };
    assert_eq!(request.resume, Some(SessionId("sa".into())));
}

#[test]
fn a_lost_session_only_resets_the_conversation_that_owned_it() {
    let mut s = session();
    s.conversation_mut().engine_session = Some(SessionId("keep".into()));
    let a = s.active;
    submit(&mut s, "one", 1.0);
    s.apply(Action::NewConversation, 2.0);
    s.conversation_mut().engine_session = Some(SessionId("gone".into()));
    let run_b = submit(&mut s, "two", 3.0);
    event(&mut s, run_b, EngineEvent::Diagnosed(EngineProblem::SessionNotFound), 4.0);
    assert_eq!(s.conversation().engine_session, None);
    assert!(s.notice.is_some());
    s.apply(Action::SelectConversation(a), 5.0);
    assert_eq!(s.conversation().engine_session, Some(SessionId("keep".into())));
}

#[test]
fn selecting_an_unknown_conversation_does_nothing() {
    let mut s = session();
    let active = s.active;
    s.popover = Some(PopoverKind::Model);
    s.apply(Action::SelectConversation(superfik_core::ConversationId::new()), 0.0);
    assert_eq!(s.active, active);
    assert_eq!(s.popover, Some(PopoverKind::Model));
    s.apply(Action::SelectConversation(active), 0.0);
    assert_eq!(s.popover, None);
}

#[test]
fn timeline_groups_by_local_day_newest_first_with_the_fresh_one_on_top() {
    let mut s = session();
    let old = submit(&mut s, "yesterday's work", -DAY);
    finish(&mut s, old, -DAY + 1.0);
    s.apply(Action::NewConversation, 0.0);
    let first = submit(&mut s, "morning", 10.0);
    finish(&mut s, first, 11.0);
    s.apply(Action::NewConversation, 20.0);
    let second = submit(&mut s, "afternoon", 30.0);
    s.apply(Action::NewConversation, 40.0);
    assert_eq!(
        titles(&s, 50.0),
        vec![
            ("Today".to_owned(), vec!["New session".to_owned(), "afternoon".to_owned(), "morning".to_owned()]),
            ("Yesterday".to_owned(), vec!["yesterday's work".to_owned()]),
        ]
    );
    let today = &s.timeline(50.0)[0];
    assert!(today.items[0].selected);
    assert!(today.items[1].running, "the afternoon run is in flight");
    assert!(!today.items[2].running);
    finish(&mut s, second, 60.0);
    assert!(!s.timeline(61.0)[0].items[1].running);
}

#[test]
fn leaving_the_workspace_cancels_every_run_and_starts_fresh() {
    let mut s = session();
    let run_a = submit(&mut s, "one", 1.0);
    s.apply(Action::NewConversation, 2.0);
    let run_b = submit(&mut s, "two", 3.0);
    s.apply(Action::SetEffort(superfik_core::Effort::Max), 3.0);
    let effects = s.apply(Action::OpenWorkspace("/other".into()), 4.0);
    let cancelled: Vec<RunId> = effects
        .iter()
        .filter_map(|e| match e {
            Effect::Command(c) => match c.as_ref() {
                ClientCommand::CancelRun { run } => Some(*run),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(cancelled, vec![run_a, run_b]);
    assert_eq!(s.conversations.len(), 1);
    assert!(s.conversation().is_empty());
    assert_eq!(s.effort(), superfik_core::Effort::Max, "model and effort carry over");
    assert_eq!(s.clock, CLOCK);
}

#[test]
fn branch_switch_waits_for_every_conversation() {
    let mut s = session();
    submit(&mut s, "one", 1.0);
    s.apply(Action::NewConversation, 2.0);
    assert!(!s.is_running());
    assert!(s.apply(Action::SelectBranch(superfik_core::BranchRef::local("dev")), 3.0).is_empty());
    assert_eq!(s.notice, Some(super::Notice::BusySwitch));
}

#[test]
fn leaving_an_untouched_conversation_drops_it_unless_something_was_typed() {
    let mut s = session();
    let run = submit(&mut s, "one", 1.0);
    finish(&mut s, run, 2.0);
    let first = s.active;
    s.apply(Action::NewConversation, 3.0);
    s.apply(Action::SelectConversation(first), 4.0);
    assert_eq!(s.conversations.len(), 1, "the empty one went away");
    s.apply(Action::NewConversation, 5.0);
    s.apply(Action::Edit(EditKey::Insert("draft".into())), 5.0);
    s.apply(Action::SelectConversation(first), 6.0);
    assert_eq!(s.conversations.len(), 2, "a draft is worth keeping");
    assert_eq!(s.timeline(7.0)[0].items.len(), 1, "but it is not on the timeline until sent");
}

#[test]
fn archiving_hides_a_conversation_and_moves_the_canvas_to_the_newest_one() {
    let mut s = session();
    let a = s.active;
    let run_a = submit(&mut s, "first", 1.0);
    finish(&mut s, run_a, 2.0);
    s.apply(Action::NewConversation, 3.0);
    let b = s.active;
    let run_b = submit(&mut s, "second", 4.0);
    s.apply(Action::ArchiveConversation(b), 5.0);
    assert_eq!(s.active, a, "falls back to the newest remaining");
    assert_eq!(s.timeline(6.0)[0].items.len(), 1);
    assert!(s.conversations.iter().any(|c| c.id == b && c.archived && c.is_running()), "its run still finishes");
    event(&mut s, run_b, EngineEvent::TextDelta("late".into()), 6.0);
    finish(&mut s, run_b, 7.0);
    assert_eq!(s.conversations.iter().find(|c| c.id == b).map(|c| c.turns[0].response.text.as_str()), Some("late"));
    s.apply(Action::ArchiveConversation(a), 8.0);
    assert!(s.conversation().is_empty(), "nothing left: a fresh conversation takes the canvas");
    assert_eq!(s.timeline(9.0)[0].items[0].title, "New session");
    s.apply(Action::SelectConversation(b), 9.0);
    assert_ne!(s.active, b, "archived conversations cannot be selected");
}

#[test]
fn archived_sessions_wait_in_a_folded_group_until_restored() {
    let mut s = session();
    let a = s.active;
    let run = submit(&mut s, "first", 1.0);
    finish(&mut s, run, 2.0);
    assert!(s.timeline(3.0).iter().all(|g| !g.archived), "no group while nothing is archived");

    s.apply(Action::ArchiveConversation(a), 4.0);
    let groups = s.timeline(5.0);
    let archived = groups.last().expect("groups");
    assert!(archived.archived && archived.collapsed, "folded by default");
    assert_eq!(archived.items.iter().map(|i| i.id).collect::<Vec<_>>(), vec![a]);
    assert!(archived.items.iter().all(|i| !i.selected));

    s.apply(Action::ToggleArchived, 6.0);
    assert!(!s.timeline(7.0).last().expect("groups").collapsed, "unfolded");
    s.apply(Action::ToggleArchived, 7.0);
    assert!(s.timeline(8.0).last().expect("groups").collapsed);

    s.apply(Action::UnarchiveConversation(a), 9.0);
    assert!(s.timeline(10.0).iter().all(|g| !g.archived), "the group goes with its last session");
    assert!(s.timeline(10.0).iter().flat_map(|g| &g.items).any(|i| i.id == a), "back among the days");
    s.apply(Action::SelectConversation(a), 11.0);
    assert_eq!(s.active, a, "restored sessions can be selected again");
}

#[test]
fn archived_sessions_are_listed_newest_first_and_skipped_by_stepping() {
    let mut s = session();
    let first = s.active;
    let run = submit(&mut s, "first", 1.0);
    finish(&mut s, run, 2.0);
    s.apply(Action::NewConversation, 3.0);
    let second = s.active;
    let run = submit(&mut s, "second", 4.0);
    finish(&mut s, run, 5.0);
    s.apply(Action::ArchiveConversation(first), 6.0);
    s.apply(Action::ArchiveConversation(second), 7.0);
    let groups = s.timeline(8.0);
    let ids: Vec<_> = groups.last().expect("groups").items.iter().map(|i| i.id).collect();
    assert_eq!(ids, vec![second, first]);
    let fresh = s.active;
    s.apply(Action::StepConversation(Toward::Older), 9.0);
    assert_eq!(s.active, fresh, "stepping never lands on an archived session");
}

#[test]
fn restoring_an_unknown_or_live_session_changes_nothing() {
    let mut s = session();
    let a = s.active;
    let run = submit(&mut s, "first", 1.0);
    finish(&mut s, run, 2.0);
    assert!(s.apply(Action::UnarchiveConversation(a), 3.0).is_empty(), "not archived: nothing to restore");
    assert_eq!(s.active, a);
}

#[test]
fn renaming_edits_the_title_inline_and_enter_or_escape_ends_it() {
    let mut s = session();
    let run = submit(&mut s, "first prompt", 1.0);
    finish(&mut s, run, 2.0);
    let id = s.active;
    s.apply(Action::BeginRename(id), 3.0);
    assert!(s.renaming.as_ref().is_some_and(|(r, edit)| *r == id && edit.selected_text() == Some("first prompt")));
    s.apply(Action::Edit(EditKey::Insert("Night sky".into())), 3.0);
    assert!(s.conversation().composer.is_empty(), "typing goes to the title, not the composer");
    s.apply(Action::CommitRename, 4.0);
    assert_eq!(s.conversation().title.as_deref(), Some("Night sky"));
    assert!(s.renaming.is_none());

    s.apply(Action::BeginRename(id), 5.0);
    s.apply(Action::Edit(EditKey::Insert("thrown away".into())), 5.0);
    s.apply(Action::CancelRename, 6.0);
    assert_eq!(s.conversation().title.as_deref(), Some("Night sky"));

    s.apply(Action::BeginRename(id), 7.0);
    s.apply(Action::Edit(EditKey::Backspace), 7.0);
    s.apply(Action::CommitRename, 8.0);
    assert_eq!(s.conversation().title.as_deref(), Some("Night sky"), "an emptied title keeps the old one");
    s.apply(Action::BeginRename(superfik_core::ConversationId::new()), 9.0);
    assert!(s.renaming.is_none(), "unknown ids are ignored");
}

#[test]
fn clicking_a_day_folds_its_sessions_and_keeps_the_count() {
    let mut s = session();
    let old = submit(&mut s, "yesterday's work", -DAY);
    finish(&mut s, old, -DAY + 1.0);
    s.apply(Action::NewConversation, 0.0);
    submit(&mut s, "today", 1.0);
    let groups = s.timeline(2.0);
    assert!(groups.iter().all(|g| !g.collapsed));
    let yesterday = groups[1].day;
    s.apply(Action::ToggleGroup(yesterday), 3.0);
    let groups = s.timeline(4.0);
    assert!(!groups[0].collapsed && groups[1].collapsed);
    assert_eq!(groups[1].items.len(), 1, "the items are still listed for the count");
    s.apply(Action::ToggleGroup(yesterday), 5.0);
    assert!(!s.timeline(6.0)[1].collapsed);
}

#[test]
fn stepping_walks_down_the_timeline_and_wraps_around() {
    let mut s = session();
    let alone = s.active;
    s.apply(Action::StepConversation(Toward::Older), 0.0);
    assert_eq!(s.active, alone, "a lone session has nowhere to go");
    let old = submit(&mut s, "yesterday's work", -DAY);
    finish(&mut s, old, -DAY + 1.0);
    let yesterday = s.active;
    s.apply(Action::NewConversation, 0.0);
    submit(&mut s, "today", 1.0);
    let today = s.active;
    s.apply(Action::NewConversation, 2.0);

    s.apply(Action::StepConversation(Toward::Older), 3.0);
    assert_eq!(s.active, today, "down from the fresh session at the top");
    s.apply(Action::ToggleGroup(s.timeline(3.0)[1].day), 3.0);
    s.apply(Action::StepConversation(Toward::Older), 4.0);
    assert_eq!(s.active, yesterday, "a folded day is still stepped through");
    s.apply(Action::StepConversation(Toward::Older), 5.0);
    assert_eq!(s.active, today, "past the oldest it wraps; the untouched one was dropped when left");
    s.apply(Action::StepConversation(Toward::Newer), 6.0);
    assert_eq!(s.active, yesterday, "up from the newest wraps to the oldest");
}

#[test]
fn stepping_skips_archived_sessions() {
    let mut s = session();
    let a = submit(&mut s, "a", 0.0);
    finish(&mut s, a, 1.0);
    let first = s.active;
    s.apply(Action::NewConversation, 2.0);
    let b = submit(&mut s, "b", 3.0);
    finish(&mut s, b, 4.0);
    let middle = s.active;
    s.apply(Action::NewConversation, 5.0);
    submit(&mut s, "c", 6.0);
    s.apply(Action::ArchiveConversation(middle), 7.0);
    s.apply(Action::StepConversation(Toward::Older), 8.0);
    assert_eq!(s.active, first);
}

#[test]
fn scrolling_through_sessions_stops_at_the_ends() {
    let mut s = session();
    let alone = s.active;
    s.apply(Action::ScrollConversation(Toward::Older), 0.0);
    assert_eq!(s.active, alone, "a lone session has nowhere to go");
    let old = submit(&mut s, "older", 0.0);
    finish(&mut s, old, 1.0);
    let older = s.active;
    s.apply(Action::NewConversation, 2.0);
    let new = submit(&mut s, "newer", 3.0);
    finish(&mut s, new, 4.0);
    let newer = s.active;

    s.apply(Action::ScrollConversation(Toward::Newer), 5.0);
    assert_eq!(s.active, newer, "the newest stays put instead of wrapping");
    s.apply(Action::ScrollConversation(Toward::Older), 6.0);
    assert_eq!(s.active, older);
    s.apply(Action::ScrollConversation(Toward::Older), 7.0);
    assert_eq!(s.active, older, "the oldest stays put instead of wrapping");
    s.apply(Action::ScrollConversation(Toward::Newer), 8.0);
    assert_eq!(s.active, newer);
}

#[test]
fn command_digits_select_the_visible_rows_in_timeline_order() {
    let mut s = session();
    let first = s.active;
    let run = submit(&mut s, "first", 1.0);
    finish(&mut s, run, 2.0);
    s.apply(Action::NewConversation, 3.0);
    let second = s.active;
    let run = submit(&mut s, "second", 4.0);
    finish(&mut s, run, 5.0);
    assert_eq!(s.shortcut_order(6.0), vec![second, first]);
    s.apply(Action::SelectShortcut(2), 6.0);
    assert_eq!(s.active, first);
    s.apply(Action::SelectShortcut(3), 6.0);
    assert_eq!(s.active, first, "there is no third row");
}

#[test]
fn only_nine_rows_have_a_command_digit_and_folded_days_have_none() {
    let mut s = session();
    for n in 0..10 {
        let run = submit(&mut s, &format!("prompt {n}"), 1.0 + n as f64);
        finish(&mut s, run, 1.5 + n as f64);
        s.apply(Action::NewConversation, 2.0 + n as f64);
    }
    assert_eq!(s.shortcut_order(20.0).len(), 9, "⌘1 … ⌘9 and no more");
    let day = s.timeline(20.0)[0].day;
    s.apply(Action::ToggleGroup(day), 20.0);
    assert!(s.shortcut_order(20.0).is_empty(), "a folded day's rows are not shown, so not numbered");
}

#[test]
fn trying_a_failed_turn_again_resends_its_prompt_and_keeps_the_draft() {
    let mut s = session();
    let run = submit(&mut s, "fix the build", 1.0);
    event(&mut s, run, EngineEvent::Failed("Overloaded".into()), 2.0);
    s.apply(Action::Edit(EditKey::Insert("half a thought".into())), 3.0);
    let effects = s.apply(Action::RetryTurn(0), 4.0);
    let sent = super::tests::commands(&effects);
    assert!(matches!(sent.as_slice(), [ClientCommand::SubmitPrompt { .. }]), "one new prompt");
    assert_eq!(s.conversation().turns.len(), 2, "a new turn for the retry");
    assert_eq!(s.conversation().turns[1].user.text, "fix the build");
    assert_eq!(s.conversation().composer.text(), "half a thought", "the draft comes back");
}

#[test]
fn only_a_failed_turn_can_be_tried_again() {
    let mut s = session();
    let run = submit(&mut s, "first", 1.0);
    finish(&mut s, run, 2.0);
    assert!(s.apply(Action::RetryTurn(0), 3.0).is_empty(), "a finished turn has no retry");
    assert_eq!(s.conversation().turns.len(), 1);
}

fn attention_of(s: &Session, id: ConversationId) -> Option<super::timeline::Attention> {
    s.timeline(0.0).iter().flat_map(|g| g.items.iter()).find(|i| i.id == id).and_then(|i| i.attention)
}

#[test]
fn a_run_that_ends_out_of_sight_asks_to_be_looked_at_until_it_is_shown() {
    let mut s = session();
    let first = s.active;
    let run = submit(&mut s, "first", 1.0);
    s.apply(Action::NewConversation, 2.0);
    finish(&mut s, run, 3.0);
    assert_eq!(attention_of(&s, first), Some(super::timeline::Attention::Done));
    s.apply(Action::SelectConversation(first), 4.0);
    assert_eq!(attention_of(&s, first), None, "seen");
}

#[test]
fn a_failed_run_out_of_sight_says_so_and_a_finished_one_in_sight_does_not() {
    let mut s = session();
    let first = s.active;
    let run = submit(&mut s, "first", 1.0);
    finish(&mut s, run, 2.0);
    assert_eq!(attention_of(&s, first), None, "watched as it ended");
    s.apply(Action::NewConversation, 3.0);
    let second = s.active;
    let run = submit(&mut s, "second", 4.0);
    s.apply(Action::SelectConversation(first), 5.0);
    event(&mut s, run, EngineEvent::Failed("boom".into()), 6.0);
    assert_eq!(attention_of(&s, second), Some(super::timeline::Attention::Failed), "it failed out of sight");
}

#[test]
fn an_open_question_always_shows_even_while_the_session_is_on_screen() {
    let mut s = session();
    let id = s.active;
    let run = submit(&mut s, "first", 1.0);
    let kind =
        superfik_core::AskKind::Permission { tool: "Bash".into(), subject: "cargo test".into(), rememberable: false };
    let ask = superfik_core::AgentAsk { id: superfik_core::AskId("ask-1".into()), kind };
    event(&mut s, run, EngineEvent::Asked(ask), 2.0);
    assert_eq!(attention_of(&s, id), Some(super::timeline::Attention::Asking));
}
