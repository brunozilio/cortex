use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRun {
    pub command: String,
    pub folder: PathBuf,
    pub env: Vec<(String, String)>,
}

impl TestRun {
    pub fn label(&self, project: &Path) -> String {
        match self.folder.strip_prefix(project) {
            Ok(inner) if !inner.as_os_str().is_empty() => format!("{} in {}", self.command, inner.display()),
            _ => self.command.clone(),
        }
    }
}

pub fn tests_for(
    path: &Path,
    project: &Path,
    exists: &dyn Fn(&Path) -> bool,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Option<TestRun> {
    let extension = path.extension()?.to_str()?;
    let folder = path.parent()?;
    let stem = path.file_stem()?.to_str()?;
    match extension {
        "rs" => rust(folder, project, read),
        "go" => Some(TestRun { command: "go test .".into(), folder: folder.to_path_buf(), env: Vec::new() }),
        "py" => python(path, folder, stem, project, exists),
        "ts" | "tsx" | "js" | "jsx" | "mjs" => javascript(path, folder, stem, extension, project, exists),
        _ => None,
    }
}

fn rust(folder: &Path, project: &Path, read: &dyn Fn(&Path) -> Option<String>) -> Option<TestRun> {
    let crate_folder = folder.ancestors().take_while(|at| at.starts_with(project)).find_map(|at| {
        let manifest = read(&at.join("Cargo.toml"))?;
        package_name(&manifest).map(|name| (at, name))
    });
    let (at, name) = crate_folder?;
    let target = project.join("target").join("agent-tests");
    Some(TestRun {
        command: format!("cargo test -q -p {name}"),
        folder: at.to_path_buf(),
        env: vec![("CARGO_TARGET_DIR".into(), target.display().to_string())],
    })
}

fn package_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_package = line == "[package]";
        } else if in_package && let Some(value) = line.strip_prefix("name") {
            let value = value.trim_start().strip_prefix('=')?.trim().trim_matches('"');
            return (!value.is_empty()).then(|| value.to_owned());
        }
    }
    None
}

fn python(path: &Path, folder: &Path, stem: &str, project: &Path, exists: &dyn Fn(&Path) -> bool) -> Option<TestRun> {
    let named = format!("test_{stem}.py");
    let candidates = if stem.starts_with("test_") {
        vec![path.to_path_buf()]
    } else {
        vec![folder.join(&named), folder.join("tests").join(&named), project.join("tests").join(&named)]
    };
    let found = candidates.into_iter().find(|candidate| exists(candidate))?;
    Some(TestRun {
        command: format!("python -m pytest -q '{}'", found.display()),
        folder: project.to_path_buf(),
        env: Vec::new(),
    })
}

fn javascript(
    path: &Path,
    folder: &Path,
    stem: &str,
    extension: &str,
    project: &Path,
    exists: &dyn Fn(&Path) -> bool,
) -> Option<TestRun> {
    let is_test = stem.ends_with(".test") || stem.ends_with(".spec");
    let candidates = if is_test {
        vec![path.to_path_buf()]
    } else {
        vec![folder.join(format!("{stem}.test.{extension}")), folder.join(format!("{stem}.spec.{extension}"))]
    };
    let found = candidates.into_iter().find(|candidate| exists(candidate))?;
    exists(&project.join("package.json")).then(|| TestRun {
        command: format!("npm test --silent -- '{}'", found.display()),
        folder: project.to_path_buf(),
        env: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn tree(files: &[(&str, &str)]) -> BTreeMap<PathBuf, String> {
        files.iter().map(|(path, text)| (PathBuf::from(path), (*text).to_owned())).collect()
    }

    fn planned(files: &BTreeMap<PathBuf, String>, path: &str) -> Option<TestRun> {
        tests_for(Path::new(path), Path::new("/repo"), &|at| files.contains_key(at), &|at| files.get(at).cloned())
    }

    #[test]
    fn a_rust_file_runs_its_crate_s_tests_in_a_target_of_their_own() {
        let files = tree(&[
            ("/repo/Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
            (
                "/repo/crates/core/Cargo.toml",
                "[package]\nname = \"core-lib\"\nversion = \"1.0.0\"\n[dependencies]\nname = \"x\"\n",
            ),
        ]);

        let run = planned(&files, "/repo/crates/core/src/parse/mod.rs").expect("planned");

        assert_eq!(run.command, "cargo test -q -p core-lib");
        assert_eq!(run.folder, PathBuf::from("/repo/crates/core"));
        assert_eq!(run.env, [("CARGO_TARGET_DIR".to_owned(), "/repo/target/agent-tests".to_owned())]);
    }

    #[test]
    fn python_and_javascript_files_run_the_test_file_beside_them() {
        let files =
            tree(&[("/repo/tests/test_parser.py", ""), ("/repo/package.json", "{}"), ("/repo/src/cart.test.ts", "")]);

        assert_eq!(
            planned(&files, "/repo/app/parser.py").map(|run| run.command),
            Some("python -m pytest -q '/repo/tests/test_parser.py'".into())
        );
        assert_eq!(
            planned(&files, "/repo/src/cart.ts").map(|run| run.command),
            Some("npm test --silent -- '/repo/src/cart.test.ts'".into())
        );
        assert_eq!(
            planned(&files, "/repo/src/cart.test.ts").map(|run| run.command),
            Some("npm test --silent -- '/repo/src/cart.test.ts'".into())
        );
        assert_eq!(planned(&files, "/repo/app/other.py"), None, "no test file, nothing to run");
        assert_eq!(planned(&files, "/repo/README.md"), None);
    }

    #[test]
    fn a_go_file_tests_its_package() {
        let run = planned(&tree(&[]), "/repo/pkg/store/db.go").expect("planned");
        assert_eq!((run.command.as_str(), run.folder), ("go test .", PathBuf::from("/repo/pkg/store")));
    }

    #[test]
    fn the_label_names_the_folder_so_two_packages_running_the_same_command_stay_apart() {
        let project = Path::new("/repo");
        let store = planned(&tree(&[]), "/repo/pkg/store/db.go").expect("store");
        let api = planned(&tree(&[]), "/repo/pkg/api/server.go").expect("api");
        let root = planned(&tree(&[]), "/repo/main.go").expect("root");

        assert_eq!(store.label(project), "go test . in pkg/store");
        assert_eq!(api.label(project), "go test . in pkg/api");
        assert_eq!(root.label(project), "go test .", "the project folder needs no name");
    }
}
