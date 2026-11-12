use serde_json::{Value, json};

use crate::model::ToolSpec;
use crate::tools::read_files::Freshness;
use crate::tools::{Access, Outcome, Tool, Workspace, field, required, shown};

#[derive(Debug)]
pub struct Write;

impl Tool for Write {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "Write".into(),
            description: crate::tools::describe("Write", &serde_json::Value::Null),
            schema: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "The file to write." },
                    "content": { "type": "string", "description": "The file's whole new content." }
                },
                "required": ["file_path", "content"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Edits
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "file_path").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let (path, content) = match (required(input, "file_path"), required(input, "content")) {
            (Ok(path), Ok(content)) => (workspace.resolve(path), content),
            (Err(error), _) | (_, Err(error)) => return error,
        };
        let name = shown(&path, workspace);
        let existed = path.exists();
        if existed {
            match workspace.freshness(&path) {
                Freshness::Unread => {
                    return Outcome::error(format!("{name} already exists; Read it before overwriting it."));
                }
                Freshness::Changed => return Outcome::error(super::changed_since_read(&name)),
                Freshness::Partial => {
                    return Outcome::error(format!(
                        "You read only part of {name}, and Write replaces all of it, so the rest would be lost. \
                         Read the whole file first, or change it with Edit."
                    ));
                }
                Freshness::Current => {}
            }
        }
        if let Some(parent) = path.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            return Outcome::error(format!("Could not create {}: {error}", parent.display()));
        }
        workspace.before_write(&path);
        match std::fs::write(&path, content) {
            Ok(()) => {
                workspace.mark_read(&path);
                let lines = content.lines().count();
                let said = if existed {
                    format!("Replaced {name} ({lines} lines).")
                } else {
                    format!("Created {name} ({lines} lines).")
                };
                let heading = if existed { format!("Replaced {name}") } else { format!("Created {name}") };
                Outcome::ok(said + &workspace.diagnosed(&path, content))
                    .showing(super::shown_diff::created(&heading, content))
            }
            Err(error) => Outcome::error(format!("Could not write {name}: {error}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_new_files_and_overwrites_only_what_was_read() {
        let dir = tempfile::tempdir().expect("dir");
        let workspace = Workspace::new(dir.path());
        let created = Write.run(&json!({"file_path": "deep/new.txt", "content": "a\nb"}), &workspace);
        assert_eq!(created.content, "Created deep/new.txt (2 lines).");

        std::fs::write(dir.path().join("old.txt"), "keep").expect("write");
        let blind = Write.run(&json!({"file_path": "old.txt", "content": "x"}), &workspace);
        assert!(blind.is_error);
        assert_eq!(std::fs::read_to_string(dir.path().join("old.txt")).expect("read"), "keep");

        workspace.mark_read(&dir.path().join("old.txt"));
        assert!(!Write.run(&json!({"file_path": "old.txt", "content": "x"}), &workspace).is_error);
    }

    #[test]
    fn a_part_read_or_changed_file_is_not_overwritten() {
        let dir = tempfile::tempdir().expect("dir");
        let workspace = Workspace::new(dir.path());
        let path = dir.path().join("big.txt");
        std::fs::write(&path, "head\ntail\n").expect("write");
        workspace.mark_partly_read(&path);
        let partial = Write.run(&json!({"file_path": "big.txt", "content": "x"}), &workspace);
        assert!(partial.is_error && partial.content.contains("only part of big.txt"), "{partial:?}");
        workspace.mark_read(&path);
        std::fs::write(&path, "head\ntail\nmore\n").expect("the user edits");
        let changed = Write.run(&json!({"file_path": "big.txt", "content": "x"}), &workspace);
        assert!(changed.is_error && changed.content.contains("changed on disk"), "{changed:?}");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "head\ntail\nmore\n");
    }

    #[test]
    fn a_written_file_s_problems_follow_and_other_files_are_left_alone() {
        let dir = tempfile::tempdir().expect("dir");
        let mut workspace = Workspace::new(dir.path());
        let server = crate::lsp::fake::ScriptedServer::answering([]);
        *server.diagnostics.lock().expect("diagnostics") = Some(vec![json!({"severity": 2, "message": "unused",
            "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 3}}})]);
        workspace.lsp = Some(server.lsp());
        let written = Write.run(&json!({"file_path": "src/a.py", "content": "import os\nx = 1\n"}), &workspace);
        assert_eq!(
            written.content,
            "Created src/a.py (2 lines).\n\n<diagnostics>\nsrc/a.py:2:1 warning: unused\n</diagnostics>"
        );
        let notes = Write.run(&json!({"file_path": "notes.md", "content": "hi"}), &workspace);
        assert_eq!(notes.content, "Created notes.md (1 lines).");
        assert_eq!(server.synced.lock().expect("synced").len(), 1);
    }
}
