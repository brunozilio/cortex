use superfik_core::{AuthKind, Connection};

use super::tests_plugins::{account, listed, plugin, session, type_text};
use super::*;

fn installed() -> (Session, Connection, Connection) {
    let (work, home) = (account("mail", "Work"), account("mail", "Home"));
    let mut s = session(vec![]);
    let plugins = vec![plugin("mail", AuthKind::OAuth, true), plugin("github", AuthKind::Cli, true)];
    s.apply(listed(plugins, vec![work.clone(), home.clone()]), 0.0);
    s.apply(Action::UseConnection(work.id), 0.0);
    (s, work, home)
}

#[test]
fn the_node_lists_each_installed_plugin_with_the_sessions_account() {
    let (s, work, home) = installed();
    let rows = s.plugin_rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].plugin.as_str(), "mail");
    assert_eq!(rows[0].used, Some(&work));
    assert_eq!(rows[0].accounts, vec![&work, &home]);
    assert!(matches!(rows[0].action(), Action::OpenAccountMenu(p) if p.as_str() == "mail"));
    assert_eq!((rows[1].plugin.as_str(), rows[1].used), ("github", None));
    assert!(matches!(rows[1].action(), Action::ConnectPlugin(p) if p.as_str() == "github"), "no account: connect one");
}

#[test]
fn the_select_switches_the_account_or_takes_the_plugin_out() {
    let (mut s, _, home) = installed();
    let mail = s.plugin_rows()[0].plugin.clone();
    s.apply(Action::OpenAccountMenu(mail.clone()), 0.0);
    assert_eq!(s.popover, Some(PopoverKind::PluginAccounts));
    let choices = s.account_choices();
    assert_eq!(choices.len(), 4, "two accounts, not used, add");
    assert!(matches!(choices[0], AccountChoice::Account { selected: true, .. }));

    let home_line = s.account_choice_action(1).expect("home");
    s.apply(home_line, 0.0);
    assert_eq!(s.conversation().connections, vec![home.clone()], "one account per plugin");
    assert_eq!(s.popover, None);

    s.apply(Action::OpenAccountMenu(mail.clone()), 0.0);
    let not_used = s.account_choice_action(2).expect("not used");
    s.apply(not_used, 0.0);
    assert!(s.session_connections().is_empty());
    assert_eq!(s.plugin_rows()[0].used, None);

    s.apply(Action::OpenAccountMenu(mail), 0.0);
    assert!(matches!(s.account_choice_action(3), Some(Action::ConnectPlugin(p)) if p.as_str() == "mail"));
}

#[test]
fn connecting_from_the_composer_drops_the_half_typed_name() {
    let (mut s, _, _) = installed();
    type_text(&mut s, "check /git");
    s.apply(Action::ConnectPlugin(s.plugin_rows()[1].plugin.clone()), 0.0);
    assert_eq!(s.conversation().composer.text(), "check ");
    let sheet = s.plugins.panel.sheet.as_ref().expect("sheet open");
    assert_eq!(sheet.plugin.as_str(), "github");
}

#[test]
fn the_slash_picker_offers_plugins_without_an_account() {
    let (mut s, _, _) = installed();
    type_text(&mut s, "/");
    assert_eq!(s.popover, Some(PopoverKind::Skills));
    assert_eq!(s.picking(), Some(SKILL));
    let labels: Vec<String> = s.picks().iter().map(Pick::label).collect();
    let at = |label: &str| labels.iter().position(|l| l == label).expect(label);
    assert!(at("/mail-home") < at("/github") && at("/github") < at("/context"), "accounts, plugins, then commands");
    type_text(&mut s, "GIT");
    assert!(matches!(s.picks()[..], [Pick::Plugin(p)] if p.id.as_str() == "github"));
}
