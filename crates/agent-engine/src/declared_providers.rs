use std::path::{Path, PathBuf};

use domain::ModelProvider;
use llm_providers::declared::{Declared, register};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loaded {
    Ready(ModelProvider),
    Unapproved { path: PathBuf, id: String },
    Broken { path: PathBuf, why: String },
}

pub fn load(home: Option<&Path>, cwd: &Path, approved: &[String]) -> Vec<Loaded> {
    let user = home.map(|home| home.join(brand::DIR).join("providers"));
    let project = cwd.join(brand::DIR).join("providers");
    let mut loaded: Vec<Loaded> = user.iter().flat_map(|dir| scripts(dir)).map(|path| one(&path, None)).collect();
    if user.as_ref().is_none_or(|user| !same(user, &project)) {
        loaded.extend(scripts(&project).into_iter().map(|path| one(&path, Some(approved))));
    }
    loaded
}

fn one(path: &Path, approved: Option<&[String]>) -> Loaded {
    let broken = |why: String| Loaded::Broken { path: path.to_owned(), why };
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => return broken(error.to_string()),
    };
    let shown = path.display().to_string();
    let value = match script::exported(&shown, &source, "default") {
        Ok(value) => value,
        Err(error) => return broken(error),
    };
    let declared: Declared = match serde_json::from_value(value) {
        Ok(declared) => declared,
        Err(error) => return broken(format!("the default export is not a provider: {error}")),
    };
    if approved.is_some_and(|approved| !approved.contains(&declared.id)) {
        return Loaded::Unapproved { path: path.to_owned(), id: declared.id };
    }
    register(declared).map_or_else(|error| broken(error.to_string()), Loaded::Ready)
}

fn scripts(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "js") && path.is_file())
        .collect();
    found.sort();
    found
}

fn same(a: &Path, b: &Path) -> bool {
    a.canonicalize().ok().zip(b.canonicalize().ok()).is_some_and(|(a, b)| a == b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, id: &str) {
        let folder = dir.join(brand::DIR).join("providers");
        std::fs::create_dir_all(&folder).expect("folder");
        let source = format!(
            "export default {{ id: \"{id}\", name: \"{id}\", wire: \"chat\", base: \"https://{id}.example/v1\" }};"
        );
        std::fs::write(folder.join(name), source).expect("write");
    }

    #[test]
    fn the_user_s_providers_load_and_a_project_s_wait_for_approval() {
        let home = tempfile::tempdir().expect("home");
        let project = tempfile::tempdir().expect("project");
        write(home.path(), "mine.js", "loaded-mine");
        write(project.path(), "theirs.js", "loaded-theirs");
        let loaded = load(Some(home.path()), project.path(), &[]);
        assert_eq!(loaded[0], Loaded::Ready(ModelProvider::named("loaded-mine")));
        assert!(matches!(&loaded[1], Loaded::Unapproved { id, .. } if id == "loaded-theirs"));
        assert!(llm_providers::find(ModelProvider::named("loaded-theirs")).is_none());

        let approved = load(Some(home.path()), project.path(), &["loaded-theirs".to_owned()]);
        assert_eq!(approved[1], Loaded::Ready(ModelProvider::named("loaded-theirs")));
    }

    #[test]
    fn a_broken_file_says_why_and_the_rest_still_load() {
        let home = tempfile::tempdir().expect("home");
        let folder = home.path().join(brand::DIR).join("providers");
        std::fs::create_dir_all(&folder).expect("folder");
        std::fs::write(folder.join("a.js"), "export default { id: 1 };").expect("write");
        write(home.path(), "b.js", "loaded-after-broken");
        let loaded = load(Some(home.path()), &home.path().join("elsewhere"), &[]);
        assert!(matches!(&loaded[0], Loaded::Broken { why, .. } if why.contains("not a provider")), "{loaded:?}");
        assert_eq!(loaded[1], Loaded::Ready(ModelProvider::named("loaded-after-broken")));
    }
}
