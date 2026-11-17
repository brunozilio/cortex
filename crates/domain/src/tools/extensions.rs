use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::trust::Trust;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extensions {
    #[serde(default)]
    pub dirs: Vec<PathBuf>,
    #[serde(default)]
    pub mcp_config: Option<PathBuf>,
    #[serde(default)]
    pub plugins: Vec<PathBuf>,
    #[serde(default)]
    pub home: Option<PathBuf>,
    #[serde(default)]
    pub trust: Trust,
}

impl Extensions {
    pub fn is_empty(&self) -> bool {
        self.dirs.is_empty()
            && self.mcp_config.is_none()
            && self.plugins.is_empty()
            && self.home.is_none()
            && self.trust == Trust::Untrusted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_is_untrusted_until_the_host_says_otherwise_and_trust_alone_is_worth_carrying() {
        let read: Extensions = serde_json::from_str("{}").expect("read");
        assert_eq!(read.trust, Trust::Untrusted);
        assert!(read.is_empty());
        assert!(!Extensions { trust: Trust::Trusted, ..Extensions::default() }.is_empty());
    }
}
