//! Scenarios for plugins: the Plugins screen, its account sheet, and a
//! session working with connected accounts.

use superfik_core::{Accounts, AuthKind, Connection, ConnectionId, PluginId, PluginOrigin, PluginSummary, SecretField};
use superfik_protocol::ServerEvent;
use superfik_ui::TextEdit;

use super::scenarios::{SyntheticImage, reference};
use crate::state::{Action, ConnectSheet, PopoverKind, Session};

pub(crate) fn plugin(id: &str, name: &str, icon: &str, auth: AuthKind, installed: bool) -> PluginSummary {
    let (category, description) = match id {
        "github" => ("development", "Repos, pull requests, issues and reviews."),
        "sentry" => ("development", "Investigate errors and releases."),
        "context7" => ("development", "Current docs for any library."),
        "linear" => ("productivity", "Track and update issues and projects."),
        "notion" => ("productivity", "Search and edit pages and databases."),
        "stripe" => ("data", "Look up customers, payments and plans."),
        "postgres" => ("data", "Query your database, read-only."),
        "firecrawl" => ("web", "Turn any website into clean markdown."),
        _ => ("web", "Let agents drive a real browser."),
    };
    PluginSummary {
        id: PluginId(id.into()),
        name: name.into(),
        version: "1.0.0".into(),
        description: description.into(),
        icon: icon.into(),
        category: category.into(),
        accounts: if auth == AuthKind::OAuth { Accounts::Many } else { Accounts::One },
        auth,
        fields: match auth {
            AuthKind::ApiKey => {
                vec![SecretField {
                    key: "api_key".into(),
                    label: "API key".into(),
                    placeholder: "sk-…".into(),
                    default_value: None,
                }]
            }
            _ => Vec::new(),
        },
        origin: PluginOrigin::Catalog,
        installed,
        skills: if id == "linear" { vec!["linear-workflow".into()] } else { Vec::new() },
        servers: vec![id.into()],
        vault: None,
        channel: None,
    }
}

pub(crate) fn account(plugin: &str, name: &str, icon: &str, label: &str) -> Connection {
    Connection {
        id: ConnectionId::new(),
        plugin: PluginId(plugin.into()),
        plugin_name: name.into(),
        label: label.into(),
        icon: icon.into(),
    }
}

fn catalog() -> Vec<PluginSummary> {
    vec![
        plugin("github", "GitHub", "github", AuthKind::Cli, true),
        plugin("linear", "Linear", "linear", AuthKind::OAuth, true),
        plugin("notion", "Notion", "notion", AuthKind::OAuth, false),
        plugin("sentry", "Sentry", "sentry", AuthKind::OAuth, false),
        plugin("stripe", "Stripe", "stripe", AuthKind::ApiKey, false),
        plugin("postgres", "PostgreSQL", "postgresql", AuthKind::ApiKey, false),
        plugin("context7", "Context7", "context7", AuthKind::ApiKey, true),
        plugin("firecrawl", "Firecrawl", "firecrawl", AuthKind::ApiKey, false),
        plugin("playwright", "Playwright", "playwright", AuthKind::None, true),
    ]
}

fn accounts() -> Vec<Connection> {
    vec![
        account("github", "GitHub", "github", ""),
        account("linear", "Linear", "linear", "Acme"),
        account("linear", "Linear", "linear", "Side project"),
        account("playwright", "Playwright", "playwright", ""),
    ]
}

pub(crate) fn with_plugins(mut s: Session) -> Session {
    s.apply(Action::Server(ServerEvent::Plugins { plugins: catalog(), connections: accounts() }), 0.0);
    s
}

pub fn vaults() -> (Session, Vec<SyntheticImage>) {
    let (s, images) = reference();
    let mut s = with_plugins(s);
    let bitwarden = PluginSummary {
        vault: Some(superfik_core::VaultKind::Bitwarden),
        description: "Let agents use your passwords without seeing them.".into(),
        category: "development".into(),
        ..plugin("bitwarden", "Bitwarden", "bitwarden", AuthKind::ApiKey, false)
    };
    let mut list: Vec<PluginSummary> = s.plugins.list.clone();
    list.push(bitwarden);
    s.apply(Action::Server(ServerEvent::Plugins { plugins: list, connections: accounts() }), 0.0);
    s.apply(Action::OpenPlugins, 0.0);
    s.apply(Action::ShowSettingsPane(crate::state::SettingsPane::Vaults), 0.0);
    (s, images)
}

pub fn plugins() -> (Session, Vec<SyntheticImage>) {
    let (s, images) = reference();
    let mut s = with_plugins(s);
    s.apply(Action::OpenPlugins, 0.0);
    (s, images)
}

pub fn plugin_page() -> (Session, Vec<SyntheticImage>) {
    let (s, images) = plugins();
    let mut s = s;
    s.apply(Action::SelectPlugin(PluginId("linear".into())), 0.0);
    (s, images)
}

pub fn plugins_connect() -> (Session, Vec<SyntheticImage>) {
    let (s, images) = reference();
    let mut s = with_plugins(s);
    s.apply(Action::OpenPlugins, 0.0);
    s.apply(Action::SelectPlugin(PluginId("context7".into())), 0.0);
    s.plugins.panel.sheet = Some(ConnectSheet {
        plugin: PluginId("context7".into()),
        label: TextEdit::new("personal"),
        fields: vec![TextEdit::default()],
        focus: 1,
    });
    (s, images)
}

pub fn connections() -> (Session, Vec<SyntheticImage>) {
    let (s, images) = reference();
    let mut s = with_plugins(s);
    let picked: Vec<Connection> = s.plugins.connections.iter().take(2).cloned().collect();
    s.conversation_mut().connections = picked;
    s.conversation_mut().composer =
        TextEdit::new("Triage the new bugs with /motion and file them in /linear-side-project see /GitHub then /");
    s.popover = Some(PopoverKind::Skills);
    (s, images)
}

pub fn plugins_node() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = connections();
    s.conversation_mut().composer = TextEdit::new("Triage the new bugs with /motion and #GitHub");
    s.popover = None;
    (s, images)
}

pub fn plugin_accounts() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = plugins_node();
    s.apply(Action::OpenAccountMenu(PluginId("linear".into())), 0.0);
    (s, images)
}

pub fn plugins_many() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = plugins_node();
    let plugins = catalog().into_iter().map(|p| PluginSummary { installed: true, ..p }).collect();
    s.apply(Action::Server(ServerEvent::Plugins { plugins, connections: accounts() }), 0.0);
    s.conversation_mut().pages.insert(crate::state::Carousel::Plugins, 1);
    (s, images)
}

pub fn whatsapp() -> (Session, Vec<SyntheticImage>) {
    let (s, images) = reference();
    let mut s = with_plugins(s);
    let whatsapp = PluginSummary {
        channel: Some(superfik_core::channel::ChannelKind::WhatsappWeb),
        description: "Answer customers on a WhatsApp number linked by QR code.".into(),
        category: "productivity".into(),
        accounts: Accounts::Many,
        ..plugin("whatsapp", "WhatsApp", "whatsapp", AuthKind::Qr, true)
    };
    let (support, sales) =
        (account("whatsapp", "WhatsApp", "whatsapp", "support"), account("whatsapp", "WhatsApp", "whatsapp", "sales"));
    let mut list = s.plugins.list.clone();
    list.insert(0, whatsapp);
    let mut connections = accounts();
    connections.extend([support.clone(), sales.clone()]);
    s.apply(Action::Server(ServerEvent::Plugins { plugins: list, connections }), 0.0);
    let linked = superfik_core::channel::LinkStatus::Linked { account: "5511999990000".into(), hook: None };
    s.apply(Action::Server(ServerEvent::Channel { connection: support.id, status: linked, drafts: vec![] }), 0.0);
    let qr =
        "2@Q1x9v8ZyKpWm3t,8Hk2Lr6Uo4Pq9Xs1Df7Gh5Jk3Lz0=,Yv3Bn8Mm4Qp6Ws2Ex9Rt7Uy5Io1Pa=,Vb2Nc7Xz9Lk4Jh6Gf8Ds1Aq3We5Rt=";
    let pairing = superfik_core::channel::LinkStatus::Pairing { qr: qr.into() };
    s.apply(Action::Server(ServerEvent::Channel { connection: sales.id, status: pairing, drafts: vec![] }), 0.0);
    s.apply(Action::OpenPlugins, 0.0);
    s.apply(Action::SelectPlugin(PluginId("whatsapp".into())), 0.0);
    (s, images)
}

pub(crate) fn ai(connect: bool) -> Session {
    let mut s = Session::default();
    s.apply(Action::OpenSettings, 0.0);
    s.apply(Action::ShowSettingsPane(crate::state::SettingsPane::Ai), 0.0);
    let mut providers = vec![
        plugin("openai-subscription", "ChatGPT Subscription", "openai", AuthKind::OAuth, true),
        plugin("claude-subscription", "Claude Subscription", "anthropic", AuthKind::OAuth, true),
        plugin("openai", "OpenAI API", "openai", AuthKind::ApiKey, true),
        plugin("anthropic", "Anthropic API", "anthropic", AuthKind::ApiKey, true),
    ];
    for provider in &mut providers {
        provider.accounts = Accounts::Many;
        provider.description = match provider.id.as_str() {
            "openai-subscription" => "Entre com sua assinatura do ChatGPT.",
            "claude-subscription" => "Entre com sua assinatura do Claude.",
            "openai" => "Modelos da OpenAI usando sua chave de API.",
            _ => "Modelos Claude usando sua chave de API.",
        }
        .into();
    }
    if let Some(openai) = providers.iter_mut().find(|provider| provider.id.as_str() == "openai") {
        openai.fields.insert(
            0,
            SecretField {
                key: "base_url".into(),
                label: "API endpoint".into(),
                placeholder: "https://api.openai.com/v1".into(),
                default_value: Some("https://api.openai.com/v1".into()),
            },
        );
    }
    s.apply(
        Action::Server(ServerEvent::Plugins {
            plugins: providers,
            connections: vec![
                account("openai-subscription", "ChatGPT Subscription", "openai", "Minha conta ChatGPT"),
                account("openai-subscription", "ChatGPT Subscription", "openai", "Conta de trabalho"),
                account("openai", "OpenAI API", "openai", "Minha chave de API"),
            ],
        }),
        0.0,
    );
    if connect {
        s.apply(Action::ConnectPlugin(PluginId("openai".into())), 0.0);
    } else {
        s.plugins.panel.selected = Some(PluginId("openai-subscription".into()));
    }
    s
}

pub(crate) fn ai_local() -> Session {
    let mut s = ai(false);
    s.apply(Action::ToggleLocalModels, 0.0);
    s.models.push(
        superfik_core::ModelOption::new(
            superfik_core::EngineKind::Native,
            "mlx-community/Qwen3-8B-4bit",
            "Qwen3 8B 4-bit",
        )
        .served_by(superfik_core::ModelProvider::Local),
    );
    let fetching = superfik_protocol::DownloadState::Fetching { done: 1_960_000_000, total: Some(4_623_784_971) };
    s.plugins.downloads.insert("mlx-community/gemma-3-4b-it-4bit".into(), fetching);
    s
}
