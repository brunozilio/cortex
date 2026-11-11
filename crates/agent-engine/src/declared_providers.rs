use std::path::{Path, PathBuf};

use domain::ModelProvider;
use llm_providers::declared::{Declared, Shaper, register_shaped};

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
    if crate::providers::ALL.iter().any(|known| known.id() == declared.id) {
        return broken(format!("{}: a built-in provider already has this id", declared.id));
    }
    if approved.is_some_and(|approved| !approved.contains(&declared.id)) {
        return Loaded::Unapproved { path: path.to_owned(), id: declared.id };
    }
    let shaper = shapes(&source).then(|| shaper(shown, source, path));
    register_shaped(declared, shaper).map_or_else(|error| broken(error.to_string()), Loaded::Ready)
}

fn shapes(source: &str) -> bool {
    ["export function shape", "export async function shape", "export const shape"]
        .iter()
        .any(|start| source.contains(start))
}

fn shaper(shown: String, source: String, path: &Path) -> Shaper {
    let cwd = path.parent().map(Path::to_path_buf).unwrap_or_default();
    Box::new(move |body: &mut serde_json::Value| {
        let none = crate::tools::script_tool::ScriptAccess { files: false, processes: false, network: false };
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let host = std::sync::Arc::new(crate::tools::script_io::StandaloneHost {
            cwd: cwd.clone(),
            access: none,
            stop: std::sync::Arc::clone(&stop),
        });
        let limit = Some(std::time::Duration::from_secs(2));
        let shaped = script::run_within(
            &shown,
            &source,
            "shape",
            std::slice::from_ref(body),
            &serde_json::Value::Null,
            host,
            stop,
            limit,
        )?;
        match shaped {
            serde_json::Value::Null => Ok(()),
            serde_json::Value::Object(_) => {
                *body = shaped;
                Ok(())
            }
            other => Err(format!("shape must return the body object, not {other}")),
        }
    })
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
        assert!(crate::providers::find(ModelProvider::named("loaded-theirs")).is_none());

        let approved = load(Some(home.path()), project.path(), &["loaded-theirs".to_owned()]);
        assert_eq!(approved[1], Loaded::Ready(ModelProvider::named("loaded-theirs")));
    }

    #[test]
    fn a_provider_that_exports_shape_rewrites_each_body() {
        let home = tempfile::tempdir().expect("home");
        let folder = home.path().join(brand::DIR).join("providers");
        std::fs::create_dir_all(&folder).expect("folder");
        let source = r#"export default { id: "loaded-shaped", name: "Shaped", wire: "chat", base: "https://s.example/v1" };
export function shape(body) { return { ...body, temperature: 0.1, model: body.model.replace("fast", "fast-v2") }; }"#;
        std::fs::write(folder.join("shaped.js"), source).expect("write");
        let loaded = load(Some(home.path()), &home.path().join("elsewhere"), &[]);
        assert_eq!(loaded, [Loaded::Ready(ModelProvider::named("loaded-shaped"))]);
        let provider = crate::providers::get(ModelProvider::named("loaded-shaped")).expect("found");
        let mut body = serde_json::json!({"model": "fast", "messages": []});
        provider.shape(None, &mut body, "low").expect("shaped");
        assert_eq!(body, serde_json::json!({"model": "fast-v2", "messages": [], "temperature": 0.1}));
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
