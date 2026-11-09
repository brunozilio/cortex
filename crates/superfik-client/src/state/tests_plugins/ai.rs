//! AI provider plugins: their own Settings pane, connect forms and the
//! model picker they refresh.

use superfik_core::{AuthKind, PluginId, SecretField};
use superfik_protocol::{ClientCommand, ServerEvent};

use super::{account, listed, plugin, session};
use crate::state::tests::commands;
use crate::state::*;

#[test]
fn connecting_and_disconnecting_provider_plugins_refreshes_the_model_picker() {
    let provider = plugin("openai-subscription", AuthKind::OAuth, true);
    let connection = account("openai-subscription", "");
    let mut s = session(vec![]);
    s.plugins.list = vec![provider.clone()];
    let effects = s.apply(
        Action::Server(ServerEvent::Plugins { plugins: vec![provider.clone()], connections: vec![connection] }),
        0.0,
    );
    assert!(commands(&effects).iter().any(|c| matches!(c, ClientCommand::ListModels)));
    let effects = s.apply(Action::Server(ServerEvent::Plugins { plugins: vec![provider], connections: vec![] }), 0.0);
    assert!(commands(&effects).iter().any(|c| matches!(c, ClientCommand::ListModels)));
}

#[test]
fn openai_connection_starts_with_the_official_endpoint_filled_in() {
    let mut provider = plugin("openai", AuthKind::ApiKey, true);
    provider.fields.insert(
        0,
        SecretField {
            key: "base_url".into(),
            label: "API endpoint".into(),
            placeholder: String::new(),
            default_value: Some("https://api.openai.com/v1".into()),
        },
    );
    let mut s = session(vec![]);
    s.plugins.list = vec![provider];
    s.apply(Action::BeginConnect(PluginId("openai".into())), 0.0);
    let sheet = s.plugins.panel.sheet.as_ref().expect("sheet");
    assert_eq!(sheet.fields[0].text(), "https://api.openai.com/v1");
    assert!(sheet.fields[1].is_empty(), "the API key starts empty");
}

#[test]
fn ai_accounts_have_their_own_settings_pane_and_do_not_mix_with_integrations() {
    let mut s = session(vec![]);
    let mut catalog = vec![plugin("mail", AuthKind::OAuth, true)];
    for provider in superfik_core::ModelProvider::ACCOUNTS {
        catalog.push(plugin(
            provider.plugin_id(),
            if provider.subscription() { AuthKind::OAuth } else { AuthKind::ApiKey },
            false,
        ));
    }
    s.apply(listed(catalog, vec![]), 0.0);
    s.apply(Action::OpenSettings, 0.0);
    let effects = s.apply(Action::ShowSettingsPane(SettingsPane::Ai), 0.0);
    assert!(s.plugins_pane_open());
    assert!(commands(&effects).iter().any(|command| matches!(command, ClientCommand::ListPlugins)));
    assert_eq!(
        s.plugins.ai_groups().iter().flat_map(|group| group.plugins.iter().map(|p| p.id.as_str())).collect::<Vec<_>>(),
        ["openai-subscription", "openai", "claude-subscription", "anthropic"]
    );
    assert_eq!(
        s.plugins.shelves(false).into_iter().flat_map(|(_, p)| p).map(|p| p.id.as_str()).collect::<Vec<_>>(),
        ["mail"]
    );
    assert!(SettingsPane::offered(Platform::WEB).any(|pane| pane == SettingsPane::Ai));
}
#[test]
fn the_ai_connect_button_installs_internally_then_opens_the_account_form() {
    let id = PluginId("openai".into());
    let mut s = session(vec![]);
    s.apply(listed(vec![plugin("openai", AuthKind::ApiKey, false)], vec![]), 0.0);
    s.apply(Action::OpenSettings, 0.0);
    s.apply(Action::ShowSettingsPane(SettingsPane::Ai), 0.0);
    let effects = s.apply(Action::ConnectPlugin(id.clone()), 0.0);
    assert!(
        commands(&effects).iter().any(
            |command| matches!(command, ClientCommand::InstallPlugin { plugin: Some(plugin), .. } if *plugin == id)
        )
    );
    s.apply(listed(vec![plugin("openai", AuthKind::ApiKey, true)], vec![]), 0.0);
    let sheet = s.plugins.panel.sheet.as_mut().expect("connect form after install");
    assert_eq!(sheet.plugin, id);
    sheet.label = superfik_ui::TextEdit::new("Minha conta");
    sheet.fields[0] = superfik_ui::TextEdit::new("sk-fixture");
    let effects = s.apply(Action::ConfirmConnect, 0.0);
    assert!(matches!(commands(&effects).as_slice(),[ClientCommand::Connect{plugin,..}] if *plugin==id));
}
#[test]
fn ai_subscriptions_use_browser_authorization_and_refresh_models_when_connected() {
    let mut s = session(vec![]);
    let id = PluginId("claude-subscription".into());
    s.apply(listed(vec![plugin(id.as_str(), AuthKind::OAuth, true)], vec![]), 0.0);
    s.apply(Action::ConnectPlugin(id.clone()), 0.0);
    assert_eq!(s.settings.pane, SettingsPane::Ai);
    assert!(s.plugins.panel.sheet.as_ref().expect("OAuth form").fields.is_empty());
    let effects = s.apply(Action::ConfirmConnect, 0.0);
    assert!(
        matches!(commands(&effects).as_slice(),[ClientCommand::Connect{plugin,values,..}] if *plugin==id && values.is_empty())
    );
    let effects = s.apply(
        Action::Server(ServerEvent::AuthorizeAt {
            plugin: id.clone(),
            url: "https://claude.ai/oauth/authorize".into(),
        }),
        0.0,
    );
    assert!(
        effects.iter().any(|effect| matches!(effect,Effect::OpenUrl(url) if url=="https://claude.ai/oauth/authorize"))
    );
    let effects = s.apply(
        listed(vec![plugin(id.as_str(), AuthKind::OAuth, true)], vec![account(id.as_str(), "Minha conta")]),
        0.0,
    );
    assert!(commands(&effects).iter().any(|command| matches!(command, ClientCommand::ListModels)));
}

#[test]
fn switching_ai_connection_method_uses_the_right_provider_without_mixing_credentials() {
    let mut s = session(vec![]);
    let providers = vec![
        plugin("openai-subscription", AuthKind::OAuth, true),
        plugin("openai", AuthKind::ApiKey, true),
        plugin("claude-subscription", AuthKind::OAuth, true),
        plugin("anthropic", AuthKind::ApiKey, true),
    ];
    let connection = account("openai-subscription", "Conta pessoal");
    let connection_id = connection.id;
    s.apply(listed(providers, vec![connection]), 0.0);
    assert_eq!(s.plugins.ai_groups().iter().map(|group| group.name).collect::<Vec<_>>(), ["ChatGPT", "Claude"]);
    for (subscription, api) in [("openai-subscription", "openai"), ("claude-subscription", "anthropic")] {
        s.apply(Action::ConnectPlugin(PluginId(api.into())), 0.0);
        let sheet = s.plugins.panel.sheet.as_mut().expect("API form");
        assert_eq!(sheet.plugin.as_str(), api);
        sheet.fields[0] = superfik_ui::TextEdit::new("sk-test-not-a-real-key");
        s.apply(Action::ConnectPlugin(PluginId(subscription.into())), 0.0);
        let sheet = s.plugins.panel.sheet.as_ref().expect("subscription form");
        assert!(sheet.fields.is_empty(), "API secrets do not carry over to OAuth");
        let effects = s.apply(Action::ConfirmConnect, 0.0);
        assert!(
            matches!(commands(&effects).as_slice(), [ClientCommand::Connect {plugin, values, ..}] if plugin.as_str()==subscription && values.is_empty())
        );
    }
    assert_eq!(s.plugins.connections[0].id, connection_id, "switching methods keeps existing accounts");
}
