//! Lays an installed plugin out as a Claude Code plugin folder
//! (`.claude-plugin/plugin.json`, `skills/`, `agents/`), which a run loads
//! with `--plugin-dir`; Claude Code then names its skills `<plugin>:<name>`.
//! The manifest is kept beside it, so the folder loads back as a package.
//! MCP servers are not written here: they carry secrets and go per run.

use std::fs;
use std::path::Path;

use serde_json::json;

use crate::error::PluginError;
use crate::manifest::{MANIFEST_FILE, PluginManifest};
use crate::package::{PluginPackage, file_path};

pub fn claude_plugin_json(manifest: &PluginManifest) -> String {
    let value = json!({
        "name": manifest.id.as_str(),
        "version": manifest.version,
        "description": manifest.description,
    });
    serde_json::to_string_pretty(&value).unwrap_or_default()
}

pub fn materialize(package: &PluginPackage, dir: &Path) -> Result<(), PluginError> {
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    fs::create_dir_all(dir.join(".claude-plugin"))?;
    fs::write(dir.join(".claude-plugin").join("plugin.json"), claude_plugin_json(&package.manifest))?;
    let manifest = serde_json::to_string_pretty(&package.manifest).map_err(|e| PluginError::Io(e.to_string()))?;
    fs::write(dir.join(MANIFEST_FILE), manifest)?;
    for file in &package.files {
        let path = file_path(dir, file);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, &file.bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use superfik_core::PluginOrigin;

    use super::*;
    use crate::package::PackageFile;

    #[test]
    fn writes_a_claude_code_plugin_folder() {
        let manifest = PluginManifest::parse(r#"{"id":"linear","name":"Linear","version":"2.1"}"#).expect("manifest");
        let files =
            vec![PackageFile { path: "skills/triage/SKILL.md".into(), bytes: b"---\nname: triage\n---".to_vec() }];
        let package = PluginPackage::new(manifest, PluginOrigin::Catalog, files).expect("package");
        let dir = tempfile::tempdir().expect("dir");
        let target = dir.path().join("linear");
        fs::create_dir_all(&target).expect("mkdir");
        fs::write(target.join("stale.md"), "old").expect("write");

        materialize(&package, &target).expect("materialize");
        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(target.join(".claude-plugin/plugin.json")).expect("read"))
                .expect("json");
        assert_eq!(json["name"], "linear");
        assert_eq!(json["version"], "2.1");
        assert!(target.join("skills/triage/SKILL.md").is_file());
        assert!(!target.join("stale.md").exists());
        let back = PluginPackage::load(&target).expect("loads back");
        assert_eq!((back.manifest, back.files.len()), (package.manifest, 1));
    }
}
