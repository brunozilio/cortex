//! Wraps a message with who sent it.

use serde::{Deserialize, Serialize};
use domain::Principal;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope<T> {
    pub principal: Principal,
    pub payload: T,
}

impl<T> Envelope<T> {
    pub fn new(principal: Principal, payload: T) -> Self {
        Self { principal, payload }
    }

    pub fn local(payload: T) -> Self {
        Self::new(Principal::LocalOwner, payload)
    }
}

#[cfg(test)]
mod tests {
    use domain::{OrgId, UserId};

    use super::*;

    #[test]
    fn local_envelopes_carry_the_local_owner() {
        assert_eq!(Envelope::local(1).principal, Principal::LocalOwner);
    }

    #[test]
    fn round_trips() {
        let envelope = Envelope::new(Principal::member(UserId::new(), OrgId::new()), "hello".to_owned());
        let json = serde_json::to_string(&envelope).expect("serialize");
        let back: Envelope<String> = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, envelope);
    }
}
