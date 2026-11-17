use std::path::PathBuf;

const NAME: &str = "verification.md";
const MOST_BYTES: u64 = 64 * 1024;

pub fn nearest<'a>(roots: impl DoubleEndedIterator<Item = &'a PathBuf>) -> Option<String> {
    roots.rev().map(|root| root.join(NAME)).find_map(|path| {
        std::fs::metadata(&path).ok().filter(|meta| meta.is_file() && meta.len() <= MOST_BYTES)?;
        let text = std::fs::read_to_string(&path).ok()?;
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_nearest_project_folder_s_criteria_are_read_and_the_owner_s_are_not() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        let nested = repo.path().join("app");
        for (root, text) in [
            (home.path().join(brand::DIR), "the owner's"),
            (repo.path().join(brand::DIR), "the repository's"),
            (nested.join(brand::DIR), "\n  the app's  \n"),
        ] {
            std::fs::create_dir_all(&root).expect("dir");
            std::fs::write(root.join("verification.md"), text).expect("write");
        }

        let settings = super::super::settings::load_trusted(&nested, Some(home.path()), &[]).expect("load");
        assert_eq!(settings.project_criteria.as_deref(), Some("the app's"));

        std::fs::remove_file(nested.join(brand::DIR).join("verification.md")).expect("remove");
        let settings = super::super::settings::load_trusted(&nested, Some(home.path()), &[]).expect("load");
        assert_eq!(settings.project_criteria.as_deref(), Some("the repository's"));

        let lone = tempfile::tempdir().expect("lone");
        let settings = super::super::settings::load_trusted(lone.path(), Some(home.path()), &[]).expect("load");
        assert_eq!(settings.project_criteria, None, "the owner's own file is not a project's criteria");
    }
}
