//! A plugin with its files: the manifest plus the skills and agents it
//! ships, read from the catalog or from a folder, kept in memory (they are
//! small text files).

use std::fs;
use std::path::{Component, Path, PathBuf};

use superfik_core::{CustomAgent, PluginOrigin, PluginSummary, Skill};

use crate::agents::agent_from_markdown;
use crate::error::PluginError;
use crate::manifest::{MANIFEST_FILE, PluginManifest};
use crate::skills::skill_from_markdown;

const CONTENT_DIRS: [&str; 2] = ["skills", "agents"];
const MAX_FILE: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPackage {
    pub manifest: PluginManifest,
    pub origin: PluginOrigin,
    pub files: Vec<PackageFile>,
}

impl PluginPackage {
    pub fn new(manifest: PluginManifest, origin: PluginOrigin, files: Vec<PackageFile>) -> Result<Self, PluginError> {
        if let Some(file) = files.iter().find(|file| !is_safe(&file.path)) {
            return Err(PluginError::Invalid(format!("unsafe path `{}`", file.path)));
        }
        Ok(Self { manifest, origin, files })
    }

    pub fn load(dir: &Path) -> Result<Self, PluginError> {
        let json = fs::read_to_string(dir.join(MANIFEST_FILE))
            .map_err(|error| PluginError::Io(format!("{}: {error}", dir.join(MANIFEST_FILE).display())))?;
        let manifest = PluginManifest::parse(&json)?;
        let mut files = Vec::new();
        for sub in CONTENT_DIRS {
            collect(&dir.join(sub), sub, &mut files)?;
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Self::new(manifest, PluginOrigin::Local(dir.to_path_buf()), files)
    }

    pub fn id(&self) -> &str {
        self.manifest.id.as_str()
    }

    pub fn skills(&self) -> Vec<Skill> {
        let mut skills: Vec<Skill> = self
            .files
            .iter()
            .filter_map(|file| {
                let rest = file.path.strip_prefix("skills/")?;
                let (folder, name) = rest.split_once('/')?;
                name.eq_ignore_ascii_case("skill.md").then(|| {
                    let mut skill = skill_from_markdown(folder, &String::from_utf8_lossy(&file.bytes));
                    skill.name = format!("{}:{}", self.id(), skill.name);
                    skill
                })
            })
            .collect();
        skills.sort_by(|a, b| a.name.cmp(&b.name));
        skills
    }

    pub fn agents(&self) -> Vec<CustomAgent> {
        let mut agents: Vec<CustomAgent> = self
            .files
            .iter()
            .filter_map(|file| {
                let stem = file.path.strip_prefix("agents/")?.strip_suffix(".md")?;
                (!stem.contains('/'))
                    .then(|| agent_from_markdown(stem, &String::from_utf8_lossy(&file.bytes), Some(self.id())))?
            })
            .collect();
        agents.sort_by(|a, b| a.name.cmp(&b.name));
        agents
    }

    pub fn summary(&self, installed: bool) -> PluginSummary {
        let m = &self.manifest;
        PluginSummary {
            id: m.id.clone(),
            name: m.name.clone(),
            version: m.version.clone(),
            description: m.description.clone(),
            icon: m.icon.clone(),
            category: m.category.clone(),
            accounts: m.accounts,
            auth: m.auth.kind(),
            fields: m.auth.fields().to_vec(),
            origin: self.origin.clone(),
            installed,
            skills: self.skills().iter().map(|s| crate::claude_home::bare_name(&s.name).to_owned()).collect(),
            servers: m.mcp.iter().map(|server| server.name().to_owned()).collect(),
            vault: m.vault,
            channel: m.channel,
        }
    }
}

fn is_safe(path: &str) -> bool {
    !path.is_empty() && Path::new(path).components().all(|part| matches!(part, Component::Normal(_)))
}

fn collect(dir: &Path, prefix: &str, files: &mut Vec<PackageFile>) -> Result<(), PluginError> {
    let Ok(entries) = fs::read_dir(dir) else { return Ok(()) };
    for entry in entries.flatten() {
        let kind = entry.file_type()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}/{name}");
        if kind.is_dir() {
            collect(&entry.path(), &path, files)?;
        } else if kind.is_file() {
            if entry.metadata()?.len() > MAX_FILE {
                return Err(PluginError::Invalid(format!("`{path}` is larger than 1 MB")));
            }
            files.push(PackageFile { path, bytes: fs::read(entry.path())? });
        }
    }
    Ok(())
}

pub fn file_path(root: &Path, file: &PackageFile) -> PathBuf {
    file.path.split('/').fold(root.to_path_buf(), |path, part| path.join(part))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, path: &str, text: &str) {
        let file = root.join(path);
        fs::create_dir_all(file.parent().expect("parent")).expect("mkdir");
        fs::write(file, text).expect("write");
    }

    #[test]
    fn loads_skills_and_agents_from_a_folder() {
        let dir = tempfile::tempdir().expect("dir");
        write(dir.path(), MANIFEST_FILE, r#"{"id":"linear","name":"Linear","version":"1"}"#);
        write(dir.path(), "skills/triage/SKILL.md", "---\nname: triage\ndescription: Triage issues\n---\n");
        write(dir.path(), "skills/triage/notes/extra.md", "more");
        write(dir.path(), "agents/planner.md", "---\ndescription: Plans cycles\n---\n");
        write(dir.path(), "README.md", "not copied");
        let package = PluginPackage::load(dir.path()).expect("load");
        let paths: Vec<&str> = package.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["agents/planner.md", "skills/triage/SKILL.md", "skills/triage/notes/extra.md"]);
        assert_eq!(package.skills()[0].name, "linear:triage");
        assert_eq!(package.skills()[0].description, "Triage issues");
        assert_eq!(package.agents()[0].name, "linear:planner");
        let summary = package.summary(true);
        assert_eq!(summary.skills, vec!["triage"]);
        assert_eq!(summary.origin, PluginOrigin::Local(dir.path().to_path_buf()));
    }

    #[test]
    fn missing_manifest_and_unsafe_paths_fail() {
        let dir = tempfile::tempdir().expect("dir");
        assert!(PluginPackage::load(dir.path()).is_err());
        let manifest = PluginManifest::parse(r#"{"id":"x","name":"X","version":"1"}"#).expect("manifest");
        let file = |path: &str| PackageFile { path: path.into(), bytes: Vec::new() };
        assert!(PluginPackage::new(manifest.clone(), PluginOrigin::Catalog, vec![file("../evil")]).is_err());
        assert!(PluginPackage::new(manifest.clone(), PluginOrigin::Catalog, vec![file("/etc/x")]).is_err());
        assert!(PluginPackage::new(manifest, PluginOrigin::Catalog, vec![file("skills/a/SKILL.md")]).is_ok());
    }
}
