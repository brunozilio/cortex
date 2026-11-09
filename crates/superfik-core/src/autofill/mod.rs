//! Filling a page's sign-in form with a login kept in a vault, after the
//! Bitwarden extension: which saved logins belong to the page ([`uri`]),
//! what the page's fields are ([`PageField`]) and which of them take the
//! username and the password ([`plan`]). Reading the page and placing the
//! values happen in the browser; the rules live here. Pure.

mod equivalent;
mod fields;
mod plan;
mod uri;

pub use fields::PageField;
pub use plan::{FillPlan, plan};
pub use uri::{LoginUri, UriRule, matches_any, same_document};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub item: String,
    pub account: String,
    pub username: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginFill {
    pub username: Option<String>,
    pub password: Option<String>,
}

impl std::fmt::Debug for LoginFill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginFill")
            .field("username", &self.username.as_ref().map(|_| "…"))
            .field("password", &self.password.as_ref().map(|_| "…"))
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewLogin {
    pub name: String,
    pub url: String,
    pub username: String,
    pub password: String,
}

impl std::fmt::Debug for NewLogin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewLogin")
            .field("name", &self.name)
            .field("url", &self.url)
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillOutcome {
    Filled,
    Absent,
    Denied,
}

pub fn sign_in_subject(item: &str, url: &str) -> String {
    let host = url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_owned));
    format!("{item} → sign in on {}", host.as_deref().unwrap_or(url))
}

pub fn agent_answer(outcome: FillOutcome) -> Result<String, String> {
    match outcome {
        FillOutcome::Filled => Ok("Filled the sign-in form. Submit it, then take a new browser_snapshot.".into()),
        FillOutcome::Absent => Err("This page has no sign-in form to fill.".into()),
        FillOutcome::Denied => Err("The page changed before the login was filled; nothing was filled.".into()),
    }
}

#[cfg(test)]
mod tests;
