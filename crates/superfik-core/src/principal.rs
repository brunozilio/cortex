//! Who is acting. Every command carries a `Principal` so a server can
//! authorize it: the local owner of the embedded server, or a member of an
//! organization.

use serde::{Deserialize, Serialize};

use crate::ids::{OrgId, UserId};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Principal {
    LocalOwner,
    Member(Member),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Member {
    pub user: UserId,
    pub org: OrgId,
}

impl Principal {
    pub fn member(user: UserId, org: OrgId) -> Self {
        Self::Member(Member { user, org })
    }

    pub fn is_local(&self) -> bool {
        matches!(self, Self::LocalOwner)
    }

    pub fn org(&self) -> OrgId {
        match self {
            Self::LocalOwner => OrgId::LOCAL,
            Self::Member(member) => member.org,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_local_owner_belongs_to_the_local_organization() {
        assert_eq!(Principal::LocalOwner.org(), OrgId::LOCAL);
    }

    #[test]
    fn a_member_acts_for_their_organization() {
        let org = OrgId::new();
        let member = Principal::member(UserId::new(), org);
        assert_eq!(member.org(), org);
        assert!(!member.is_local());
    }
}
