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

const ENTRIES: &[Entry] =
    &[entry!("openai"), entry!("openai-subscription"), entry!("claude-subscription"), entry!("anthropic")];

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
    }
}
