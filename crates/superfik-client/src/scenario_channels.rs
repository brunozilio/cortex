//! Channel accounts on the providers' official APIs, as Settings shows
//! them once linked: the account and the webhook to give the provider.

use superfik_core::channel::{ChannelKind, LinkStatus};
use superfik_core::{Accounts, AuthKind, PluginId, PluginSummary};
use superfik_protocol::ServerEvent;

use super::scenario_plugins::{account, plugin, with_plugins};
use super::scenarios::{SyntheticImage, reference};
use crate::state::{Action, Session};

fn linked(
    id: &str,
    name: &str,
    icon: &str,
    kind: ChannelKind,
    about: &str,
    shown_as: &str,
) -> (Session, Vec<SyntheticImage>) {
    let (s, images) = reference();
    let mut s = with_plugins(s);
    let summary = PluginSummary {
        channel: Some(kind),
        description: about.into(),
        category: "productivity".into(),
        accounts: Accounts::Many,
        ..plugin(id, name, icon, AuthKind::ApiKey, true)
    };
    let support = account(id, name, icon, "support");
    let mut list = s.plugins.list.clone();
    list.insert(0, summary);
    let mut connections = s.plugins.connections.clone();
    connections.push(support.clone());
    s.apply(Action::Server(ServerEvent::Plugins { plugins: list, connections }), 0.0);
    let hook = Some(format!("/hooks/channels/{}", support.id));
    let status = LinkStatus::Linked { account: shown_as.into(), hook };
    s.apply(Action::Server(ServerEvent::Channel { connection: support.id, status, drafts: vec![] }), 0.0);
    s.apply(Action::OpenPlugins, 0.0);
    s.apply(Action::SelectPlugin(PluginId(id.into())), 0.0);
    (s, images)
}

pub fn instagram() -> (Session, Vec<SyntheticImage>) {
    let about = "Answer direct messages and comments on a professional Instagram account.";
    linked("instagram", "Instagram", "instagram", ChannelKind::Instagram, about, "@superfik.app")
}

pub fn x() -> (Session, Vec<SyntheticImage>) {
    let about = "Answer mentions and direct messages on an X account.";
    linked("x", "X", "x", ChannelKind::X, about, "@superfikapp")
}

pub fn linkedin() -> (Session, Vec<SyntheticImage>) {
    let about = "Answer comments and publish posts on a LinkedIn company page.";
    linked("linkedin", "LinkedIn", "linkedin", ChannelKind::Linkedin, about, "Superfik")
}

pub fn whatsapp_cloud() -> (Session, Vec<SyntheticImage>) {
    let about = "Answer customers on WhatsApp through Meta's official Cloud API.";
    linked("whatsapp-business", "WhatsApp Business", "whatsapp", ChannelKind::WhatsappCloud, about, "15550783881")
}
