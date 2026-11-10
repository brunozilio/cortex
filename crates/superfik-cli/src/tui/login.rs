//! Connecting a provider without leaving the chat (`/login`): pick one;
//! a subscription signs in in the browser, an API key is typed hidden.

use superfik_sdk::ModelProvider;

use super::app::{App, Msg, Picker, PickerKind};
use crate::login::{MENU, how};

/// An API key being typed: shown as dots, never echoed.
#[derive(Debug)]
pub struct SecretPrompt {
    pub provider: ModelProvider,
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
        let items = MENU.iter().map(|p| (p.name().to_owned(), how(*p).to_owned())).collect();
        self.picker = Some(Picker { kind: PickerKind::Login, title: "Connect a provider".into(), items, cursor: 0 });
    }

    pub(super) fn login_picked(&mut self, cursor: usize) {
        let Some(provider) = MENU.get(cursor).copied() else { return };
        if !provider.subscription() {
            self.secret = Some(SecretPrompt { provider, text: String::new() });
            return;
        }
        self.transcript.notice(format!("Signing in to {} in the browser…", provider.name()));
        let tx = self.tx.clone();
        self.spawn(move |superfik| {
            let opened = |url: &str| {
                crate::login::open(url);
                let _ = tx.send(Msg::Notice(format!("If the browser did not open, visit:\n{url}")));
            };
            match superfik.sign_in(provider, opened) {
                Ok(_) => Msg::Connected(provider.name().to_owned()),
                Err(error) => Msg::Error(format!("{} did not connect: {error}", provider.name())),
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
                let Some(SecretPrompt { provider, text }) = self.secret.take() else { return };
                self.spawn(move |superfik| match superfik.connect_provider(provider, &text) {
                    Ok(_) => Msg::Connected(provider.name().to_owned()),
                    Err(error) => Msg::Error(format!("{} did not connect: {error}", provider.name())),
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
