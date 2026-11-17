use std::path::Path;

use crate::api::{ClientCommand, ProviderEvent, Reply};
use domain::provider::{ModelProvider, ProviderSummary};
use domain::{CustomAgent, ModelOption, Skill};

use crate::{Client, Error};

impl Client {
    pub fn models(&self) -> Result<Vec<ModelOption>, Error> {
        self.dispatcher.call(ClientCommand::ListModels, |reply| match reply {
            Reply::Models(models) => Some(Ok(models)),
            _ => None,
        })
    }

    pub fn unsolicited(&self) -> Vec<crate::api::Unsolicited> {
        self.dispatcher.unsolicited()
    }

    pub fn cancel_sign_ins(&self) {
        self.dispatcher.cancel_flows();
    }

    pub fn providers(&self) -> Result<Vec<ProviderSummary>, Error> {
        self.dispatcher.call(ClientCommand::ListProviders, listed)
    }

    pub fn connect_provider(&self, provider: ModelProvider, key: &str) -> Result<Vec<ProviderSummary>, Error> {
        self.dispatcher.call(ClientCommand::ConnectProvider { provider, key: key.to_owned() }, listed)
    }

    pub fn sign_in(&self, provider: ModelProvider, open: impl FnMut(&str, Option<&str>)) -> Result<SignedIn, Error> {
        self.signing_in(ClientCommand::SignInProvider { provider }, open)
    }

    pub fn close_and_sign_in(
        &self,
        provider: ModelProvider,
        port: u16,
        pid: u32,
        open: impl FnMut(&str, Option<&str>),
    ) -> Result<SignedIn, Error> {
        self.signing_in(ClientCommand::CloseAndSignIn { provider, port, pid }, open)
    }

    fn signing_in(&self, command: ClientCommand, mut open: impl FnMut(&str, Option<&str>)) -> Result<SignedIn, Error> {
        self.dispatcher.flow(command, crate::client::dispatch::sign_in_patience(), |reply| match reply {
            Reply::Providers(ProviderEvent::AuthorizeAt { url, code, .. }) => {
                open(&url, code.as_deref());
                None
            }
            Reply::Providers(ProviderEvent::PortBusy { port, holder, .. }) => {
                Some(Ok(SignedIn::PortBusy { port, holder }))
            }
            other => listed(other).map(|answer| answer.map(SignedIn::Connected)),
        })
    }

    pub fn disconnect_provider(&self, provider: ModelProvider) -> Result<Vec<ProviderSummary>, Error> {
        self.dispatcher.call(ClientCommand::DisconnectProvider { provider }, listed)
    }

    pub fn use_account(&self, provider: ModelProvider, account: &str) -> Result<Vec<ProviderSummary>, Error> {
        self.dispatcher.call(ClientCommand::UseProviderAccount { provider, account: account.to_owned() }, listed)
    }

    pub fn set_balance(
        &self,
        provider: ModelProvider,
        balance: domain::account_pool::Balance,
    ) -> Result<Vec<ProviderSummary>, Error> {
        self.dispatcher.call(ClientCommand::SetProviderBalance { provider, balance }, listed)
    }

    pub fn remove_account(&self, provider: ModelProvider, account: &str) -> Result<Vec<ProviderSummary>, Error> {
        self.dispatcher.call(ClientCommand::RemoveProviderAccount { provider, account: account.to_owned() }, listed)
    }

    pub fn register_peer(&self, cwd: &Path) -> Result<(), Error> {
        self.dispatcher.call(ClientCommand::RegisterPeer { path: cwd.to_path_buf() }, |reply| match reply {
            Reply::Done => Some(Ok(())),
            _ => None,
        })
    }

    pub fn skills(&self, cwd: &Path) -> Result<Vec<Skill>, Error> {
        self.dispatcher.call(ClientCommand::ListSkills { path: cwd.to_path_buf() }, |reply| match reply {
            Reply::Skills(skills) => Some(Ok(skills)),
            _ => None,
        })
    }

    pub fn agents(&self, cwd: &Path) -> Result<Vec<CustomAgent>, Error> {
        self.dispatcher.call(ClientCommand::ListAgents { path: cwd.to_path_buf() }, |reply| match reply {
            Reply::Agents(agents) => Some(Ok(agents)),
            _ => None,
        })
    }

    pub fn settings(&self) -> Result<serde_json::Value, Error> {
        self.dispatcher.call(ClientCommand::LoadSettings, |reply| match reply {
            Reply::Settings { settings } => Some(Ok(settings.unwrap_or_else(|| serde_json::json!({})))),
            _ => None,
        })
    }

    pub fn change_settings(&self, changes: Vec<crate::settings::SettingsChange>) -> Result<(), Error> {
        self.dispatcher.call(ClientCommand::ChangeSettings { changes }, |reply| match reply {
            Reply::Done => Some(Ok(())),
            _ => None,
        })
    }

    pub fn save_settings(&self, settings: serde_json::Value) -> Result<(), Error> {
        self.dispatcher.call(ClientCommand::SaveSettings { settings }, |reply| match reply {
            Reply::Done => Some(Ok(())),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignedIn {
    Connected(Vec<ProviderSummary>),
    PortBusy { port: u16, holder: Option<domain::provider::PortHolder> },
}

fn listed(reply: Reply) -> Option<Result<Vec<ProviderSummary>, String>> {
    match reply {
        Reply::Providers(ProviderEvent::Listed { providers }) => Some(Ok(providers)),
        Reply::Providers(ProviderEvent::Failed { message, .. }) => Some(Err(message)),
        _ => None,
    }
}
