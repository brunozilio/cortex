use std::path::Path;

use serde_json::{Value, json};

use crate::model::ToolSpec;
use crate::tools::{Access, Outcome, Tool, Workspace, clip, field, required, shown};

const OUTPUT: usize = 2_000_000;

#[derive(Debug)]
pub struct Grep;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Content,
    Files,
    Count,
}

impl Tool for Grep {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "Grep".into(),
            description: crate::tools::describe("Grep", &serde_json::Value::Null),
            schema: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "The regular expression." },
                    "path": { "type": "string", "description": "File or directory to search; the working directory if omitted." },
                    "glob": { "type": "string", "description": "Only files whose name matches this glob." },
                    "output_mode": { "type": "string", "enum": ["content", "files_with_matches", "count"] },
                    "-i": { "type": "boolean", "description": "Ignore case." },
                    "-n": { "type": "boolean", "description": "Number lines (content mode)." },
                    "-A": { "type": "integer", "description": "Lines after each match (content mode)." },
                    "-B": { "type": "integer", "description": "Lines before each match (content mode)." },
                    "-C": { "type": "integer", "description": "Lines around each match (content mode)." },
                    "multiline": { "type": "boolean", "description": "Let matches span lines." },
                    "head_limit": { "type": "integer", "description": "Keep only the first N results." }
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
        let flag = |name: &str| input.get(name).and_then(Value::as_bool).unwrap_or(false);
        let number = |name: &str| input.get(name).and_then(Value::as_u64).map(|n| n as usize);
        let multiline = flag("multiline");
        let regex = match regex::RegexBuilder::new(pattern)
            .case_insensitive(flag("-i"))
            .multi_line(true)
            .dot_matches_new_line(multiline)
            .build()
        {
            Ok(regex) => regex,
            Err(error) => return Outcome::error(format!("Invalid pattern: {error}")),
        };
        let mode = match field(input, "output_mode") {
            Some("content") => Mode::Content,
            Some("count") => Mode::Count,
            _ => Mode::Files,
        };
        let filter = match field(input, "glob").map(globset::Glob::new) {
            Some(Ok(glob)) => Some(glob.compile_matcher()),
            Some(Err(error)) => return Outcome::error(format!("Invalid glob: {error}")),
            None => None,
        };
        let context = number("-C").unwrap_or(0);
        let (before, after) = (number("-B").unwrap_or(context), number("-A").unwrap_or(context));
        let root = field(input, "path").map_or_else(|| workspace.cwd.clone(), |path| workspace.resolve(path));

        let mut results: Vec<String> = Vec::new();
        let walk =
            ignore::WalkBuilder::new(&root).hidden(false).filter_entry(|entry| entry.file_name() != ".git").build();
        for entry in walk.filter_map(Result::ok) {
            if workspace.stop.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            let path = entry.path();
            if !entry.file_type().is_some_and(|kind| kind.is_file()) || !matches_glob(filter.as_ref(), path) {
                continue;
            }
            let Ok(bytes) = super::read::read_bytes(path, &workspace.stop) else { continue };
            let Ok(text) = String::from_utf8(bytes) else { continue };
            let name = shown(path, workspace);
            match mode {
                Mode::Files if regex.is_match(&text) => results.push(name),
                Mode::Count => {
                    let count = regex.find_iter(&text).count();
                    if count > 0 {
                        results.push(format!("{name}:{count}"));
                    }
                }
                Mode::Content => lines(&regex, &text, &name, flag("-n"), (before, after), &mut results),
                Mode::Files => {}
            }
        }
        if results.is_empty() {
            return Outcome::ok("No matches found.");
        }
        if let Some(limit) = number("head_limit") {
            results.truncate(limit);
        }
        Outcome::ok(clip(&results.join("\n"), OUTPUT))
    }
}

fn matches_glob(filter: Option<&globset::GlobMatcher>, path: &Path) -> bool {
    filter.is_none_or(|glob| path.file_name().is_some_and(|name| glob.is_match(name)) || glob.is_match(path))
}

fn lines(
    regex: &regex::Regex,
    text: &str,
    name: &str,
    numbered: bool,
    (before, after): (usize, usize),
    out: &mut Vec<String>,
) {
    let all: Vec<&str> = text.lines().collect();
    let hits: Vec<usize> = all.iter().enumerate().filter(|(_, line)| regex.is_match(line)).map(|(at, _)| at).collect();
    let mut shown_until = 0;
    for hit in hits {
        let start = hit.saturating_sub(before).max(shown_until);
        let end = (hit + after + 1).min(all.len());
        if start > shown_until && shown_until > 0 && (before > 0 || after > 0) {
            out.push("--".to_owned());
        }
        for (at, line) in all.iter().enumerate().take(end).skip(start) {
            let mark = if at == hit { ':' } else { '-' };
            out.push(if numbered {
                format!("{name}{mark}{}{mark}{line}", at + 1)
            } else {
                format!("{name}{mark}{line}")
            });
        }
        shown_until = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> (tempfile::TempDir, Workspace) {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::create_dir(dir.path().join("src")).expect("dir");
        std::fs::write(dir.path().join("src/a.rs"), "fn main() {\n    run();\n}\n").expect("write");
        std::fs::write(dir.path().join("src/b.md"), "Run it\nrun again\n").expect("write");
        let workspace = Workspace::new(dir.path());
        (dir, workspace)
    }

    #[test]
    fn lists_files_counts_and_lines() {
        let (_dir, workspace) = workspace();
        let mut files: Vec<String> = Grep
            .run(&json!({"pattern": "run"}), &workspace)
            .content
            .lines()
            .map(|line| line.replace('\\', "/"))
            .collect();
        files.sort();
        assert_eq!(files, vec!["src/a.rs", "src/b.md"]);

        let count =
            Grep.run(&json!({"pattern": "run", "-i": true, "glob": "*.md", "output_mode": "count"}), &workspace);
        assert_eq!(count.content.replace('\\', "/"), "src/b.md:2");

        let content =
            Grep.run(&json!({"pattern": "run\\(", "output_mode": "content", "-n": true, "-B": 1}), &workspace);
        assert_eq!(content.content.replace('\\', "/"), "src/a.rs-1-fn main() {\nsrc/a.rs:2:    run();");
    }

    #[test]
    fn reports_no_match_and_bad_patterns() {
        let (_dir, workspace) = workspace();
        assert_eq!(Grep.run(&json!({"pattern": "absent"}), &workspace).content, "No matches found.");
        assert!(Grep.run(&json!({"pattern": "("}), &workspace).is_error);
    }
}
