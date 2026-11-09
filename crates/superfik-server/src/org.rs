//! The organization a server serves: its users (`superfik_identity`) and
//! its settings (`settings`), read against the wall clock.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use superfik_backend::Roles;
use superfik_core::access::{Ability, OrgRole, ProjectRole, allows};
use superfik_core::{Member, OrgId, Principal};
use superfik_identity::{AccountStore, Directory};
use superfik_protocol::ClientCommand;

use crate::Authenticator;
use crate::settings::{ServerFile, SettingsFile};

pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

pub fn system_clock() -> Clock {
    Arc::new(|| SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs()))
}

pub struct Organization {
    pub directory: Directory<Arc<dyn AccountStore>>,
    pub clock: Clock,
    pub teams: Arc<dyn crate::teams::TeamStore>,
    settings: Mutex<SettingsFile>,
}

impl std::fmt::Debug for Organization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Organization").field("org", &self.directory.org()).finish_non_exhaustive()
    }
}

impl Organization {
    pub fn new(org: OrgId, store: Arc<dyn AccountStore>, clock: Clock) -> Self {
        Self::with_settings(store, SettingsFile::in_memory(ServerFile::new(org)), clock)
    }

    pub fn with_settings(store: Arc<dyn AccountStore>, mut settings: SettingsFile, clock: Clock) -> Self {
        Self {
            directory: Directory::new(settings.read().org, store),
            clock,
            teams: Arc::new(crate::teams::InMemoryTeams::default()),
            settings: Mutex::new(settings),
        }
    }

    pub fn with_teams(self, teams: Arc<dyn crate::teams::TeamStore>) -> Self {
        Self { teams, ..self }
    }

    pub fn reads(&self, member: &Member) -> bool {
        self.directory.role(member).is_some_and(|role| allows(role, None, Ability::Read))
    }

    pub fn to_readers(&self, event: &superfik_protocol::ServerEvent) -> Vec<superfik_protocol::Addressed> {
        let org = self.directory.org();
        self.directory
            .members()
            .unwrap_or_default()
            .into_iter()
            .map(|account| Member { user: account.user, org })
            .filter(|member| self.reads(member))
            .map(|member| superfik_protocol::Addressed {
                to: superfik_protocol::Recipient::Principal(Principal::Member(member)),
                event: event.clone(),
            })
            .collect()
    }

    pub fn teams_of(&self, member: &Member) -> Vec<superfik_core::TeamId> {
        superfik_core::sharing::teams_of(&self.teams.load(), member.user)
    }

    pub fn now(&self) -> u64 {
        (self.clock)()
    }

    pub fn settings(&self) -> ServerFile {
        self.settings.lock().unwrap_or_else(PoisonError::into_inner).read()
    }

    pub fn change_settings(&self, change: impl FnOnce(&mut ServerFile)) -> std::io::Result<ServerFile> {
        self.settings.lock().unwrap_or_else(PoisonError::into_inner).change(change)
    }

    pub fn origin(&self) -> Option<String> {
        self.settings().domain.as_deref().and_then(origin_of)
    }
}

impl Authenticator for Organization {
    fn authenticate(&self, token: &str) -> Option<Principal> {
        self.directory.authenticate(token, self.now())
    }

    fn sign_out(&self, token: &str) {
        if let Err(error) = self.directory.sign_out(token) {
            tracing::warn!(%error, "could not end a sign-in");
        }
    }
}

pub fn origin_of(domain: &str) -> Option<String> {
    let domain = domain.trim();
    let rest = domain.strip_prefix("https://").or_else(|| domain.strip_prefix("http://")).unwrap_or(domain);
    let host = rest.split(['/', '?', '#']).next().filter(|host| !host.is_empty() && !host.contains('@'))?;
    Some(format!("https://{}", host.to_ascii_lowercase()))
}

#[cfg(test)]
mod origin_tests {
    use super::origin_of;

    #[test]
    fn a_domain_or_address_names_its_https_origin() {
        assert_eq!(origin_of("Superfik.Acme.io"), Some("https://superfik.acme.io".into()));
        assert_eq!(origin_of("https://acme.io:8443/app?x"), Some("https://acme.io:8443".into()));
        assert_eq!(origin_of("https://user@evil.io"), None);
        assert_eq!(origin_of(" "), None);
    }
}

#[derive(Debug, Clone)]
pub struct DirectoryRoles(pub Arc<Organization>);

impl Roles for DirectoryRoles {
    fn org_role(&self, member: &Member) -> Option<OrgRole> {
        self.0.directory.role(member)
    }

    fn project_role(&self, _: &Member, _: &ClientCommand) -> Option<ProjectRole> {
        None
    }
}
