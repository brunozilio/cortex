use domain::provider::ProviderSummary;
use sdk::ModelProvider;

use super::app::{App, Msg};
use super::choices::{Picker, PickerKind};
use crate::login::how;

#[derive(Debug)]
pub struct SecretPrompt {
    pub provider: ModelProvider,
    pub name: String,
    pub text: String,
}

pub enum SecretKey {
    Char(char),
    Backspace,
    Enter,
    Esc,
}

impl App {
    pub(super) fn login(&mut self) {
        self.spawn(|sdk| match sdk.providers() {
            Ok(providers) => Msg::Menu(providers),
            Err(error) => Msg::Error(error.to_string()),
        });
    }

    pub(super) fn menu(&mut self, providers: Vec<ProviderSummary>) {
        let items = providers.iter().map(|p| (p.name.clone(), how(p.access).to_owned())).collect();
        self.menu = providers;
        self.picker = Some(Picker { kind: PickerKind::Login, title: "Connect a provider".into(), items, cursor: 0 });
    }

    pub(super) fn login_picked(&mut self, cursor: usize) {
        let Some(ProviderSummary { provider, name, access, .. }) = self.menu.get(cursor).cloned() else { return };
        if !access.subscription() {
            self.secret = Some(SecretPrompt { provider, name, text: String::new() });
            return;
        }
        self.transcript.notice(format!("Signing in to {name}…"));
        let tx = self.tx.clone();
        self.spawn(move |sdk| {
            let opened = |url: &str, code: Option<&str>| {
                crate::login::open(url);
                let _ = tx.send(Msg::Notice(crate::login::visit(url, code)));
            };
            match sdk.sign_in(provider, opened) {
                Ok(_) => Msg::Connected(name),
                Err(error) => Msg::Error(format!("{name} did not connect: {error}")),
            }
        });
    }

    pub(super) fn secret_key(&mut self, key: SecretKey) {
        let Some(prompt) = &mut self.secret else { return };
        match key {
            SecretKey::Char(c) => prompt.text.push(c),
            SecretKey::Backspace => {
                prompt.text.pop();
            }
            SecretKey::Esc => self.secret = None,
            SecretKey::Enter => {
                let Some(SecretPrompt { provider, name, text }) = self.secret.take() else { return };
                self.spawn(move |sdk| match sdk.connect_provider(provider, &text) {
                    Ok(_) => Msg::Connected(name),
                    Err(error) => Msg::Error(format!("{name} did not connect: {error}")),
                });
            }
        }
    }

    pub(super) fn paste_secret(&mut self, text: &str) {
        if let Some(prompt) = &mut self.secret {
            prompt.text.push_str(text.trim());
        }
    }
}
