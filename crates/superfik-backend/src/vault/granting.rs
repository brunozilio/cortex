//! The Superfik Vault's credential grants for a run: asked for at once
//! (`request_credentials`), kept until released or the run ends, and
//! filled on the page without asking again.

use serde_json::Value;
use superfik_core::{Principal, RunId};

use super::credentials::{self, Granted, Kind, Status};
use super::{APPROVAL_LIMIT, Grant, Vault, asker, locked};

impl Vault {
    pub fn granted_login(&self, owner: &Principal, run: RunId, website: Option<&str>) -> Result<String, String> {
        let grant = self.run_grant(owner, run)?;
        let granted = locked(&grant.granted);
        let logins: Vec<&Granted> = granted
            .iter()
            .filter(|g| g.kind == Kind::Login)
            .filter(|g| website.is_none_or(|site| g.website.as_deref().is_some_and(|w| same_site(w, site))))
            .collect();
        match logins.as_slice() {
            [login] => Ok(login.item.clone()),
            [] => Err("no_match: no login is approved for this; call request_credentials first.".into()),
            _ => Err("Several logins are approved; give the `website` to fill.".into()),
        }
    }

    pub(super) fn login_approved(grant: &Grant, item: &str) -> bool {
        locked(&grant.granted).iter().any(|g| g.kind == Kind::Login && g.item == item)
    }

    pub(super) fn request_credentials(&self, grant: &Grant, arguments: &Value) -> Result<String, String> {
        let request = credentials::parse(arguments)?;
        let folders = self.folders(grant)?;
        let found: Vec<Option<Granted>> = request
            .entries
            .iter()
            .map(|entry| match entry.kind {
                Kind::Login => credentials::find_login(&folders, entry),
                Kind::Address | Kind::Card => None,
            })
            .collect();
        let asked: Vec<Granted> = found.iter().flatten().cloned().collect();
        let subject = credentials::subject(&request, &asked);
        let approved = !asked.is_empty()
            && self.approvals.ask(&grant.owner, grant.run, &asker(&grant.accounts), subject, APPROVAL_LIMIT);
        let statuses: Vec<Status> = request
            .entries
            .iter()
            .zip(found)
            .map(|(entry, found)| match (entry.kind, found) {
                (Kind::Address | Kind::Card, _) => Status::Unsupported,
                (Kind::Login, None) => Status::NotFound,
                (Kind::Login, Some(granted)) if approved => Status::Approved(granted),
                (Kind::Login, Some(_)) => Status::Denied,
            })
            .collect();
        let mut granted = locked(&grant.granted);
        for status in &statuses {
            if let Status::Approved(credential) = status
                && !granted.contains(credential)
            {
                granted.push(credential.clone());
            }
        }
        Ok(credentials::report(&request, &statuses))
    }

    pub(super) fn list_granted(grant: &Grant) -> String {
        credentials::listing(&locked(&grant.granted))
    }

    pub(super) fn release(grant: &Grant) -> String {
        locked(&grant.granted).clear();
        "Released every credential approved for this run.".to_owned()
    }
}

fn same_site(a: &str, b: &str) -> bool {
    let host = |url: &str| {
        let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
        rest.split(['/', ':', '?', '#']).next().unwrap_or_default().trim_start_matches("www.").to_lowercase()
    };
    host(a) == host(b)
}
