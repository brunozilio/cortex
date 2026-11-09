//! Plugins shipped with Superfik: manifests and files under
//! `crates/superfik-plugins/catalog/<id>/`, embedded at build time.

use superfik_core::PluginOrigin;

use crate::manifest::PluginManifest;
use crate::package::{PackageFile, PluginPackage};

struct Entry {
    manifest: &'static str,
    files: &'static [(&'static str, &'static str)],
}

macro_rules! entry {
    ($id:literal $(, $file:literal)*) => {
        Entry {
            manifest: include_str!(concat!("../catalog/", $id, "/superfik-plugin.json")),
            files: &[$(($file, include_str!(concat!("../catalog/", $id, "/", $file)))),*],
        }
    };
}

const ENTRIES: &[Entry] = &[
    entry!("openai"),
    entry!("openai-subscription"),
    entry!("claude-subscription"),
    entry!("anthropic"),
    entry!("whatsapp"),
    entry!("whatsapp-business"),
    entry!("instagram"),
    entry!("x"),
    entry!("linkedin"),
    entry!("email"),
    entry!("github"),
    entry!("linear", "skills/linear-workflow/SKILL.md"),
    entry!("notion"),
    entry!("atlassian"),
    entry!("intercom"),
    entry!("sentry"),
    entry!("stripe"),
    entry!("postgres"),
    entry!("context7"),
    entry!("firecrawl"),
    entry!("playwright"),
    entry!("bitwarden"),
];

pub fn packages() -> Vec<PluginPackage> {
    ENTRIES
        .iter()
        .filter_map(|entry| match load(entry) {
            Ok(package) => Some(package),
            Err(error) => {
                tracing::error!(%error, "broken catalog plugin");
                None
            }
        })
        .collect()
}

fn load(entry: &Entry) -> Result<PluginPackage, crate::PluginError> {
    let manifest = PluginManifest::parse(entry.manifest)?;
    let files = entry
        .files
        .iter()
        .map(|(path, text)| PackageFile { path: (*path).to_owned(), bytes: text.as_bytes().to_vec() })
        .collect();
    PluginPackage::new(manifest, PluginOrigin::Catalog, files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalog_plugin_is_valid_and_unique() {
        let packages = packages();
        assert_eq!(packages.len(), ENTRIES.len());
        let mut ids: Vec<&str> = packages.iter().map(PluginPackage::id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), packages.len());
        let linear = packages.iter().find(|p| p.id() == "linear").expect("linear");
        assert_eq!(linear.skills()[0].name, "linear:linear-workflow");
    }

    #[test]
    fn the_summary_says_which_plugins_are_vaults() {
        let vault_of = |id: &str| {
            let package = packages().into_iter().find(|p| p.id() == id).expect("in catalog");
            package.summary(false).vault
        };
        assert_eq!(vault_of("bitwarden"), Some(superfik_core::VaultKind::Bitwarden));
        assert_eq!(vault_of("linear"), None);
    }
}
