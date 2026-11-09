use std::path::PathBuf;

use superfik_core::{
    Accounts, AuthKind, Connection, ConnectionId, PluginId, PluginOrigin, PluginSummary, SecretField, Skill, SkillIcon,
};
use superfik_protocol::{ClientCommand, ServerEvent};

use super::tests::commands;
use super::*;

pub(super) fn plugin(id: &str, auth: AuthKind, installed: bool) -> PluginSummary {
    PluginSummary {
        id: PluginId(id.into()),
        name: id.to_uppercase(),
        version: "1".into(),
        description: String::new(),
        icon: String::new(),
        category: "productivity".into(),
        accounts: Accounts::Many,
        auth,
        fields: match auth {
            AuthKind::ApiKey => {
                vec![SecretField {
                    key: "api_key".into(),
                    label: "Key".into(),
                    placeholder: String::new(),
                    default_value: None,
                }]
            }
            _ => vec![],
        },
        origin: PluginOrigin::Catalog,
        installed,
        skills: vec![],
        servers: vec![id.into()],
        vault: None,
        channel: None,
    }
}

pub(super) fn account(plugin: &str, label: &str) -> Connection {
    Connection {
        id: ConnectionId::new(),
        plugin: PluginId(plugin.into()),
        plugin_name: plugin.to_uppercase(),
        label: label.into(),
        icon: String::new(),
    }
}

pub(super) fn listed(plugins: Vec<PluginSummary>, connections: Vec<Connection>) -> Action {
    Action::Server(ServerEvent::Plugins { plugins, connections })
}

pub(super) fn session(connections: Vec<Connection>) -> Session {
    let mut s = Session::new(Some(PathBuf::from("/repo")));
    s.available_skills = vec![Skill {
        name: "motion".into(),
        title: "Motion".into(),
        description: String::new(),
        icon: SkillIcon::Cube,
    }];
    s.apply(listed(vec![plugin("mail", AuthKind::OAuth, true)], connections), 0.0);
    s
}

pub(super) fn type_text(s: &mut Session, text: &str) {
    s.apply(Action::Edit(EditKey::Insert(text.into())), 0.0);
}

#[test]
fn accounts_are_picked_with_a_slash_like_skills() {
    let (work, home) = (account("mail", "Work"), account("mail", "Home"));
    let mut s = session(vec![work.clone(), home.clone()]);
    type_text(&mut s, "check inbox /mail");
    assert_eq!(s.popover, Some(PopoverKind::Skills));
    let labels: Vec<String> = s.picks().iter().map(Pick::label).collect();
    assert_eq!(labels, vec!["/mail-work", "/mail-home"]);

    s.apply(s.picks()[0].action(), 0.0);
    assert_eq!(s.conversation().composer.text(), "check inbox /mail-work ");
    assert_eq!(s.popover, None);
    assert_eq!(s.session_connections(), vec![&work]);
    let tokens = s.composer_tokens();
    assert_eq!((tokens[0].start, tokens[0].end), (12, 22), "highlighted as typed");
    assert!(matches!(tokens[0].pick, Pick::Connection(c) if c.id == work.id));

    type_text(&mut s, "/");
    let labels: Vec<String> = s.picks().iter().map(Pick::label).collect();
    assert!(labels.contains(&"/mail-home".to_owned()) && labels.contains(&"/motion".to_owned()));
    assert!(!labels.contains(&"/mail-work".to_owned()));
}

#[test]
fn the_session_sends_and_keeps_its_accounts() {
    let work = account("mail", "Work");
    let mut s = session(vec![work.clone()]);
    s.apply(Action::PickConnection(work.id), 0.0);
    type_text(&mut s, "summarise my mail");
    let effects = s.apply(Action::Submit, 1.0);
    let request = commands(&effects)
        .into_iter()
        .find_map(|c| match c {
            ClientCommand::SubmitPrompt { request, .. } => Some(request.clone()),
            _ => None,
        })
        .expect("submitted");
    assert_eq!(request.connections, vec![work.clone()]);
    let kept = effects.iter().find_map(|e| match e {
        Effect::KeepSession { session, .. } => Some(session.connections.clone()),
        _ => None,
    });
    assert_eq!(kept, Some(vec![work.clone()]));
    assert_eq!(s.conversation().connections, vec![work.clone()]);
    s.apply(Action::RemoveConnection(work.id), 2.0);
    assert!(s.conversation().connections.is_empty());
}

#[test]
fn disconnected_accounts_leave_the_session() {
    let work = account("mail", "Work");
    let mut s = session(vec![work.clone()]);
    s.apply(Action::PickConnection(work.id), 0.0);
    s.apply(listed(vec![plugin("mail", AuthKind::OAuth, true)], vec![]), 0.0);
    assert!(s.session_connections().is_empty(), "no longer connected");
    let effects = s.apply(Action::Disconnect(work.id), 0.0);
    assert!(s.conversation().connections.is_empty());
    assert!(matches!(commands(&effects)[..], [ClientCommand::Disconnect { connection }] if *connection == work.id));
}

#[test]
fn installing_asks_again_for_skills_and_agents() {
    let mut s = session(vec![]);
    let effects = s.apply(Action::OpenPlugins, 0.0);
    assert!(matches!(commands(&effects)[..], [ClientCommand::ListPlugins, ClientCommand::ListChannels]));
    let effects = s.apply(Action::InstallPlugin(PluginId("docs".into())), 0.0);
    assert!(matches!(commands(&effects)[..], [ClientCommand::InstallPlugin { plugin: Some(_), folder: None }]));
    assert_eq!(s.plugins.panel.pending, Some(PluginId("docs".into())));
    let plugins = vec![plugin("mail", AuthKind::OAuth, true), plugin("docs", AuthKind::ApiKey, true)];
    let effects = s.apply(listed(plugins, vec![]), 0.0);
    let names: Vec<&str> = commands(&effects).iter().map(|c| c.name()).collect();
    assert_eq!(names, vec!["list_skills", "list_agents"]);
    assert_eq!(s.plugins.panel.pending, None);
}

#[test]
fn back_closes_the_sheet_then_the_card_then_settings() {
    let mut s = session(vec![]);
    s.apply(Action::OpenPlugins, 0.0);
    s.apply(Action::BeginConnect(PluginId("mail".into())), 0.0);
    s.apply(Action::PluginsBack, 0.0);
    assert!(s.plugins.panel.sheet.is_none() && s.plugins.panel.selected.is_some());
    s.apply(Action::PluginsBack, 0.0);
    assert!(s.plugins.panel.selected.is_none(), "every card folded");
    assert!(s.settings.open);
    s.apply(Action::PluginsBack, 0.0);
    assert!(!s.settings.open);
}

#[test]
fn the_plug_opens_integrations_and_asks_for_plugins() {
    let mut s = session(vec![]);
    let effects = s.apply(Action::OpenPlugins, 0.0);
    assert!(s.settings.open && s.settings.pane == SettingsPane::Integrations);
    assert!(matches!(commands(&effects)[..], [ClientCommand::ListPlugins, ClientCommand::ListChannels]));
    assert!(s.plugins_pane_open());
    s.apply(Action::ShowSettingsPane(SettingsPane::Sound), 0.0);
    assert!(!s.plugins_pane_open());
}

#[test]
fn vaults_list_apart_from_integrations() {
    let vault = PluginSummary {
        vault: Some(superfik_core::VaultKind::Bitwarden),
        ..plugin("bitwarden", AuthKind::ApiKey, false)
    };
    let mut s = session(vec![]);
    s.apply(listed(vec![plugin("mail", AuthKind::OAuth, true), vault], vec![]), 0.0);
    let ids = |vaults| -> Vec<&str> {
        s.plugins.shelves(vaults).into_iter().flat_map(|(_, list)| list).map(|p| p.id.as_str()).collect()
    };
    assert_eq!(ids(false), vec!["mail"]);
    assert_eq!(ids(true), vec!["bitwarden"]);
}

#[test]
fn connecting_a_vault_opens_the_vaults_pane() {
    let vault = PluginSummary {
        vault: Some(superfik_core::VaultKind::Bitwarden),
        ..plugin("bitwarden", AuthKind::ApiKey, true)
    };
    let mut s = session(vec![]);
    s.apply(listed(vec![vault], vec![]), 0.0);
    s.apply(Action::ConnectPlugin(PluginId("bitwarden".into())), 0.0);
    assert_eq!(s.settings.pane, SettingsPane::Vaults);
    assert!(s.plugins.panel.sheet.is_some());
}

#[test]
fn closing_settings_drops_the_open_sheet() {
    let mut s = session(vec![]);
    s.apply(Action::OpenPlugins, 0.0);
    s.apply(Action::BeginConnect(PluginId("mail".into())), 0.0);
    s.apply(Action::CloseSettings, 0.0);
    assert!(s.plugins.panel.sheet.is_none());
}

#[test]
fn sign_ins_open_the_browser_and_failures_show() {
    let mut s = session(vec![]);
    let effects = s.apply(
        Action::Server(ServerEvent::AuthorizeAt { plugin: PluginId("mail".into()), url: "https://a".into() }),
        0.0,
    );
    assert!(effects.iter().any(|e| matches!(e, Effect::OpenUrl(url) if url == "https://a")));
    s.apply(Action::Server(ServerEvent::PluginFailed { plugin: None, message: "denied".into() }), 0.0);
    assert_eq!(s.notice, Some(Notice::Raw("denied".into())));
}

#[test]
fn typed_tokens_attach_on_send_and_removing_strips_them() {
    let work = account("mail", "Work");
    let mut s = session(vec![work.clone()]);
    type_text(&mut s, "/motion read /mail-work today");
    let tokens = s.composer_tokens();
    assert_eq!(tokens.len(), 2);
    assert!(matches!(tokens[0].pick, Pick::Skill(skill) if skill.name == "motion"));
    s.apply(Action::RemoveConnection(work.id), 0.0);
    assert_eq!(s.conversation().composer.text(), "/motion read today");
    type_text(&mut s, " /mail-work");
    s.apply(Action::Submit, 1.0);
    assert_eq!(s.conversation().connections, vec![work]);
}

#[test]
fn an_account_is_named_by_its_plugin_in_any_case() {
    let github = Connection { plugin_name: "GitHub".into(), ..account("github", "bruno") };
    let (work, home) = (account("mail", "Work"), account("mail", "Home"));
    let mut s = session(vec![github.clone(), work, home]);
    type_text(&mut s, "open /GitHub and /Motion not /mail");
    let tokens = s.composer_tokens();
    assert_eq!(tokens.len(), 2, "a plugin with two accounts needs the account's name");
    assert!(matches!(tokens[0].pick, Pick::Connection(c) if c.id == github.id));
    assert_eq!(&s.conversation().composer.text()[tokens[0].start..tokens[0].end], "/GitHub", "kept as typed");
    assert!(matches!(tokens[1].pick, Pick::Skill(skill) if skill.name == "motion"));
    assert_eq!(s.session_connections(), vec![&github]);
    s.apply(Action::RemoveConnection(github.id), 0.0);
    assert_eq!(s.conversation().composer.text(), "open and /Motion not /mail");
}

#[test]
fn a_hash_without_a_matching_account_stays_plain_text() {
    let mut s = session(vec![account("mail", "Work")]);
    type_text(&mut s, "fix #12");
    assert_eq!(s.popover, None);
    assert!(s.composer_tokens().is_empty());
}

#[test]
fn a_vault_stays_installed() {
    let vault = PluginSummary {
        vault: Some(superfik_core::VaultKind::Bitwarden),
        ..plugin("bitwarden", AuthKind::ApiKey, true)
    };
    let mut s = session(vec![]);
    s.apply(listed(vec![plugin("mail", AuthKind::OAuth, true), vault], vec![]), 0.0);
    let effects = s.apply(Action::UninstallPlugin(PluginId("bitwarden".into())), 0.0);
    assert!(commands(&effects).is_empty(), "no uninstall is sent for a vault");
    let effects = s.apply(Action::UninstallPlugin(PluginId("mail".into())), 0.0);
    assert!(matches!(commands(&effects)[..], [ClientCommand::UninstallPlugin { .. }]));
}

mod ai;
