use domain::Principal;
use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope<T> {
    #[serde(default)]
    pub version: u32,
    pub principal: Principal,
    pub payload: T,
}

impl<T> Envelope<T> {
    pub fn new(principal: Principal, payload: T) -> Self {
        Self { version: VERSION, principal, payload }
    }

    pub fn local(payload: T) -> Self {
        Self::new(Principal::LocalOwner, payload)
    }

    pub fn spoken(&self) -> Result<(), String> {
        match self.version {
            VERSION => Ok(()),
            0 => Err(format!("the client named no protocol version; this backend speaks version {VERSION}")),
            other => Err(format!("the client speaks protocol version {other}; this backend speaks version {VERSION}")),
        }
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

    #[test]
    fn only_this_protocol_version_is_spoken() {
        assert_eq!(Envelope::local(1).spoken(), Ok(()));
        let unnamed: Envelope<u8> =
            serde_json::from_value(serde_json::json!({ "principal": "LocalOwner", "payload": 1 })).expect("old");
        assert!(unnamed.spoken().is_err_and(|error| error.contains("no protocol version")));
        let newer = Envelope { version: VERSION + 1, ..Envelope::local(1) };
        assert!(newer.spoken().is_err_and(|error| error.contains(&format!("version {}", VERSION + 1))));
    }
}
