//! Reducer rules for the composer's `/commands`: they toggle nodes and
//! controls instead of being sent to the model.

use std::path::PathBuf;

use superfik_core::Effort;
use superfik_protocol::ClientCommand;

use super::tests::commands;
use super::*;

fn session() -> Session {
    Session::new(Some(PathBuf::from("/repo")))
}

fn enter(s: &mut Session, text: &str) -> Vec<Effect> {
    s.apply(Action::Edit(EditKey::Insert(text.into())), 0.0);
    s.apply(Action::Submit, 1.0)
}

fn sent_prompt(effects: &[Effect]) -> bool {
    commands(effects).iter().any(|c| matches!(c, ClientCommand::SubmitPrompt { .. }))
}

#[test]
fn new_and_clear_start_fresh_sessions_and_keep_previous_runs() {
    for text in ["/new", "  /CLEAR "] {
        let mut s = session();
        s.apply(Action::SetEffort(Effort::High), 0.0);
        enter(&mut s, "first prompt");
        let previous = s.active;
        let model = s.conversation().model;
        s.conversation_mut().engine_session = Some(superfik_core::SessionId("previous-session".into()));

        let effects = enter(&mut s, text);
        assert!(!sent_prompt(&effects));
        assert_ne!(s.active, previous);
        assert!(s.conversation().is_empty());
        assert!(s.conversation().composer.is_empty());
        assert!(s.conversation().engine_session.is_none());
        assert_eq!(s.conversation().model, model);
        assert_eq!(s.effort(), Effort::High);
        assert_eq!(s.popover, None);
        let old = s.conversations.iter().find(|c| c.id == previous).expect("previous session retained");
        assert_eq!(old.turns.len(), 1);
        assert!(old.is_running());

        let effects = enter(&mut s, "fresh prompt");
        assert!(
            commands(&effects)
                .iter()
                .any(|c| { matches!(c, ClientCommand::SubmitPrompt { request, .. } if request.resume.is_none()) })
        );
    }
}

#[test]
fn new_and_clear_are_available_in_the_picker_and_start_fresh_sessions() {
    for text in ["/new", "/clear"] {
        let mut s = session();
        let previous = s.active;
        s.apply(Action::Edit(EditKey::Insert(text.into())), 0.0);
        let pick = s.highlighted_pick().expect("command offered");
        assert_eq!(pick.label(), text);
        let action = pick.action();
        let effects = s.apply(action, 1.0);
        assert!(!sent_prompt(&effects));
        assert_ne!(s.active, previous);
        assert_eq!(s.conversations.len(), 1, "untouched session is replaced");
        assert!(s.conversation().composer.is_empty());
        assert_eq!(s.popover, None);
    }
}

#[test]
fn optional_nodes_start_hidden() {
    let s = session();
    assert_eq!(s.nodes, ShownNodes::default());
    let n = s.nodes;
    assert!(!n.context && !n.memory && !n.skills && !n.plugins && !n.commands);
}

#[test]
fn a_node_command_toggles_its_node_and_is_not_a_prompt() {
    let mut s = session();
    let effects = enter(&mut s, "/memory");
    assert!(s.nodes.memory, "first time shows the node");
    assert!(!sent_prompt(&effects) && s.conversation().turns.is_empty());
    assert!(s.conversation().composer.is_empty(), "the command is consumed");
    enter(&mut s, "/memory");
    assert!(!s.nodes.memory, "second time hides it");
}

#[test]
fn each_node_command_flips_only_its_own_node() {
    let mut s = session();
    enter(&mut s, "/context");
    assert_eq!(s.nodes, ShownNodes { context: true, ..ShownNodes::default() });
    enter(&mut s, "/skills");
    enter(&mut s, "/plugins");
    assert_eq!(s.nodes, ShownNodes { context: true, skills: true, plugins: true, ..ShownNodes::default() });
}

#[test]
fn commands_ignore_case_and_surrounding_space() {
    let mut s = session();
    enter(&mut s, "  /Memory ");
    assert!(s.nodes.memory);
}

#[test]
fn popover_commands_toggle_the_popover() {
    let mut s = session();
    for (text, kind) in
        [("/model", PopoverKind::Model), ("/mode", PopoverKind::Permission), ("/repository", PopoverKind::Repository)]
    {
        enter(&mut s, text);
        assert_eq!(s.popover, Some(kind), "{text} opens");
        enter(&mut s, text);
        assert_eq!(s.popover, None, "{text} closes");
    }
}

#[test]
fn effort_steps_to_the_next_level_and_wraps() {
    let mut s = session();
    assert_eq!(s.effort(), Effort::Medium);
    enter(&mut s, "/effort");
    assert_eq!(s.effort(), Effort::High);
    s.apply(Action::SetEffort(Effort::Max), 0.0);
    enter(&mut s, "/effort");
    assert_eq!(s.effort(), Effort::Low);
}

#[test]
fn effort_with_a_level_sets_it_directly() {
    let mut s = session();
    for (text, effort) in [
        ("/effort low", Effort::Low),
        ("/effort medium", Effort::Medium),
        ("/effort HIGH", Effort::High),
        ("/effort xhigh", Effort::XHigh),
        ("/effort max", Effort::Max),
        ("/effort ultracode", Effort::Max),
    ] {
        let effects = enter(&mut s, text);
        assert_eq!(s.effort(), effort, "{text}");
        assert!(!sent_prompt(&effects));
    }
}

#[test]
fn effort_with_an_unknown_level_is_a_prompt() {
    let mut s = session();
    assert!(sent_prompt(&enter(&mut s, "/effort sideways")));
}

#[test]
fn browser_opens_and_closes_the_card() {
    let mut s = session();
    let effects = enter(&mut s, "/browser");
    assert!(s.conversation().browser.is_some());
    assert!(effects.iter().any(|e| matches!(e, Effect::Browser(_))));
    s.apply(Action::FocusBrowser(None), 0.5);
    enter(&mut s, "/browser");
    assert!(s.conversation().browser.is_none());
}

#[test]
fn upload_asks_for_files() {
    let mut s = session();
    let effects = enter(&mut s, "/upload");
    assert!(effects.iter().any(|e| matches!(e, Effect::PickReferences)));
    assert!(!sent_prompt(&effects));
}

#[test]
fn compact_without_a_session_says_there_is_nothing_to_compact() {
    let mut s = session();
    let effects = enter(&mut s, "/compact");
    assert!(!sent_prompt(&effects) && s.conversation().turns.is_empty());
    assert!(s.conversation().composer.is_empty());
    assert_eq!(s.notice, Some(Notice::NothingToCompact));
}

#[test]
fn other_slash_text_is_still_a_prompt() {
    let mut s = session();
    let effects = enter(&mut s, "/memory please explain this");
    assert!(sent_prompt(&effects));
    assert!(!s.nodes.memory);
}

#[test]
fn the_commands_command_toggles_the_commands_node() {
    let mut s = session();
    assert!(!s.nodes.commands);
    enter(&mut s, "/commands");
    assert!(s.nodes.commands);
    enter(&mut s, "/commands");
    assert!(!s.nodes.commands);
}

mod picker;
