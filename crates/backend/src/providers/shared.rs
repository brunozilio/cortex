use domain::Principal;

pub trait SharedCredentials: Send + Sync {
    fn holder(&self, owner: &Principal) -> Option<Principal>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NothingShared;

impl SharedCredentials for NothingShared {
    fn holder(&self, _owner: &Principal) -> Option<Principal> {
        None
    }
}
