use std::path::Path;

const MOST_ENTRIES: usize = 30;

pub fn top_level(cwd: &Path) -> Option<Vec<String>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(cwd).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if entries.len() == MOST_ENTRIES {
            return None;
        }
        entries.push(if entry.path().is_dir() { format!("{name}/") } else { name });
    }
    entries.sort();
    Some(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_hidden_entries_leave_it_empty() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::create_dir(dir.path().join(".git")).expect("hidden folder");
        std::fs::write(dir.path().join(".env"), "").expect("hidden file");
        assert_eq!(top_level(dir.path()), Some(Vec::new()));
    }

    #[test]
    fn names_visible_files_and_marks_folders() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("README.md"), "").expect("file");
        std::fs::create_dir(dir.path().join("src")).expect("folder");
        std::fs::create_dir(dir.path().join(".git")).expect("hidden folder");
        assert_eq!(top_level(dir.path()), Some(vec!["README.md".to_owned(), "src/".to_owned()]));
    }

    #[test]
    fn too_many_entries_are_not_listed() {
        let dir = tempfile::tempdir().expect("dir");
        for index in 0..=MOST_ENTRIES {
            std::fs::write(dir.path().join(format!("f{index}")), "").expect("file");
        }
        assert_eq!(top_level(dir.path()), None);
    }

    #[test]
    fn a_missing_directory_is_not_listed() {
        assert_eq!(top_level(Path::new("/does/not/exist")), None);
    }
}
