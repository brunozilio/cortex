//! The `/` picker while a word is typed: an empty picker that stays, and
//! Tab completing the word shown in grey after the caret.

use std::path::PathBuf;

use superfik_protocol::ClientCommand;

use super::tests::commands;
use super::*;

fn session() -> Session {
    Session::new(Some(PathBuf::from("/repo")))
}

fn skill(name: &str) -> superfik_core::Skill {
    superfik_core::Skill {
        name: name.into(),
        title: String::new(),
        description: String::new(),
        icon: superfik_core::SkillIcon::Cube,
    }
}

fn labels(s: &Session) -> Vec<String> {
    s.picks().iter().map(Pick::label).collect()
}

fn sent_prompt(effects: &[Effect]) -> bool {
    commands(effects).iter().any(|c| matches!(c, ClientCommand::SubmitPrompt { .. }))
}

#[test]
fn a_slash_that_matches_nothing_keeps_the_picker_open_and_empty() {
    let mut s = session();
    s.apply(Action::Edit(EditKey::Insert("/zzzz".into())), 0.0);
    assert_eq!(s.popover, Some(PopoverKind::Skills), "the nodes stay replaced by the picker");
    assert_eq!(s.picking(), Some('/'));
    assert!(labels(&s).is_empty());
    for _ in 0..4 {
        s.apply(Action::Edit(EditKey::Backspace), 0.0);
    }
    assert_eq!(s.popover, Some(PopoverKind::Skills), "a bare / is still the picker");
    assert_eq!(labels(&s).len(), Command::ALL.len());
}

#[test]
fn tab_completes_the_slash_word_the_picker_starts_with_the_rest_shown_after_the_caret() {
    let mut s = session();
    s.available_skills = vec![skill("test")];
    s.apply(Action::Edit(EditKey::Insert("/te".into())), 0.0);
    assert_eq!(s.completion().as_deref(), Some("st"), "the picker's first row starting with /te");
    let effects = s.apply(Action::AcceptCompletion, 1.0);
    assert!(!sent_prompt(&effects), "completing sends nothing");
    assert_eq!(s.conversation().composer.text(), "/test ", "completed like a pick, with a space");
    assert_eq!(s.completion(), None);
    assert_eq!(s.popover, None);
}

#[test]
fn no_completion_without_a_prefix_match_a_bare_slash_or_a_caret_before_the_end() {
    let mut s = session();
    s.available_skills = vec![skill("test")];
    s.apply(Action::Edit(EditKey::Insert("/".into())), 0.0);
    assert_eq!(s.completion(), None, "a bare / completes nothing");
    s.apply(Action::Edit(EditKey::Insert("zz".into())), 0.0);
    assert_eq!(s.completion(), None, "nothing starts with /zz");
    for _ in 0..3 {
        s.apply(Action::Edit(EditKey::Backspace), 0.0);
    }
    s.apply(Action::Edit(EditKey::Insert("/te".into())), 0.0);
    assert_eq!(s.completion().as_deref(), Some("st"));
    s.apply(Action::Edit(EditKey::Left { select: false }), 0.0);
    assert_eq!(s.completion(), None, "the caret is no longer at the end");
}
