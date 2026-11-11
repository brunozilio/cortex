use std::path::{Path, PathBuf};

pub fn dir(home: &Path) -> Option<PathBuf> {
    let root = home.join(concat!(brand::dir!(), "/builtin-skills"));
    let files: Vec<(&str, &[u8])> = content::builtin_files("skills/")
        .into_iter()
        .map(|(path, bytes)| (path.strip_prefix("skills/").unwrap_or(path), bytes))
        .collect();
    write(&root, &files, content::hash())
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
            match std::str::from_utf8(bytes) {
                Ok(text) => std::fs::write(path, brand::text(text))?,
                Err(_) => std::fs::write(path, bytes)?,
            }
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
            ("review/named.md", b"Ask {{NAME}} to read {{DIR}}/{{INSTRUCTIONS}}."),
            ("../escape", b"x"),
        ];
        let dir = write(home.path(), files, 7).expect("written");
        assert_eq!(std::fs::read_to_string(dir.join("review/levels/low.md")).expect("read"), "Few.");
        assert!(!home.path().join("escape").exists(), "nothing lands outside the copy");
        let named = std::fs::read_to_string(dir.join("review/named.md")).expect("read");
        assert_eq!(named, format!("Ask {} to read {}/{}.", brand::NAME, brand::DIR, brand::INSTRUCTIONS));
        std::fs::write(dir.join("review/SKILL.md"), "kept").expect("write");
        assert_eq!(write(home.path(), files, 7), Some(dir.clone()), "an existing copy is reused");
        assert_eq!(std::fs::read_to_string(dir.join("review/SKILL.md")).expect("read"), "kept");
        assert!(write(home.path(), &[], 8).is_none());
    }

    #[test]
    fn the_chat_s_review_and_init_commands_ship_as_skills() {
        for name in ["init", "review", "code-review", "security-review"] {
            let path = format!("{name}/SKILL.md");
            let skill = content::builtin(&format!("skills/{path}"));
            let text = String::from_utf8_lossy(skill.unwrap_or_default());
            assert!(text.contains(&format!("name: {name}\n")), "{path}: {text}");
        }
    }

    #[test]
    fn the_agent_may_load_the_configure_skill_to_set_up_model_fallback() {
        let skill = content::builtin("skills/configure/SKILL.md").expect("configure ships");
        let text = String::from_utf8_lossy(skill);
        assert!(text.contains("name: configure\n"), "{text}");
        assert!(!text.contains("disable-model-invocation"), "the model may load it on its own");
        for key in [
            "fallbackModel",
            "ModelRequest",
            "ModelError",
            "provider:model",
            "settings.local.json",
            "mcp.json",
            "{{COMMAND}} mcp add",
            "\"type\": \"http\"",
            "enabledMcpServers",
            "{{COMMAND}} mcp login",
        ] {
            assert!(text.contains(key), "configure explains {key}");
        }
    }

    #[test]
    fn every_shipped_skill_speaks_as_brand() {
        for (path, bytes) in content::builtin_files("skills/") {
            if path.ends_with("LICENSE.txt") || !path.ends_with(".md") {
                continue;
            }
            let text = String::from_utf8_lossy(bytes);
            assert!(!brand::text(&text).contains("{{"), "{path}: an unknown placeholder");
            for other in ["Claude", "Anthropic", "claude.ai"] {
                assert!(!text.contains(other), "{path} names {other}");
            }
        }
    }
}
