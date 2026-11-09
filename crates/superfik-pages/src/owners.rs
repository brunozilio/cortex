//! Whose each page is. Only who opened a page may drive it, each person's
//! persistent profiles are their own, and pages go only to the public web
//! (`superfik_browser::address::server_may_open`). Pure.

use std::collections::HashMap;

use superfik_browser::{Call, address};
use superfik_core::page::{PageCall, PageReport, Profile};
use superfik_core::{BrowserId, BrowserOp, Principal};

#[derive(Debug, Clone, PartialEq)]
pub enum Admitted {
    Call(Call),
    Refresh(BrowserId),
    Refused,
    Forbidden(PageReport),
}

fn forbidden(call: &PageCall) -> Option<PageReport> {
    let refused = |url: &str| address::server_may_open(url).err().map(|why| why.to_string());
    match call {
        PageCall::Open { url, .. } | PageCall::Navigate(_, url) => {
            refused(url).map(|error| PageReport::Failed { url: url.clone(), error })
        }
        PageCall::Agent { request, op: BrowserOp::Navigate { url }, .. } => address::resolve(url)
            .and_then(|url| refused(&url))
            .map(|error| PageReport::Answered { request: request.clone(), answer: Err(error) }),
        _ => None,
    }
}

#[derive(Debug, Default)]
pub struct Owners {
    pages: HashMap<BrowserId, Principal>,
}

impl Owners {
    pub fn admit(&mut self, principal: &Principal, call: PageCall) -> Admitted {
        let id = call.page();
        match (&call, self.pages.get(&id)) {
            (_, Some(owner)) if owner != principal => return Admitted::Refused,
            (PageCall::Open { .. }, _) | (_, Some(_)) if let Some(report) = forbidden(&call) => {
                return Admitted::Forbidden(report);
            }
            (PageCall::Open { .. }, _) => {
                self.pages.insert(id, principal.clone());
            }
            (_, None) => return Admitted::Refused,
            (PageCall::Close(_), Some(_)) => {
                self.pages.remove(&id);
            }
            _ => {}
        }
        let call = match call {
            PageCall::Open { id, url, profile, size, scale } => {
                PageCall::Open { id, url, profile: profile_of(principal, profile), size, scale }
            }
            call => call,
        };
        Call::from_remote(call).map_or(Admitted::Refresh(id), Admitted::Call)
    }

    pub fn owner(&self, id: BrowserId) -> Option<&Principal> {
        self.pages.get(&id)
    }

    pub fn pages(&self) -> impl Iterator<Item = BrowserId> + '_ {
        self.pages.keys().copied()
    }
}

pub fn profile_of(principal: &Principal, profile: Profile) -> Profile {
    match (principal, profile) {
        (Principal::LocalOwner, profile) => profile,
        (Principal::Member(member), Profile::Persistent(name)) => {
            Profile::Persistent(format!("{}-{}-{name}", member.org, member.user))
        }
        (_, Profile::Ephemeral) => Profile::Ephemeral,
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::{OrgId, UserId};

    use super::*;

    fn open(id: BrowserId, profile: Profile) -> PageCall {
        PageCall::Open { id, url: "https://a.test".into(), profile, size: (10, 10), scale: 1.0 }
    }

    #[test]
    fn only_who_opened_a_page_drives_it() {
        let mut owners = Owners::default();
        let (ana, bia) =
            (Principal::member(UserId::new(), OrgId::new()), Principal::member(UserId::new(), OrgId::new()));
        let id = BrowserId::new();
        assert_eq!(owners.admit(&ana, PageCall::Reload(id)), Admitted::Refused, "not open");
        assert!(matches!(owners.admit(&ana, open(id, Profile::Ephemeral)), Admitted::Call(Call::Open { .. })));
        assert_eq!(owners.admit(&bia, PageCall::Reload(id)), Admitted::Refused);
        assert_eq!(owners.admit(&bia, open(id, Profile::Ephemeral)), Admitted::Refused, "the id is taken");
        assert_eq!(owners.admit(&ana, PageCall::Reload(id)), Admitted::Call(Call::Reload(id)));
        assert_eq!(owners.admit(&ana, PageCall::Refresh(id)), Admitted::Refresh(id));
        assert_eq!(owners.admit(&ana, PageCall::Close(id)), Admitted::Call(Call::Close(id)));
        assert_eq!(owners.owner(id), None);
        assert_eq!(owners.admit(&ana, PageCall::Reload(id)), Admitted::Refused, "closed");
    }

    #[test]
    fn a_page_opens_and_goes_only_to_the_public_web() {
        let mut owners = Owners::default();
        let ana = Principal::member(UserId::new(), OrgId::new());
        let id = BrowserId::new();
        let at =
            |url: &str| PageCall::Open { id, url: url.into(), profile: Profile::Ephemeral, size: (1, 1), scale: 1.0 };
        let refused = owners.admit(&ana, at("file:///etc/passwd"));
        assert!(matches!(refused, Admitted::Forbidden(PageReport::Failed { .. })), "{refused:?}");
        assert_eq!(owners.owner(id), None, "a refused page is nobody's");
        assert!(matches!(owners.admit(&ana, at("https://a.test")), Admitted::Call(Call::Open { .. })));
        let metadata = PageCall::Navigate(id, "http://169.254.169.254/latest/meta-data".into());
        assert!(matches!(owners.admit(&ana, metadata), Admitted::Forbidden(PageReport::Failed { .. })));
        assert_eq!(owners.owner(id), Some(&ana), "still open");
        let request = superfik_core::BrowseId("call-1".into());
        let agent =
            |url: &str| PageCall::Agent { id, request: request.clone(), op: BrowserOp::Navigate { url: url.into() } };
        let typed = owners.admit(&ana, agent("localhost:3000"));
        assert!(matches!(typed, Admitted::Forbidden(PageReport::Answered { answer: Err(_), .. })), "{typed:?}");
        assert!(matches!(owners.admit(&ana, agent("example.com")), Admitted::Call(Call::Agent { .. })));
    }

    #[test]
    fn members_profiles_are_their_own() {
        let ana = Principal::member(UserId::new(), OrgId::new());
        let bia = Principal::member(UserId::new(), OrgId::new());
        let same = Profile::Persistent("conversation-1".into());
        assert_ne!(profile_of(&ana, same.clone()), profile_of(&bia, same.clone()));
        assert_eq!(profile_of(&Principal::LocalOwner, same.clone()), same);
        assert_eq!(profile_of(&ana, Profile::Ephemeral), Profile::Ephemeral);
    }
}
