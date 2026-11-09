//! `request_credentials`: what a task asks the Superfik Vault for at once
//! (logins, addresses, cards), which saved item answers each entry, the
//! one approval the user sees, and what the run is told back. The run only
//! ever learns names and statuses, never values. Pure.

use serde_json::Value;
use superfik_core::autofill::matches_any;

use super::lookup::Folder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Login,
    Address,
    Card,
}

impl Kind {
    const fn name(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::Address => "address",
            Self::Card => "card",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub kind: Kind,
    pub website: Option<String>,
    pub reason: String,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub goal: String,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Granted {
    pub kind: Kind,
    pub website: Option<String>,
    pub item: String,
    pub account: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Approved(Granted),
    Denied,
    NotFound,
    Unsupported,
}

pub fn parse(arguments: &Value) -> Result<Request, String> {
    let entries =
        arguments["entries"].as_array().filter(|e| !e.is_empty()).ok_or("List what the task needs in `entries`.")?;
    let entries = entries
        .iter()
        .map(|entry| {
            let kind = match entry["kind"].as_str() {
                Some("login") => Kind::Login,
                Some("address") => Kind::Address,
                Some("card") => Kind::Card,
                other => return Err(format!("`kind` must be login, address or card, not {other:?}.")),
            };
            let website = entry["website"].as_str().map(str::trim).filter(|w| !w.is_empty()).map(str::to_owned);
            if kind == Kind::Login && website.is_none() {
                return Err("A login entry needs its `website`.".into());
            }
            let keywords = entry["keywords"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|k| k.as_str().map(|k| k.trim().to_lowercase()))
                .filter(|k| !k.is_empty())
                .take(5)
                .collect();
            Ok(Entry {
                kind,
                website,
                reason: entry["reason"].as_str().unwrap_or_default().trim().to_owned(),
                keywords,
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(Request { goal: arguments["goal"].as_str().unwrap_or_default().trim().to_owned(), entries })
}

pub fn find_login(folders: &[Folder], entry: &Entry) -> Option<Granted> {
    let website = entry.website.as_deref()?;
    folders
        .iter()
        .flat_map(|folder| folder.items.iter().map(move |item| (folder, item)))
        .filter(|(_, item)| matches_any(&item.uris, website))
        .max_by_key(|(_, item)| {
            let name = item.name.to_lowercase();
            entry.keywords.iter().filter(|k| name.contains(k.as_str())).count()
        })
        .map(|(folder, item)| Granted {
            kind: Kind::Login,
            website: Some(website.to_owned()),
            item: item.name.clone(),
            account: folder.title.clone(),
        })
}

pub fn subject(request: &Request, found: &[Granted]) -> String {
    let mut lines = Vec::new();
    if !request.goal.is_empty() {
        lines.push(request.goal.clone());
    }
    for granted in found {
        let site = granted.website.as_deref().unwrap_or_default();
        lines.push(format!("{} → {} ({}) on {site}", granted.item, granted.kind.name(), granted.account));
    }
    lines.join("\n")
}

pub fn report(request: &Request, statuses: &[Status]) -> String {
    request
        .entries
        .iter()
        .zip(statuses)
        .map(|(entry, status)| {
            let what =
                format!("{} {}", entry.kind.name(), entry.website.as_deref().unwrap_or_default()).trim().to_owned();
            match status {
                Status::Approved(granted) => {
                    format!(
                        "{what}: approved ({} in {}); use autofill_credential on the page",
                        granted.item, granted.account
                    )
                }
                Status::Denied => format!("{what}: denied by the user; do not ask again unless they say so"),
                Status::NotFound => format!("{what}: not_found; nothing saved in the session's vault answers it"),
                Status::Unsupported => {
                    format!("{what}: unsupported; the Superfik Vault cannot fill {}s yet", entry.kind.name())
                }
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn listing(granted: &[Granted]) -> String {
    if granted.is_empty() {
        return "No credential is approved for this run; call request_credentials first.".into();
    }
    granted
        .iter()
        .map(|g| {
            format!("- {} {}: {} ({})", g.kind.name(), g.website.as_deref().unwrap_or_default(), g.item, g.account)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;
    use superfik_core::autofill::LoginUri;

    use super::*;
    use crate::vault::Item;

    fn login(name: &str, uri: &str) -> Item {
        Item { name: name.into(), values: BTreeMap::new(), uris: vec![LoginUri::new(uri, None)] }
    }

    fn folders() -> Vec<Folder> {
        vec![Folder {
            title: "Bitwarden · Work".into(),
            name: "Work".into(),
            items: vec![
                login("GitHub personal", "https://github.com"),
                login("GitHub work", "https://github.com/login"),
                login("Amazon", "https://www.amazon.com"),
            ],
        }]
    }

    fn request() -> Request {
        parse(&json!({
            "goal": "Open a pull request on github.com",
            "entries": [
                { "kind": "login", "website": "https://github.com", "reason": "Sign in to github.com", "keywords": ["GitHub", "work"] },
                { "kind": "card", "reason": "Pay", "keywords": ["visa"] },
            ],
        }))
        .expect("request")
    }

    #[test]
    fn reads_entries_and_refuses_what_cannot_be_looked_for() {
        let request = request();
        assert_eq!(request.entries[0].keywords, ["github", "work"]);
        assert_eq!(request.entries[1].kind, Kind::Card);
        assert!(parse(&json!({ "entries": [] })).is_err());
        assert!(parse(&json!({ "entries": [{ "kind": "login" }] })).expect_err("no site").contains("website"));
        assert!(parse(&json!({ "entries": [{ "kind": "passport" }] })).is_err());
    }

    #[test]
    fn finds_the_login_for_the_site_that_fits_the_keywords_best() {
        let request = request();
        let found = find_login(&folders(), &request.entries[0]).expect("found");
        assert_eq!((found.item.as_str(), found.account.as_str()), ("GitHub work", "Bitwarden · Work"));
        let elsewhere = Entry { website: Some("https://gitlab.com".into()), ..request.entries[0].clone() };
        assert_eq!(find_login(&folders(), &elsewhere), None);
    }

    #[test]
    fn tells_the_user_what_and_the_run_only_names_and_statuses() {
        let request = request();
        let found = find_login(&folders(), &request.entries[0]).expect("found");
        assert_eq!(
            subject(&request, std::slice::from_ref(&found)),
            "Open a pull request on github.com\nGitHub work → login (Bitwarden · Work) on https://github.com"
        );
        let report = report(&request, &[Status::Approved(found.clone()), Status::Unsupported]);
        assert!(report.starts_with("login https://github.com: approved (GitHub work in Bitwarden · Work)"));
        assert!(report.ends_with("card: unsupported; the Superfik Vault cannot fill cards yet"));
        assert!(listing(&[found]).contains("- login https://github.com: GitHub work (Bitwarden · Work)"));
        assert!(listing(&[]).contains("request_credentials first"));
    }
}
