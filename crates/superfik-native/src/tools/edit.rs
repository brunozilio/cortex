//! `Edit`: an exact string replaced in a file.

use serde_json::{Value, json};

use super::{Access, Outcome, Tool, Workspace, field, required, shown};
use crate::model::ToolSpec;

#[derive(Debug)]
pub struct Edit;

impl Tool for Edit {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "Edit".into(),
            description: "Replace `old_string` with `new_string` in a file read earlier with Read. `old_string` \
                          must match the file exactly, indentation included (without Read's line-number prefix), \
                          and appear only once unless `replace_all` is true; include surrounding lines to make it \
                          unique. To create a file, use Write."
                .into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "The file to change." },
                    "old_string": { "type": "string", "description": "The exact text to replace." },
                    "new_string": { "type": "string", "description": "The text to put in its place." },
                    "replace_all": { "type": "boolean", "description": "Replace every occurrence.", "default": false }
                },
                "required": ["file_path", "old_string", "new_string"],
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
        let fields = (required(input, "file_path"), required(input, "old_string"), required(input, "new_string"));
        let (path, old, new) = match fields {
            (Ok(path), Ok(old), Ok(new)) => (workspace.resolve(path), old, new),
            (Err(error), ..) | (_, Err(error), _) | (.., Err(error)) => return error,
        };
        let all = input.get("replace_all").and_then(Value::as_bool).unwrap_or(false);
        let name = shown(&path, workspace);
        if old == new {
            return Outcome::error("`old_string` and `new_string` are the same; nothing to change.");
        }
        if !workspace.has_read(&path) {
            return Outcome::error(format!("Read {name} before editing it."));
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => return Outcome::error(format!("Could not read {name}: {error}")),
        };
        let count = text.matches(old).count();
        let changed = match (count, all) {
            (0, _) => {
                return Outcome::error(format!(
                    "`old_string` was not found in {name}. Read the file again and copy the text exactly."
                ));
            }
            (1, _) | (_, true) => text.replace(old, new),
            (count, false) => {
                return Outcome::error(format!(
                    "`old_string` appears {count} times in {name}. Include more surrounding text to make it unique, \
                     or set `replace_all`."
                ));
            }
        };
        let before = text.find(old).map_or("", |at| &text[..at]);
        let shown = super::shown_diff::edit(&format!("Edited {name}"), before, old, new);
        match std::fs::write(&path, changed) {
            Ok(()) => Outcome::ok(if count == 1 {
                format!("Edited {name}.")
            } else {
                format!("Edited {name} ({count} replacements).")
            })
            .showing(shown),
            Err(error) => Outcome::error(format!("Could not write {name}: {error}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(text: &str) -> (tempfile::TempDir, Workspace) {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("f.rs"), text).expect("write");
        let workspace = Workspace::new(dir.path());
        workspace.mark_read(&dir.path().join("f.rs"));
        (dir, workspace)
    }

    fn content(dir: &tempfile::TempDir) -> String {
        std::fs::read_to_string(dir.path().join("f.rs")).expect("read")
    }

    #[test]
    fn replaces_a_unique_string() {
        let (dir, workspace) = setup("let a = 1;\nlet b = 2;\n");
        let outcome = Edit.run(&json!({"file_path": "f.rs", "old_string": "b = 2", "new_string": "b = 3"}), &workspace);
        assert_eq!(outcome.content, "Edited f.rs.");
        assert_eq!(content(&dir), "let a = 1;\nlet b = 3;\n");
    }

    #[test]
    fn refuses_ambiguous_missing_and_unread() {
        let (dir, workspace) = setup("x x");
        let twice = Edit.run(&json!({"file_path": "f.rs", "old_string": "x", "new_string": "y"}), &workspace);
        assert!(twice.content.contains("2 times"));
        let missing = Edit.run(&json!({"file_path": "f.rs", "old_string": "z", "new_string": "y"}), &workspace);
        assert!(missing.content.contains("not found"));
        assert_eq!(content(&dir), "x x");

        let all = Edit
            .run(&json!({"file_path": "f.rs", "old_string": "x", "new_string": "y", "replace_all": true}), &workspace);
        assert_eq!(all.content, "Edited f.rs (2 replacements).");

        std::fs::write(dir.path().join("g.rs"), "x").expect("write");
        let unread = Edit.run(&json!({"file_path": "g.rs", "old_string": "x", "new_string": "y"}), &workspace);
        assert!(unread.content.starts_with("Read g.rs"));
    }
}
