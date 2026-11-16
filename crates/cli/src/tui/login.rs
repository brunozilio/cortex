use domain::provider::ProviderSummary;
use sdk::{ModelProvider, PortHolder, SignedIn};

use super::app::{App, Msg};
use super::choices::{Picker, PickerKind};
use crate::login::how;

#[derive(Debug)]
pub struct SecretPrompt {
    pub provider: ModelProvider,
    pub name: String,
    pub text: String,
}

#[derive(Debug)]
pub struct PortBusy {
    pub provider: ModelProvider,
    pub name: String,
    pub port: u16,
    pub holder: Option<PortHolder>,
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
        self.catalog.menu = providers;
        self.overlays.picker =
            Some(Picker { kind: PickerKind::Login, title: "Connect a provider".into(), items, cursor: 0 });
    }

    pub(super) fn login_picked(&mut self, cursor: usize) {
        let Some(ProviderSummary { provider, name, access, .. }) = self.catalog.menu.get(cursor).cloned() else {
            return;
        };
        if !access.subscription() {
            self.overlays.secret = Some(SecretPrompt { provider, name, text: String::new() });
            return;
        }
        self.transcript.passing(format!("Signing in to {name}…"));
        self.signing_in(provider, name, None);
    }

    fn signing_in(&mut self, provider: ModelProvider, name: String, close: Option<(u16, u32)>) {
        let tx = self.tx.clone();
        self.spawn(move |sdk| {
            let opened = |url: &str, code: Option<&str>| {
                crate::login::open(url);
                let _ = tx.send(Msg::Passing(crate::login::visit(url, code)));
            };
            let signed_in = match close {
                Some((port, pid)) => sdk.close_and_sign_in(provider, port, pid, opened),
                None => sdk.sign_in(provider, opened),
            };
            match signed_in {
                Ok(SignedIn::Connected(_)) => Msg::Connected(name),
                Ok(SignedIn::PortBusy { port, holder }) => {
                    Msg::Feature(Box::new(move |app: &mut App| app.port_busy(provider, name, port, holder)))
                }
                Err(error) => Msg::Error(format!("{name} did not connect: {error}")),
            }
        });
    }

    pub(super) fn port_busy(&mut self, provider: ModelProvider, name: String, port: u16, holder: Option<PortHolder>) {
        let who = holder.as_ref().map_or("another app".to_owned(), |h| format!("{} (PID {})", h.app, h.pid));
        let title = format!("Port {port} is in use by {who}");
        self.transcript.error(format!("{name} did not connect: its sign-in needs port {port}, in use by {who}"));
        let mut items = Vec::new();
        if let Some(holder) = &holder {
            items.push((format!("Close {} and try again", holder.app), format!("ends PID {}", holder.pid)));
        }
        items.push(("Try again".to_owned(), "after closing it yourself".to_owned()));
        items.push(("Cancel".to_owned(), String::new()));
        self.overlays.port_busy = Some(PortBusy { provider, name, port, holder });
        self.overlays.picker = Some(Picker { kind: PickerKind::PortBusy, title, items, cursor: 0 });
    }

    pub(super) fn port_busy_picked(&mut self, cursor: usize) {
        let Some(PortBusy { provider, name, port, holder }) = self.overlays.port_busy.take() else { return };
        let close = holder.map(|holder| (port, holder.pid));
        let retry = match (close, cursor) {
            (Some(close), 0) => Some(Some(close)),
            (Some(_), 1) | (None, 0) => Some(None),
            _ => None,
        };
        if let Some(close) = retry {
            self.transcript.passing(format!("Signing in to {name}…"));
            self.signing_in(provider, name, close);
        }
    }

    pub(super) fn secret_key(&mut self, key: SecretKey) {
        let Some(prompt) = &mut self.overlays.secret else { return };
        match key {
            SecretKey::Char(c) => prompt.text.push(c),
            SecretKey::Backspace => {
                prompt.text.pop();
            }
            SecretKey::Esc => self.overlays.secret = None,
            SecretKey::Enter => {
                let Some(SecretPrompt { provider, name, text }) = self.overlays.secret.take() else { return };
                self.spawn(move |sdk| match sdk.connect_provider(provider, &text) {
                    Ok(_) => Msg::Connected(name),
                    Err(error) => Msg::Error(format!("{name} did not connect: {error}")),
                });
            }
        }
    }

    pub(super) fn paste_secret(&mut self, text: &str) {
        if let Some(prompt) = &mut self.overlays.secret {
            prompt.text.push_str(text.trim());
        }
    }
}
