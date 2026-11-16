use domain::account_pool::Balance;
use domain::provider::{ModelProvider, ProviderSummary};

use super::app::{App, Msg};
use super::choices::{Picker, PickerKind};

#[derive(Debug, Clone)]
pub struct Listed {
    pub provider: ModelProvider,
    pub number: usize,
    pub label: String,
}

impl App {
    pub(super) fn accounts(&mut self, removing: bool) {
        self.spawn(move |sdk| match sdk.providers() {
            Ok(providers) => Msg::Accounts { providers, removing },
            Err(error) => Msg::Error(error.to_string()),
        });
    }

    pub(super) fn accounts_listed(&mut self, providers: Vec<ProviderSummary>, removing: bool) {
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
            return self.transcript.notice("No account is connected: /login connects one.");
        }
        let cursor = if removing { 0 } else { items.iter().position(|(_, state)| state.is_empty()).unwrap_or(0) };
        let title = if removing { "Forget which account?" } else { "Use which account?" };
        self.catalog.accounts = listed;
        self.overlays.picker =
            Some(Picker { kind: PickerKind::Account { removing }, title: title.into(), items, cursor });
    }

    pub(super) fn balance(&mut self, chosen: Option<Balance>) {
        let Some(provider) = self.model().map(|model| model.provider) else {
            return self.transcript.notice("No model yet: /login connects a provider.");
        };
        let Some(balance) = chosen else {
            let ways: Vec<String> = Balance::ALL.iter().map(|b| format!("{}: {}", b.name(), b.about())).collect();
            return self.transcript.notice(format!(
                "/accounts balance <way> sets how this provider's accounts share the work:\n{}",
                ways.join("\n")
            ));
        };
        self.spawn(move |sdk| match sdk.set_balance(provider, balance) {
            Ok(_) => Msg::Notice(format!("Accounts balance: {} ({}).", balance.name(), balance.about())),
            Err(error) => Msg::Error(error.to_string()),
        });
    }

    pub(super) fn account_picked(&mut self, cursor: usize, removing: bool) {
        let Some(Listed { provider, number, label }) = self.catalog.accounts.get(cursor).cloned() else { return };
        self.spawn(move |sdk| {
            let done = if removing {
                sdk.remove_account(provider, &number.to_string()).map(|_| format!("Forgot {label}."))
            } else {
                sdk.use_account(provider, &number.to_string()).map(|_| format!("Using {label} from the next message."))
            };
            done.map_or_else(|error| Msg::Error(error.to_string()), Msg::Notice)
        });
    }
}
