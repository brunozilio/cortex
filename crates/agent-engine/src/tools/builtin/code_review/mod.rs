pub mod review;

use crate::transcript::{Message, Role};

pub fn tools() -> Vec<Box<dyn crate::tools::Tool>> {
    Vec::new()
}

const TOOLS: [&str; 3] = ["Read", "Glob", "Grep"];
const KIND: &str = "review";
const CHANGING: [&str; 3] = ["Edit", "Write", "NotebookEdit"];

fn changed(run: &[Message]) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    let calls = run.iter().filter(|message| message.role == Role::Assistant).flat_map(Message::tool_uses);
    for (_, _, input) in calls.filter(|(_, name, _)| CHANGING.contains(name)) {
        let path = input["file_path"].as_str().or_else(|| input["notebook_path"].as_str());
        if let Some(path) = path.filter(|path| !paths.iter().any(|known| known == path)) {
            paths.push(path.to_owned());
        }
    }
    paths
}

fn brief(run: &[Message], paths: &[String]) -> String {
    let task = run.first().map(Message::text).unwrap_or_default();
    let files: Vec<String> = paths.iter().map(|path| format!("- {path}")).collect();
    format!("# Task\n{}\n\n# Changed files\n{}", task.trim(), files.join("\n"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::transcript::Block;

    #[test]
    fn the_brief_names_each_changed_file_once() {
        let edit =
            |path: &str| Block::ToolUse { id: path.into(), name: "Edit".into(), input: json!({ "file_path": path }) };
        let read = Block::ToolUse { id: "r".into(), name: "Read".into(), input: json!({ "file_path": "c.rs" }) };
        let run = vec![
            Message::user("fix the parser"),
            Message::assistant(vec![edit("a.rs"), read, edit("a.rs")]),
            Message::assistant(vec![Block::ToolUse {
                id: "w".into(),
                name: "Write".into(),
                input: json!({ "file_path": "b.md" }),
            }]),
        ];

        let paths = changed(&run);

        assert_eq!(paths, ["a.rs", "b.md"]);
        assert_eq!(brief(&run, &paths), "# Task\nfix the parser\n\n# Changed files\n- a.rs\n- b.md");
        assert!(changed(&run[..1]).is_empty());
    }
}
