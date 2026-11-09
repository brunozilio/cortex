//! The "add account" sheet from the keyboard: typing, Tab and Enter.

use superfik_core::{AuthKind, PluginId};
use superfik_protocol::ClientCommand;

use super::tests::commands;
use super::tests_plugins::{listed, plugin, session, type_text};
use super::*;

fn sheet_open() -> Session {
    let mut s = session(vec![]);
    s.apply(listed(vec![plugin("docs", AuthKind::ApiKey, true)], vec![]), 0.0);
    s.apply(Action::OpenPlugins, 0.0);
    s.apply(Action::BeginConnect(PluginId("docs".into())), 0.0);
    s
}

#[test]
fn the_sheet_collects_the_name_and_fields() {
    let mut s = sheet_open();
    type_text(&mut s, "personal");
    assert!(s.conversation().composer.text().is_empty(), "keys go to the sheet");
    s.apply(Action::Submit, 0.0);
    type_text(&mut s, "k-123");
    let effects = s.apply(Action::Submit, 0.0);
    match commands(&effects)[..] {
        [ClientCommand::Connect { plugin, label, values }] => {
            assert_eq!(plugin.as_str(), "docs");
            assert_eq!(label, "personal");
            assert_eq!(values, &vec![("api_key".to_owned(), "k-123".to_owned())]);
        }
        ref other => panic!("unexpected {other:?}"),
    }
    assert!(s.plugins.panel.sheet.is_none());
}

#[test]
fn tab_walks_the_sheet_fields_then_its_buttons_and_wraps() {
    let mut s = sheet_open();
    let focus = |s: &Session| s.plugins.panel.sheet.as_ref().map(|sheet| sheet.focus);
    let mut seen = Vec::new();
    for _ in 0..4 {
        s.apply(Action::StepConnectFocus { back: false }, 0.0);
        seen.push(focus(&s));
    }
    assert_eq!(seen, [Some(1), Some(2), Some(3), Some(0)]);
    s.apply(Action::StepConnectFocus { back: true }, 0.0);
    assert_eq!(focus(&s), Some(3), "shift-tab wraps back to Cancel");
    type_text(&mut s, "x");
    assert!(s.plugins.panel.sheet.as_ref().is_some_and(|sheet| sheet.label.text().is_empty()), "buttons take no text");
}

#[test]
fn enter_presses_the_focused_sheet_button() {
    let mut s = sheet_open();
    s.apply(Action::StepConnectFocus { back: true }, 0.0);
    assert!(commands(&s.apply(Action::Submit, 0.0)).is_empty(), "Cancel connects nothing");
    assert!(s.plugins.panel.sheet.is_none() && s.plugins.panel.selected.is_some(), "the card stays open");

    s.apply(Action::BeginConnect(PluginId("docs".into())), 0.0);
    s.apply(Action::StepConnectFocus { back: true }, 0.0);
    s.apply(Action::StepConnectFocus { back: true }, 0.0);
    let effects = s.apply(Action::Submit, 0.0);
    assert!(matches!(commands(&effects)[..], [ClientCommand::Connect { .. }]), "Connect connects");
}
