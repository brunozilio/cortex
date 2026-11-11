use std::path::PathBuf;
use std::time::SystemTime;

use serde_json::{Value, json};

use super::{Access, Outcome, Tool, Workspace, field, required, shown};
use crate::model::ToolSpec;

const LIMIT: usize = 100;

#[derive(Debug)]
pub struct Glob;

impl Tool for Glob {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "Glob".into(),
            description: crate::tools::describe("Glob", &serde_json::json!({ "LIMIT": LIMIT })),
            schema: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "The glob to match paths against." },
                    "path": { "type": "string", "description": "The directory to search; the working directory if omitted." }
                },
                "required": ["pattern"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
    }

    fn reaches_outside(&self, input: &Value, workspace: &Workspace) -> Option<std::path::PathBuf> {
        workspace.outside(field(input, "path"))
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "pattern").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let pattern = match required(input, "pattern") {
            Ok(pattern) => pattern,
            Err(error) => return error,
        };
        let root = field(input, "path").map_or_else(|| workspace.cwd.clone(), |path| workspace.resolve(path));
        let matcher = match globset::GlobBuilder::new(pattern).literal_separator(true).build() {
            Ok(glob) => glob.compile_matcher(),
            Err(error) => return Outcome::error(format!("Invalid pattern: {error}")),
        };
        let mut found: Vec<(SystemTime, PathBuf)> = ignore::WalkBuilder::new(&root)
            .hidden(false)
            .filter_entry(|entry| entry.file_name() != ".git")
            .build()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
            .filter(|entry| entry.path().strip_prefix(&root).is_ok_and(|relative| matcher.is_match(relative)))
            .map(|entry| {
                let modified =
                    entry.metadata().ok().and_then(|meta| meta.modified().ok()).unwrap_or(SystemTime::UNIX_EPOCH);
                (modified, entry.into_path())
            })
            .collect();
        if found.is_empty() {
            return Outcome::ok("No files found.");
        }
        found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        let mut out: Vec<String> = found.iter().take(LIMIT).map(|(_, path)| shown(path, workspace)).collect();
        if found.len() > LIMIT {
            out.push(format!("… and {} more; narrow the pattern.", found.len() - LIMIT));
        }
        Outcome::ok(out.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_paths_and_skips_ignored_files() {
        let dir = tempfile::tempdir().expect("dir");
        let root = dir.path();
        std::fs::create_dir_all(root.join("src/deep")).expect("dirs");
        std::fs::create_dir_all(root.join("target")).expect("dirs");
        std::fs::create_dir(root.join(".git")).expect("git dir");
        for file in ["src/a.rs", "src/deep/b.rs", "src/c.md", "target/d.rs"] {
            std::fs::write(root.join(file), "").expect("write");
        }
        std::fs::write(root.join(".gitignore"), "target/\n").expect("write");
        let workspace = Workspace::new(root);

        let outcome = Glob.run(&json!({"pattern": "**/*.rs"}), &workspace);
        let normalized = outcome.content.replace('\\', "/");
        let mut paths: Vec<&str> = normalized.lines().collect();
        paths.sort_unstable();
        assert_eq!(paths, vec!["src/a.rs", "src/deep/b.rs"]);

        let shallow = Glob.run(&json!({"pattern": "*.rs", "path": "src"}), &workspace);
        assert_eq!(shallow.content.replace('\\', "/"), "src/a.rs");
        assert_eq!(Glob.run(&json!({"pattern": "*.py"}), &workspace).content, "No files found.");
    }
}
