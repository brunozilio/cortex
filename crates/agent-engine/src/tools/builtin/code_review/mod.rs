pub mod review;

use std::collections::HashSet;

use crate::transcript::{Block, Message, Role};

const TOOLS: [&str; 3] = ["Read", "Glob", "Grep"];
const KIND: &str = "review";
const CHANGING: [&str; 3] = ["Edit", "Write", "NotebookEdit"];

fn succeeded(run: &[Message]) -> HashSet<&str> {
    let results = run.iter().filter(|message| message.role == Role::User).flat_map(|message| &message.blocks);
    results
        .filter_map(|block| match block {
            Block::ToolResult { id, is_error: false, .. } | Block::ToolResultRich { id, is_error: false, .. } => {
                Some(id.as_str())
            }
            _ => None,
        })
        .collect()
}

fn changed(run: &[Message]) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    let done = succeeded(run);
    let calls = run.iter().filter(|message| message.role == Role::Assistant).flat_map(Message::tool_uses);
    for (_, _, input) in calls.filter(|(id, name, _)| CHANGING.contains(name) && done.contains(id)) {
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

    fn done(ids: &[&str]) -> Message {
        let results =
            ids.iter().map(|id| Block::ToolResult { id: (*id).into(), content: "ok".into(), is_error: false });
        Message { role: Role::User, blocks: results.collect() }
    }

    #[test]
    fn the_brief_names_each_changed_file_once() {
        let edit =
            |path: &str| Block::ToolUse { id: path.into(), name: "Edit".into(), input: json!({ "file_path": path }) };
        let read = Block::ToolUse { id: "r".into(), name: "Read".into(), input: json!({ "file_path": "c.rs" }) };
        let run = vec![
            Message::user("fix the parser"),
            Message::assistant(vec![edit("a.rs"), read, edit("a.rs")]),
            done(&["a.rs", "r"]),
            Message::assistant(vec![Block::ToolUse {
                id: "w".into(),
                name: "Write".into(),
                input: json!({ "file_path": "b.md" }),
            }]),
            done(&["w"]),
        ];

        let paths = changed(&run);

        assert_eq!(paths, ["a.rs", "b.md"]);
        assert_eq!(brief(&run, &paths), "# Task\nfix the parser\n\n# Changed files\n- a.rs\n- b.md");
        assert!(changed(&run[..1]).is_empty());
    }

    #[test]
    fn an_edit_that_was_denied_or_failed_or_never_answered_changed_nothing() {
        let write = |id: &str, path: &str| Block::ToolUse {
            id: id.into(),
            name: "Write".into(),
            input: json!({ "file_path": path }),
        };
        let result = |id: &str, is_error: bool| Block::ToolResult { id: id.into(), content: "said".into(), is_error };
        let run = vec![
            Message::user("write them"),
            Message::assistant(vec![write("denied", "denied.rs"), write("done", "done.rs")]),
            Message { role: Role::User, blocks: vec![result("denied", true), result("done", false)] },
            Message::assistant(vec![write("cut", "cut.rs")]),
        ];

        assert_eq!(changed(&run), ["done.rs"]);
    }
}
