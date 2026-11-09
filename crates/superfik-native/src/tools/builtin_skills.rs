//! The skills Superfik ships (`crates/superfik-native/skills/`), embedded
//! at build time. A skill reads its own files by path, so they are written
//! once per build to `~/.superfik/builtin-skills/<hash>/` and found after
//! the project's and the user's skills: one of theirs with the same name
//! wins.

use std::path::{Path, PathBuf};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/builtin_skills.rs"));
}

pub fn dir(home: &Path) -> Option<PathBuf> {
    let root = home.join(".superfik/builtin-skills");
    write(&root, embedded::FILES, embedded::HASH)
}

fn write(root: &Path, files: &[(&str, &[u8])], hash: u64) -> Option<PathBuf> {
    if files.is_empty() {
        return None;
    }
    let target = root.join(format!("{hash:016x}"));
    let marker = target.join(".complete");
    if marker.is_file() {
        return Some(target);
    }
    let staging = root.join(format!(".{hash:016x}-{}", std::process::id()));
    let staged = (|| -> std::io::Result<()> {
        for (relative, bytes) in files {
            if Path::new(relative).components().any(|part| !matches!(part, std::path::Component::Normal(_))) {
                continue;
            }
            let path = staging.join(relative);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, bytes)?;
        }
        std::fs::write(staging.join(".complete"), b"")
    })();
    if staged.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
        return None;
    }
    if std::fs::rename(&staging, &target).is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    marker.is_file().then_some(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_is_written_once_and_whole() {
        let home = tempfile::tempdir().expect("home");
        let files: &[(&str, &[u8])] = &[
            ("review/SKILL.md", b"---\nname: review\n---\nLook."),
            ("review/levels/low.md", b"Few."),
            ("../escape", b"x"),
        ];
        let dir = write(home.path(), files, 7).expect("written");
        assert_eq!(std::fs::read_to_string(dir.join("review/levels/low.md")).expect("read"), "Few.");
        assert!(!home.path().join("escape").exists(), "nothing lands outside the copy");
        std::fs::write(dir.join("review/SKILL.md"), "kept").expect("write");
        assert_eq!(write(home.path(), files, 7), Some(dir.clone()), "an existing copy is reused");
        assert_eq!(std::fs::read_to_string(dir.join("review/SKILL.md")).expect("read"), "kept");
        assert!(write(home.path(), &[], 8).is_none());
    }

    #[test]
    fn every_shipped_skill_speaks_as_superfik() {
        for (path, bytes) in embedded::FILES {
            if path.ends_with("LICENSE.txt") || !path.ends_with(".md") {
                continue;
            }
            let text = String::from_utf8_lossy(bytes);
            for other in ["Claude", "Anthropic", "claude.ai"] {
                assert!(!text.contains(other), "{path} names {other}");
            }
        }
    }
}
