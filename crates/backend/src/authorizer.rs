//! Who may run which command: the seam for a future multi-user server.

use domain::Principal;
use protocol::ClientCommand;

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
            Principal::Member(_) => Err(forbidden(principal, command.name())),
        }
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
    use domain::{OrgId, UserId};
    use protocol::ClientCommand as C;

    use super::*;

    #[test]
    fn local_owner_only() {
        assert!(LocalAuthorizer.authorize(&Principal::LocalOwner, &C::ListModels).is_ok());
        let member = Principal::member(UserId::new(), OrgId::new());
        assert!(LocalAuthorizer.authorize(&member, &C::ListModels).is_err());
    }
}
