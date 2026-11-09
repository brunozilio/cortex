//! Versions (`major.minor.patch`) and the platforms a release is built for.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl Version {
    pub const fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self { major, minor, patch }
    }

    pub fn current() -> Self {
        env!("CARGO_PKG_VERSION").parse().unwrap_or(Self::new(0, 0, 0))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a version: {0}")]
pub struct BadVersion(pub String);

impl FromStr for Version {
    type Err = BadVersion;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let bad = || BadVersion(text.to_owned());
        let trimmed = text.trim();
        let digits = trimmed.strip_prefix('v').unwrap_or(trimmed);
        let mut parts = digits.split('.').map(|part| {
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return Err(bad());
            }
            part.parse::<u64>().map_err(|_| bad())
        });
        let (Some(major), Some(minor), Some(patch), None) = (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(bad());
        };
        Ok(Self::new(major?, minor?, patch?))
    }
}

impl TryFrom<String> for Version {
    type Error = BadVersion;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<Version> for String {
    fn from(version: Version) -> Self {
        version.to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    MacosAarch64,
    MacosX86_64,
    LinuxX86_64,
    WindowsX86_64,
}

impl Platform {
    pub const ALL: [Platform; 4] =
        [Platform::MacosAarch64, Platform::MacosX86_64, Platform::LinuxX86_64, Platform::WindowsX86_64];

    pub fn current() -> Option<Self> {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Some(Self::MacosAarch64)
        } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
            Some(Self::MacosX86_64)
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Some(Self::LinuxX86_64)
        } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            Some(Self::WindowsX86_64)
        } else {
            None
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::MacosAarch64 => "macos-aarch64",
            Self::MacosX86_64 => "macos-x86_64",
            Self::LinuxX86_64 => "linux-x86_64",
            Self::WindowsX86_64 => "windows-x86_64",
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Platform {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL.into_iter().find(|p| p.as_str() == text).ok_or_else(|| format!("not a platform: {text}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_with_or_without_a_v() {
        assert_eq!("0.15.2".parse(), Ok(Version::new(0, 15, 2)));
        assert_eq!("v1.0.10".parse(), Ok(Version::new(1, 0, 10)));
        assert_eq!(Version::new(2, 3, 4).to_string(), "2.3.4");
    }

    #[test]
    fn refuses_what_is_not_three_numbers() {
        for bad in ["", "1.2", "1.2.3.4", "1..3", "1.2.x", "1.2.3-beta", "+1.2.3", "vv1.2.3"] {
            assert!(bad.parse::<Version>().is_err(), "{bad}");
        }
    }

    #[test]
    fn orders_numerically() {
        let v = |s: &str| s.parse::<Version>().ok();
        assert!(v("0.9.0") < v("0.10.0"));
        assert!(v("1.0.0") > v("0.99.99"));
        assert!(v("1.2.3") < v("1.2.10"));
        assert_eq!(v("1.2.3"), v("v1.2.3"));
    }

    #[test]
    fn this_build_has_a_version() {
        assert_eq!(Version::current().to_string(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn platforms_round_trip_their_names() {
        for platform in Platform::ALL {
            assert_eq!(platform.as_str().parse(), Ok(platform));
        }
        assert!("macos".parse::<Platform>().is_err());
    }

    #[test]
    fn serializes_as_a_string() {
        let json = serde_json::to_string(&Version::new(1, 2, 3)).unwrap_or_default();
        assert_eq!(json, "\"1.2.3\"");
        assert_eq!(serde_json::from_str::<Version>("\"1.2.3\"").ok(), Some(Version::new(1, 2, 3)));
        assert!(serde_json::from_str::<Version>("\"1.2\"").is_err());
    }
}
