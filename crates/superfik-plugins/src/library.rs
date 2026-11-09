//! The organization's library (`superfik_core::library`) as a plugin
//! package, laid out like any installed plugin so runs get its skills and
//! agents; and reading a member's own skill or agent to publish it.

use std::path::{Path, PathBuf};

use serde_json::json;
use superfik_core::PluginOrigin;
use superfik_core::library::{LIBRARY_PLUGIN, LibraryItem, LibraryKind, valid_name};

use crate::error::PluginError;
use crate::manifest::PluginManifest;
use crate::package::{PackageFile, PluginPackage};

pub fn package(items: &[LibraryItem], dir: &Path) -> Result<PluginPackage, PluginError> {
    let manifest = json!({
        "id": LIBRARY_PLUGIN,
        "name": "Organization",
        "version": "1",
        "description": "Skills and agents your organization published.",
    });
    let manifest = PluginManifest::parse(&manifest.to_string())?;
    let files = items
        .iter()
        .filter(|item| valid_name(&item.name))
        .map(|item| PackageFile { path: file_of(item.kind, &item.name), bytes: item.body.clone().into_bytes() })
        .collect();
    PluginPackage::new(manifest, PluginOrigin::Local(dir.to_path_buf()), files)
}

fn file_of(kind: LibraryKind, name: &str) -> String {
    match kind {
        LibraryKind::Skill => format!("skills/{name}/SKILL.md"),
        LibraryKind::Agent => format!("agents/{name}.md"),
    }
}

fn candidates<'a>(
    kind: LibraryKind,
    name: &str,
    repo: &'a Path,
    home: Option<&Path>,
) -> Vec<(PathBuf, Option<&'a Path>)> {
    let file = file_of(kind, name);
    let mut found = vec![(repo.join(".claude").join(&file), Some(repo))];
    found.extend(home.map(|home| (home.join(&file), None)));
    found
}

pub fn read_own(name: &str, repo: &Path, home: Option<&Path>) -> Option<(LibraryKind, String)> {
    if !valid_name(name) {
        return None;
    }
    [LibraryKind::Skill, LibraryKind::Agent].into_iter().find_map(|kind| {
        candidates(kind, name, repo, home)
            .iter()
            .find_map(|(path, confined)| read_within(path, *confined))
            .map(|body| (kind, body))
    })
}

fn read_within(path: &Path, root: Option<&Path>) -> Option<String> {
    let real = path.canonicalize().ok()?;
    if let Some(root) = root
        && !real.starts_with(root.canonicalize().ok()?)
    {
        return None;
    }
    if !std::fs::metadata(&real).ok()?.is_file() {
        return None;
    }
    std::fs::read_to_string(real).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: LibraryKind, name: &str) -> LibraryItem {
        LibraryItem {
            kind,
            name: name.into(),
            body: format!("---\nname: {name}\n---\nDo it."),
            author_name: "Olga".into(),
        }
    }

    #[test]
    fn the_library_is_a_plugin_named_org() {
        let items = [
            item(LibraryKind::Skill, "release"),
            item(LibraryKind::Agent, "reviewer"),
            item(LibraryKind::Skill, "../x"),
        ];
        let package = package(&items, Path::new("/p/org")).expect("package");
        assert_eq!(package.id(), "org");
        let paths: Vec<&str> = package.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["skills/release/SKILL.md", "agents/reviewer.md"]);
    }

    #[test]
    fn a_members_own_skill_is_found_in_the_project_then_home() {
        let repo = tempfile::tempdir().expect("repo");
        let home = tempfile::tempdir().expect("home");
        std::fs::create_dir_all(home.path().join("skills/motion")).expect("mkdir");
        std::fs::write(home.path().join("skills/motion/SKILL.md"), "home").expect("write");
        assert_eq!(read_own("motion", repo.path(), Some(home.path())), Some((LibraryKind::Skill, "home".into())));
        std::fs::create_dir_all(repo.path().join(".claude/skills/motion")).expect("mkdir");
        std::fs::write(repo.path().join(".claude/skills/motion/SKILL.md"), "project").expect("write");
        assert_eq!(read_own("motion", repo.path(), Some(home.path())), Some((LibraryKind::Skill, "project".into())));
        std::fs::create_dir_all(repo.path().join(".claude/agents")).expect("mkdir");
        std::fs::write(repo.path().join(".claude/agents/reviewer.md"), "agent").expect("write");
        assert_eq!(read_own("reviewer", repo.path(), None), Some((LibraryKind::Agent, "agent".into())));
        assert_eq!(read_own("missing", repo.path(), None), None);
        assert_eq!(read_own("../etc", repo.path(), None), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_project_file_that_links_outside_the_project_is_not_read() {
        use std::os::unix::fs::symlink;
        let repo = tempfile::tempdir().expect("repo");
        let outside = tempfile::tempdir().expect("outside");
        std::fs::write(outside.path().join("secret"), "server secret").expect("write");
        std::fs::create_dir_all(repo.path().join(".claude/agents")).expect("mkdir");
        symlink(outside.path().join("secret"), repo.path().join(".claude/agents/leak.md")).expect("link");
        assert_eq!(read_own("leak", repo.path(), None), None, "a file linking out");

        std::fs::create_dir_all(outside.path().join("skills/grab")).expect("mkdir");
        std::fs::write(outside.path().join("skills/grab/SKILL.md"), "server secret").expect("write");
        std::fs::remove_dir_all(repo.path().join(".claude")).expect("rm");
        symlink(outside.path(), repo.path().join(".claude")).expect("link");
        assert_eq!(read_own("grab", repo.path(), None), None, "a folder linking out");

        std::fs::remove_file(repo.path().join(".claude")).expect("rm");
        std::fs::create_dir_all(repo.path().join(".claude/agents")).expect("mkdir");
        std::fs::write(repo.path().join("reviewer.md"), "kept in the project").expect("write");
        symlink(repo.path().join("reviewer.md"), repo.path().join(".claude/agents/reviewer.md")).expect("link");
        assert_eq!(
            read_own("reviewer", repo.path(), None),
            Some((LibraryKind::Agent, "kept in the project".into())),
            "a link that stays in the project is fine"
        );
    }
}
