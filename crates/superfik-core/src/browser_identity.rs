//! Versioned native hardware experiment configuration. No JavaScript shims.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserIdentity {
    pub schema: u32,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub hardware_concurrency: Option<u16>,
    #[serde(default)]
    pub device_memory: Option<f64>,
}

impl BrowserIdentity {
    pub fn parse(json: &str) -> Result<Self, String> {
        let identity: Self = serde_json::from_str(json).map_err(|error| error.to_string())?;
        if identity.schema != 1 {
            return Err("unsupported native identity schema".into());
        }
        if identity.hardware_concurrency.is_some_and(|value| !(1..=1024).contains(&value)) {
            return Err("hardware_concurrency must be 1..=1024".into());
        }
        if identity.device_memory.is_some_and(|value| ![0.25, 0.5, 1.0, 2.0, 4.0, 8.0].contains(&value)) {
            return Err("device_memory must be a supported memory bucket".into());
        }
        if identity.enabled && identity.hardware_concurrency.is_none() && identity.device_memory.is_none() {
            return Err("enabled identity must specify at least one override".into());
        }
        Ok(identity)
    }
}

#[cfg(test)]
mod tests {
    use super::BrowserIdentity;

    #[test]
    fn hardware_profile_requires_explicit_opt_in() {
        let identity = BrowserIdentity::parse(r#"{"schema":1,"hardware_concurrency":4}"#).expect("valid profile");
        assert!(!identity.enabled);
        let identity =
            BrowserIdentity::parse(r#"{"schema":1,"enabled":true,"hardware_concurrency":4,"device_memory":8}"#)
                .expect("explicit experiment");
        assert!(identity.enabled);
        assert_eq!(identity.hardware_concurrency, Some(4));
        assert_eq!(identity.device_memory, Some(8.0));
    }

    #[test]
    fn malformed_or_unsupported_experiments_are_rejected() {
        for json in [
            r#"{"schema":2}"#,
            r#"{"schema":1,"enabled":true}"#,
            r#"{"schema":1,"hardware_concurrency":0}"#,
            r#"{"schema":1,"hardware_concurrency":1025}"#,
            r#"{"schema":1,"device_memory":3}"#,
            r#"{"schema":1,"gpu":"unsupported"}"#,
        ] {
            assert!(BrowserIdentity::parse(json).is_err(), "accepted: {json}");
        }
    }
}
