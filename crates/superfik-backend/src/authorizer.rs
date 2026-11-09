//! Who may run which command: the seam for multi-user servers.

use superfik_core::access::{OrgRole, ProjectRole, allows};
use superfik_core::{Member, OrgId, Principal};
use superfik_protocol::ClientCommand;

pub trait Authorizer: Send + Sync {
    fn authorize(&self, principal: &Principal, command: &ClientCommand) -> Result<(), AuthError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    #[error("{principal} may not {command}")]
    Forbidden { principal: String, command: &'static str },
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LocalAuthorizer;

impl Authorizer for LocalAuthorizer {
    fn authorize(&self, principal: &Principal, command: &ClientCommand) -> Result<(), AuthError> {
        match principal {
            Principal::LocalOwner => Ok(()),
            Principal::Member(member) => {
                Err(AuthError::Forbidden { principal: format!("user {}", member.user), command: command.name() })
            }
        }
    }
}

pub trait Roles: Send + Sync {
    fn org_role(&self, member: &Member) -> Option<OrgRole>;
    fn project_role(&self, member: &Member, command: &ClientCommand) -> Option<ProjectRole>;
}

#[derive(Debug)]
pub struct RoleAuthorizer<R> {
    org: OrgId,
    roles: R,
}

impl<R: Roles> RoleAuthorizer<R> {
    pub fn new(org: OrgId, roles: R) -> Self {
        Self { org, roles }
    }
}

impl<R: Roles> Authorizer for RoleAuthorizer<R> {
    fn authorize(&self, principal: &Principal, command: &ClientCommand) -> Result<(), AuthError> {
        let allowed = match principal {
            Principal::LocalOwner => self.org == OrgId::LOCAL,
            Principal::Member(member) => {
                member.org == self.org
                    && self
                        .roles
                        .org_role(member)
                        .is_some_and(|org| allows(org, self.roles.project_role(member, command), command.ability()))
            }
        };
        allowed.then_some(()).ok_or_else(|| forbidden(principal, command.name()))
    }
}

pub fn forbidden(principal: &Principal, command: &'static str) -> AuthError {
    let principal = match principal {
        Principal::LocalOwner => "the local owner".to_owned(),
        Principal::Member(member) => format!("user {}", member.user),
    };
    AuthError::Forbidden { principal, command }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use superfik_core::{OrgId, UserId};
    use superfik_protocol::ClientCommand as C;

    use super::*;

    #[derive(Default)]
    struct Book(HashMap<UserId, (OrgRole, Option<ProjectRole>)>);

    impl Roles for Book {
        fn org_role(&self, member: &Member) -> Option<OrgRole> {
            self.0.get(&member.user).map(|(org, _)| *org)
        }
        fn project_role(&self, member: &Member, _: &ClientCommand) -> Option<ProjectRole> {
            self.0.get(&member.user).and_then(|(_, project)| *project)
        }
    }

    fn uninstall() -> C {
        C::UninstallPlugin { plugin: superfik_core::PluginId("linear".into()) }
    }

    #[test]
    fn roles_decide_what_a_member_may_send() {
        let org = OrgId::new();
        let (viewer, maintainer) = (UserId::new(), UserId::new());
        let mut book = Book::default();
        book.0.insert(viewer, (OrgRole::Member, Some(ProjectRole::Viewer)));
        book.0.insert(maintainer, (OrgRole::Member, Some(ProjectRole::Maintainer)));
        let authorizer = RoleAuthorizer::new(org, book);
        let viewer = Principal::member(viewer, org);
        assert!(authorizer.authorize(&viewer, &C::ListModels).is_ok());
        let error =
            authorizer.authorize(&viewer, &C::CancelRun { run: superfik_core::RunId::new() }).expect_err("deny");
        assert!(error.to_string().ends_with("may not cancel_run"));
        assert!(authorizer.authorize(&Principal::member(maintainer, org), &uninstall()).is_ok());
    }

    #[test]
    fn strangers_and_other_organizations_are_refused() {
        let org = OrgId::new();
        let user = UserId::new();
        let mut book = Book::default();
        book.0.insert(user, (OrgRole::Owner, None));
        let authorizer = RoleAuthorizer::new(org, book);
        assert!(authorizer.authorize(&Principal::member(user, OrgId::new()), &C::ListModels).is_err());
        assert!(authorizer.authorize(&Principal::member(UserId::new(), org), &C::ListModels).is_err());
    }

    #[test]
    fn the_local_owner_owns_the_local_organization_only() {
        let local = RoleAuthorizer::new(OrgId::LOCAL, Book::default());
        assert!(local.authorize(&Principal::LocalOwner, &uninstall()).is_ok());
        let company = RoleAuthorizer::new(OrgId::new(), Book::default());
        assert!(company.authorize(&Principal::LocalOwner, &C::ListModels).is_err());
    }

    #[test]
    fn local_owner_only() {
        let authorizer = LocalAuthorizer;
        assert!(authorizer.authorize(&Principal::LocalOwner, &ClientCommand::ListModels).is_ok());
        let error = authorizer
            .authorize(&Principal::member(UserId::new(), OrgId::new()), &ClientCommand::ListModels)
            .expect_err("deny");
        assert!(error.to_string().ends_with("may not list_models"));
    }
}
