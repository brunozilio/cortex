//! Test support (feature `testing`): an organization with its users in
//! memory, and members added and signed in the way there is: a
//! first-access code, then a password. Never in a shipped build.

use std::sync::Arc;

use superfik_core::access::OrgRole;
use superfik_core::{Member, OrgId};
use superfik_identity::{Actor, InMemoryAccounts};

use crate::{Clock, Organization};

pub const PASSWORD: &str = "correct horse battery";

pub fn organization(clock: Clock) -> Organization {
    Organization::new(OrgId::new(), Arc::new(InMemoryAccounts::default()), clock)
}

pub fn admit(organization: &Organization, name: &str, role: OrgRole) -> (Member, String) {
    let now = organization.now();
    let (member, code) = organization.directory.add(Actor::Console, name, role, now).expect("the user is added");
    let token = organization.directory.first_access(name, &code, PASSWORD, now).expect("the first access works");
    (member, token)
}
