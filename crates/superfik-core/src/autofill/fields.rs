//! A page's form fields as the read script reports them, and the words
//! that name a username field (after Bitwarden's `autofill-constants.ts`).

use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct PageField {
    #[serde(rename = "ref")]
    pub reference: u32,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub id: String,
    pub autocomplete: String,
    pub placeholder: String,
    pub label: String,
    pub form: Option<u32>,
    pub visible: bool,
    pub readonly: bool,
    pub disabled: bool,
    pub focused: bool,
}

impl PageField {
    pub(super) fn usable(&self) -> bool {
        self.visible && !self.readonly && !self.disabled
    }

    pub(super) fn is_password(&self) -> bool {
        self.kind == "password" && self.usable()
    }

    pub(super) fn takes_text(&self) -> bool {
        matches!(self.kind.as_str(), "text" | "email" | "tel" | "")
    }

    fn autocomplete_has(&self, token: &str) -> bool {
        self.autocomplete.split_ascii_whitespace().any(|t| t.eq_ignore_ascii_case(token))
    }

    pub(super) fn wants_new_password(&self) -> bool {
        self.autocomplete_has("new-password")
    }

    pub(super) fn names_username(&self) -> bool {
        if self.autocomplete_has("username") || self.autocomplete_has("email") || self.kind == "email" {
            return true;
        }
        let words = self.words();
        !contains_any(&words, NOT_USERNAME) && contains_any(&words, USERNAME)
    }

    pub(super) fn names_something_else(&self) -> bool {
        self.autocomplete_has("one-time-code") || self.kind == "search" || contains_any(&self.words(), NOT_USERNAME)
    }

    fn words(&self) -> String {
        [&self.name, &self.id, &self.placeholder, &self.label, &self.autocomplete]
            .iter()
            .map(|s| s.to_lowercase().replace(['-', '_'], " "))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn contains_any(words: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|k| words.contains(k))
}

const USERNAME: &[&str] = &[
    "username",
    "user name",
    "userid",
    "user id",
    "login",
    "email",
    "e mail",
    "account",
    "customer id",
    "usuario",
    "usuário",
    "nome de usuário",
    "conta",
    "identifier",
    "handle",
];

const NOT_USERNAME: &[&str] =
    &["search", "pesquis", "buscar", "captcha", "otp", "one time", "verification", "verificação", "código", "code"];
