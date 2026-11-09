//! Skill discovery: `SKILL.md` files under `<dir>/<name>/`, in the
//! project, the user's Claude home, synced skills and installed plugins.

mod frontmatter;
mod naming;

use std::fs;
use std::path::{Path, PathBuf};

use superfik_core::Skill;

use crate::claude_home::{ClaudeHome, Source, bare_name, qualified};

pub use frontmatter::{Frontmatter, parse as parse_frontmatter};
pub use naming::{skill_icon, skill_title};

pub trait SkillCatalog: Send + Sync {
    fn skills(&self, repo: &Path) -> Vec<Skill>;
}

#[derive(Debug, Clone, Default)]
pub struct SkillDiscovery {
    home: Option<ClaudeHome>,
}

impl SkillDiscovery {
    pub fn new() -> Self {
        Self { home: ClaudeHome::detect() }
    }

    pub fn with_home(root: Option<PathBuf>) -> Self {
        Self { home: root.map(ClaudeHome::new) }
    }
}

impl SkillCatalog for SkillDiscovery {
    fn skills(&self, repo: &Path) -> Vec<Skill> {
        let repo_dir = repo.join(".claude").join("skills");
        let mut sources = vec![Source { dir: repo_dir.clone(), namespace: None }];
        let home_sources = self.home.iter().flat_map(|home| home.skill_sources(repo));
        sources.extend(home_sources.filter(|source| source.dir != repo_dir));
        let mut skills: Vec<Skill> = Vec::new();
        for source in sources {
            for skill in scan_dir(&source.dir, source.namespace.as_deref()) {
                if !skills.iter().any(|known| bare_name(&known.name) == bare_name(&skill.name)) {
                    skills.push(skill);
                }
            }
        }
        skills
    }
}

fn scan_dir(dir: &Path, namespace: Option<&str>) -> Vec<Skill> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut skills: Vec<Skill> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let folder = entry.file_name().to_string_lossy().into_owned();
            let file = skill_file(&entry.path())?;
            match fs::read_to_string(&file) {
                Ok(text) => Some(namespaced(skill_from_markdown(&folder, &text), namespace)),
                Err(error) => {
                    tracing::debug!(%error, path = %file.display(), "unreadable skill");
                    None
                }
            }
        })
        .collect();
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

fn skill_file(dir: &Path) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.is_file() && path.file_name().is_some_and(|name| name.eq_ignore_ascii_case("skill.md")))
}

fn namespaced(mut skill: Skill, namespace: Option<&str>) -> Skill {
    skill.name = qualified(namespace, &skill.name);
    skill
}

pub fn skill_from_markdown(folder: &str, text: &str) -> Skill {
    let (front, body) = parse_frontmatter(text);
    let name = front.get("name").unwrap_or(folder).trim().to_owned();
    let description = front.get("description").map(str::to_owned).unwrap_or_else(|| first_paragraph(body));
    Skill { title: skill_title(&name), icon: skill_icon(&name), name, description }
}

fn first_paragraph(body: &str) -> String {
    body.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("---"))
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
mod tests;
