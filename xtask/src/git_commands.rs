use std::path::{Path, PathBuf};

const HELPER: &str = "crates/platform/src/git.rs";

const STARTS_GIT: [&str; 3] = ["Command::new(\"git\")", "command(\"git\")", "OsStr::new(\"git\")"];

pub fn findings(root: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    crate::env_vars::rust_files(&root.join("crates"), &mut files);
    let mut found = Vec::new();
    for file in files {
        let shown = shown(root, &file);
        let tests = file.file_name().is_some_and(|name| name.to_string_lossy().starts_with("tests_"));
        if shown != HELPER && !tests && starts_git(&std::fs::read_to_string(&file)?) {
            found.push(format!("git-command {shown}: start git through platform::git::command"));
        }
    }
    Ok(found)
}

fn shown(root: &Path, file: &Path) -> String {
    let relative: PathBuf = file.strip_prefix(root).unwrap_or(file).components().collect();
    relative.to_string_lossy().replace('\\', "/")
}

fn starts_git(source: &str) -> bool {
    let code = source.split("#[cfg(test)]").next().unwrap_or_default();
    let code: String = code.chars().filter(|letter| !letter.is_whitespace()).collect();
    STARTS_GIT.iter().any(|call| code.contains(call))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_started_outside_the_helper_is_found_and_tests_are_left_alone() {
        assert!(starts_git("fn a() { let c = std::process::Command::new(\"git\"); }"));
        assert!(starts_git("fn a() { platform::command( \"git\" ).arg(\"status\"); }"));
        assert!(starts_git("fn a() { run(OsStr::new(\"git\"), cwd) }"));
        assert!(!starts_git("fn a() { platform::git::command(cwd).arg(\"status\"); }"));
        assert!(!starts_git("fn a() {}\n#[cfg(test)]\nmod tests { fn t() { Command::new(\"git\"); } }"));
    }

    #[test]
    fn the_workspace_starts_git_only_through_the_helper() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root");
        assert_eq!(findings(root).expect("scanned"), Vec::<String>::new());
    }
}
