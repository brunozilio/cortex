//! The release manifest each GitHub release carries per platform, and
//! what it offers the running version.

use serde::{Deserialize, Serialize};

use crate::Version;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    pub url: String,
    pub size: u64,
    pub sha256: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: Version,
    #[serde(default)]
    pub minimum: Option<Version>,
    #[serde(default)]
    pub notes_url: Option<String>,
    pub published_unix: u64,
    pub asset: Asset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub version: Version,
    pub required: bool,
    pub asset: Asset,
    pub notes_url: Option<String>,
}

pub fn offer(current: &Version, manifest: &Manifest) -> Option<Offer> {
    (manifest.version > *current).then(|| Offer {
        version: manifest.version,
        required: manifest.minimum.is_some_and(|minimum| *current < minimum),
        asset: manifest.asset.clone(),
        notes_url: manifest.notes_url.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{
        "version": "0.15.0",
        "minimum": "0.13.0",
        "notes_url": "https://github.com/brunozilio/superfik/releases/tag/v0.15.0",
        "published_unix": 1794077310,
        "asset": {
            "url": "https://example.com/Superfik-0.15.0-macos-aarch64.zip",
            "size": 123,
            "sha256": "00",
            "signature": "11"
        }
    }"#;

    fn manifest() -> Manifest {
        serde_json::from_str(JSON).unwrap_or_else(|error| panic!("{error}"))
    }

    #[test]
    fn reads_the_services_json() {
        let manifest = manifest();
        assert_eq!(manifest.version, Version::new(0, 15, 0));
        assert_eq!(manifest.minimum, Some(Version::new(0, 13, 0)));
        assert_eq!(manifest.published_unix, 1794077310);
        assert_eq!(manifest.asset.size, 123);
        assert!(manifest.asset.url.ends_with(".zip"));
    }

    #[test]
    fn minimum_and_notes_may_be_null_or_absent() {
        let null = JSON.replace(r#""0.13.0""#, "null");
        let parsed: Manifest = serde_json::from_str(&null).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(parsed.minimum, None);
        let mut value: serde_json::Value = serde_json::from_str(JSON).unwrap_or_default();
        if let Some(object) = value.as_object_mut() {
            object.remove("minimum");
            object.remove("notes_url");
        }
        let parsed: Manifest = serde_json::from_value(value).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!((parsed.minimum, parsed.notes_url), (None, None));
    }

    #[test]
    fn nothing_is_offered_to_the_same_or_a_newer_version() {
        assert_eq!(offer(&Version::new(0, 15, 0), &manifest()), None);
        assert_eq!(offer(&Version::new(0, 16, 0), &manifest()), None);
    }

    #[test]
    fn a_newer_release_is_offered() {
        let offered = offer(&Version::new(0, 14, 2), &manifest());
        assert_eq!(offered.as_ref().map(|o| (o.version, o.required)), Some((Version::new(0, 15, 0), false)));
        assert_eq!(offered.map(|o| o.asset), Some(manifest().asset));
    }

    #[test]
    fn below_the_minimum_the_update_is_required() {
        let offered = offer(&Version::new(0, 12, 9), &manifest());
        assert_eq!(offered.map(|o| o.required), Some(true));
        let no_minimum = Manifest { minimum: None, ..manifest() };
        assert_eq!(offer(&Version::new(0, 12, 9), &no_minimum).map(|o| o.required), Some(false));
    }
}
