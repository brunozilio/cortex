use std::path::{Path, PathBuf};

use agent_engine::trust::Gate;
use domain::features::Features;
use domain::memory::{Memory, team};

pub fn folder(project: &Path) -> PathBuf {
    project.join(brand::DIR).join("memory")
}

pub fn features(gate: &Gate) -> Features {
    agent_engine::prompt::settings::load(gate, &[]).map(|settings| settings.features).unwrap_or_default()
}

pub fn load(gate: &Gate) -> Vec<Memory> {
    load_if(gate.cwd(), &features(gate))
}

pub fn load_if(project: &Path, features: &Features) -> Vec<Memory> {
    if !features.on(domain::features::TEAM_MEMORY) {
        return Vec::new();
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder(project))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .collect();
    files.sort();
    files
        .iter()
        .enumerate()
        .filter_map(|(at, path)| {
            let text = std::fs::read_to_string(path).ok()?;
            let name = path.file_name()?.to_string_lossy().into_owned();
            team::parse(at, &name, &text)
        })
        .collect()
}

pub fn share(gate: &Gate, memory: &Memory) -> Result<PathBuf, String> {
    let project = gate.cwd();
    let structured = features(gate).on(domain::features::STRUCTURED_LEARNINGS);
    let by = if structured {
        team::Sharer { author: git_user(project), date: Some(agent_engine::today()), structured }
    } else {
        team::Sharer::default()
    };
    let (name, text) = team::file(memory, &by);
    let path = folder(project).join(name);
    platform::store::Document::hand_edited(&path).write_text(&text).map_err(|error| error.to_string())?;
    Ok(path)
}

fn git_user(project: &Path) -> Option<String> {
    let name = crate::git::run(project, &["config", "user.name"], crate::git::LOCAL).ok()?.trim().to_owned();
    (!name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use domain::memory::{Author, MemoryId, MemoryKind};

    use super::*;

    #[test]
    fn a_shared_memory_is_read_back_by_everyone_in_the_project() {
        let project = tempfile::tempdir().expect("project");
        let mine = Memory {
            id: MemoryId(4),
            kind: MemoryKind::Procedural,
            text: "Run make check before pushing.".into(),
            tags: vec!["ci".into()],
            created: 0,
            origin: None,
            author: Author::Agent,
            history: Vec::new(),
            uses: 0,
            last_used: 0,
            touches: vec!["ci".into()],
            credit: None,
        };

        let gate = Gate::at(project.path(), None, domain::trust::Trust::Untrusted);
        let path = share(&gate, &mine).expect("shared");
        let loaded = load(&gate);

        assert!(path.starts_with(folder(project.path())));
        assert_eq!(loaded.len(), 1);
        assert!(team::is_team(loaded[0].id));
        assert_eq!(
            (loaded[0].kind, loaded[0].text.as_str()),
            (MemoryKind::Procedural, "Run make check before pushing.")
        );
        assert_eq!(loaded[0].touches, ["ci"]);
    }
}
