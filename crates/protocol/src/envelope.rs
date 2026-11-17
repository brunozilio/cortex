use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope<T> {
    #[serde(default)]
    pub version: u32,
    pub payload: T,
}

impl<T> Envelope<T> {
    pub fn local(payload: T) -> Self {
        Self { version: VERSION, payload }
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
    use super::*;

    #[test]
    fn round_trips() {
        let envelope = Envelope::local("hello".to_owned());
        let json = serde_json::to_string(&envelope).expect("serialize");
        let back: Envelope<String> = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, envelope);
    }

    #[test]
    fn an_envelope_that_still_names_a_principal_is_read() {
        let old: Envelope<u8> =
            serde_json::from_value(serde_json::json!({ "version": 1, "principal": "LocalOwner", "payload": 1 }))
                .expect("old");
        assert_eq!(old, Envelope::local(1));
    }

    #[test]
    fn only_this_protocol_version_is_spoken() {
        assert_eq!(Envelope::local(1).spoken(), Ok(()));
        let unnamed: Envelope<u8> = serde_json::from_value(serde_json::json!({ "payload": 1 })).expect("old");
        assert!(unnamed.spoken().is_err_and(|error| error.contains("no protocol version")));
        let newer = Envelope { version: VERSION + 1, ..Envelope::local(1) };
        assert!(newer.spoken().is_err_and(|error| error.contains(&format!("version {}", VERSION + 1))));
    }
}
