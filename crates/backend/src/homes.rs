use std::path::PathBuf;

use domain::Principal;

pub trait Homes: Send + Sync {
    fn home(&self, owner: &Principal) -> Option<PathBuf>;
}

#[derive(Debug, Clone)]
pub struct OwnerHome(Option<PathBuf>);

impl OwnerHome {
    pub fn system() -> Self {
        Self(std::env::home_dir())
    }

    pub fn at(home: impl Into<PathBuf>) -> Self {
        Self(Some(home.into()))
    }

    pub fn none() -> Self {
        Self(None)
    }
}

impl Homes for OwnerHome {
    fn home(&self, owner: &Principal) -> Option<PathBuf> {
        owner.is_local().then(|| self.0.clone()).flatten()
    }
}

#[cfg(test)]
mod tests {
    use domain::{OrgId, UserId};

    use super::*;

    #[test]
    fn the_local_owner_gets_the_machine_s_home() {
        assert_eq!(OwnerHome::at("/home/me").home(&Principal::LocalOwner), Some(PathBuf::from("/home/me")));
    }

    #[test]
    fn a_member_never_gets_the_machine_owner_s_home() {
        let member = Principal::member(UserId::new(), OrgId::new());
        assert_eq!(OwnerHome::at("/home/me").home(&member), None);
    }
}
