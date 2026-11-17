use app::{ModelProvider, PortHolder, SignedIn};
use domain::provider::ProviderSummary;

use super::app::{App, Msg};
use super::picker::{Picker, PickerKind};
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

pub(super) fn open(app: &mut App) {
    app.spawn(|client| match client.providers() {
        Ok(providers) => Msg::Menu(providers),
        Err(error) => Msg::Error(error.to_string()),
    });
}

pub(super) fn menu(app: &mut App, providers: Vec<ProviderSummary>) {
    let items = providers.iter().map(|p| (p.name.clone(), how(p.access).to_owned())).collect();
    app.catalog.menu = providers;
    app.overlays.picker =
        Some(Picker { kind: PickerKind::Login, title: "Connect a provider".into(), items, cursor: 0 });
}

pub(super) fn picked(app: &mut App, cursor: usize) {
    let Some(ProviderSummary { provider, name, access, .. }) = app.catalog.menu.get(cursor).cloned() else {
        return;
    };
    if !access.subscription() {
        app.overlays.secret = Some(SecretPrompt { provider, name, text: String::new() });
        return;
    }
    app.transcript.passing(format!("Signing in to {name}…"));
    signing_in(app, provider, name, None);
}

fn signing_in(app: &mut App, provider: ModelProvider, name: String, close: Option<(u16, u32)>) {
    let tx = app.tx.clone();
    app.spawn(move |client| {
        let opened = |url: &str, code: Option<&str>| {
            crate::login::open(url);
            let _ = tx.send(Msg::Passing(crate::login::visit(url, code)));
        };
        let signed_in = match close {
            Some((port, pid)) => client.close_and_sign_in(provider, port, pid, opened),
            None => client.sign_in(provider, opened),
        };
        match signed_in {
            Ok(SignedIn::Connected(_)) => Msg::Connected(name),
            Ok(SignedIn::PortBusy { port, holder }) => Msg::PortBusy(PortBusy { provider, name, port, holder }),
            Err(error) => Msg::Error(format!("{name} did not connect: {error}")),
        }
    });
}

pub(super) fn port_busy(app: &mut App, busy: PortBusy) {
    let PortBusy { name, port, holder, .. } = &busy;
    let who = holder.as_ref().map_or("another app".to_owned(), |h| format!("{} (PID {})", h.app, h.pid));
    let title = format!("Port {port} is in use by {who}");
    app.transcript.error(format!("{name} did not connect: its sign-in needs port {port}, in use by {who}"));
    let mut items = Vec::new();
    if let Some(holder) = holder {
        items.push((format!("Close {} and try again", holder.app), format!("ends PID {}", holder.pid)));
    }
    items.push(("Try again".to_owned(), "after closing it yourself".to_owned()));
    items.push(("Cancel".to_owned(), String::new()));
    app.overlays.port_busy = Some(busy);
    app.overlays.picker = Some(Picker { kind: PickerKind::PortBusy, title, items, cursor: 0 });
}

pub(super) fn port_busy_picked(app: &mut App, cursor: usize) {
    let Some(PortBusy { provider, name, port, holder }) = app.overlays.port_busy.take() else { return };
    let close = holder.map(|holder| (port, holder.pid));
    let retry = match (close, cursor) {
        (Some(close), 0) => Some(Some(close)),
        (Some(_), 1) | (None, 0) => Some(None),
        _ => None,
    };
    if let Some(close) = retry {
        app.transcript.passing(format!("Signing in to {name}…"));
        signing_in(app, provider, name, close);
    }
}

pub(super) fn secret_key(app: &mut App, key: SecretKey) {
    let Some(prompt) = &mut app.overlays.secret else { return };
    match key {
        SecretKey::Char(c) => prompt.text.push(c),
        SecretKey::Backspace => {
            prompt.text.pop();
        }
        SecretKey::Esc => app.overlays.secret = None,
        SecretKey::Enter => {
            let Some(SecretPrompt { provider, name, text }) = app.overlays.secret.take() else { return };
            app.spawn(move |client| match client.connect_provider(provider, &text) {
                Ok(_) => Msg::Connected(name),
                Err(error) => Msg::Error(format!("{name} did not connect: {error}")),
            });
        }
    }
}

pub(super) fn paste_secret(app: &mut App, text: &str) {
    if let Some(prompt) = &mut app.overlays.secret {
        prompt.text.push_str(text.trim());
    }
}
