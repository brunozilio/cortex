use domain::account_pool::Balance;
use domain::provider::{ModelProvider, ProviderSummary};

use super::app::{App, Msg};
use super::picker::{Picker, PickerKind};

#[derive(Debug, Clone)]
pub struct Listed {
    pub provider: ModelProvider,
    pub number: usize,
    pub label: String,
}

pub(super) fn open(app: &mut App, removing: bool) {
    app.spawn(move |client| match client.providers() {
        Ok(providers) => Msg::Accounts { providers, removing },
        Err(error) => Msg::Error(error.to_string()),
    });
}

pub(super) fn listed(app: &mut App, providers: Vec<ProviderSummary>, removing: bool) {
    let mut listed = Vec::new();
    let mut items = Vec::new();
    for summary in &providers {
        let active = summary.account.as_ref().map(|account| account.id.as_str());
        for (at, account) in summary.accounts.iter().enumerate() {
            let state = if Some(account.id.as_str()) == active { "in use" } else { "" };
            items.push((format!("{} · {}", summary.name, account.label), state.to_owned()));
            listed.push(Listed { provider: summary.provider, number: at + 1, label: account.label.clone() });
        }
    }
    if listed.is_empty() {
        return app.transcript.notice("No account is connected: /login connects one.");
    }
    let cursor = if removing { 0 } else { items.iter().position(|(_, state)| state.is_empty()).unwrap_or(0) };
    let title = if removing { "Forget which account?" } else { "Use which account?" };
    app.catalog.accounts = listed;
    app.overlays.picker = Some(Picker { kind: PickerKind::Account { removing }, title: title.into(), items, cursor });
}

pub(super) fn balance(app: &mut App, chosen: Option<Balance>) {
    let Some(provider) = app.model().map(|model| model.provider) else {
        return app.transcript.notice("No model yet: /login connects a provider.");
    };
    let Some(balance) = chosen else {
        let ways: Vec<String> = Balance::ALL.iter().map(|b| format!("{}: {}", b.name(), b.about())).collect();
        return app.transcript.notice(format!(
            "/accounts balance <way> sets how this provider's accounts share the work:\n{}",
            ways.join("\n")
        ));
    };
    app.spawn(move |client| match client.set_balance(provider, balance) {
        Ok(_) => Msg::Notice(format!("Accounts balance: {} ({}).", balance.name(), balance.about())),
        Err(error) => Msg::Error(error.to_string()),
    });
}

pub(super) fn picked(app: &mut App, cursor: usize, removing: bool) {
    let Some(Listed { provider, number, label }) = app.catalog.accounts.get(cursor).cloned() else { return };
    app.spawn(move |client| {
        let done = if removing {
            client.remove_account(provider, &number.to_string()).map(|_| format!("Forgot {label}."))
        } else {
            client.use_account(provider, &number.to_string()).map(|_| format!("Using {label} from the next message."))
        };
        done.map_or_else(|error| Msg::Error(error.to_string()), Msg::Notice)
    });
}

pub(super) fn logout(app: &mut App) {
    app.spawn(|client| match client.providers() {
        Ok(providers) => Msg::Providers(providers.into_iter().filter(|p| p.account.is_some()).collect()),
        Err(error) => Msg::Error(error.to_string()),
    });
}

pub(super) fn connected(app: &mut App, providers: Vec<ProviderSummary>) {
    if providers.is_empty() {
        return app.transcript.notice("No provider is connected.");
    }
    let items = providers.iter().map(|p| (p.name.clone(), String::new())).collect();
    app.catalog.connected = providers;
    app.overlays.picker =
        Some(Picker { kind: PickerKind::Logout, title: "Forget which provider?".into(), items, cursor: 0 });
}

pub(super) fn logout_picked(app: &mut App, cursor: usize) {
    let Some(summary) = app.catalog.connected.get(cursor).cloned() else { return };
    app.spawn(move |client| match client.disconnect_provider(summary.provider) {
        Ok(_) => Msg::Notice(format!("Forgot {}.", summary.name)),
        Err(error) => Msg::Error(error.to_string()),
    });
}
