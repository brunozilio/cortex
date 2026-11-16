use std::path::Path;

use serde_json::Value;

use crate::model::ToolSpec;
use crate::tools::{Access, Outcome, Tool, Workspace, clip, field, required, shown};

const OUTPUT: usize = 2_000_000;
const RESULTS: usize = 200;
const PER_FILE: usize = 25;
const LINE: usize = 300;

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
        crate::tools::declared("Grep", &serde_json::Value::Null)
    }

    fn access(&self) -> Access {
        crate::tools::declared_access("Grep")
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

        let limit = number("head_limit").filter(|limit| *limit > 0).unwrap_or(RESULTS);
        let mut results = Results::default();
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
                Mode::Files if regex.is_match(&text) => results.entry(name, limit),
                Mode::Count => {
                    let count = regex.find_iter(&text).count();
                    if count > 0 {
                        results.entry(format!("{name}:{count}"), limit);
                    }
                }
                Mode::Content => results.file(&name, lines(&regex, &text, flag("-n"), (before, after)), limit),
                Mode::Files => {}
            }
        }
        if results.shown.is_empty() {
            return Outcome::ok("No matches found.");
        }
        let between = if mode == Mode::Content { "\n\n" } else { "\n" };
        Outcome::ok(clip(&results.text(between), OUTPUT))
    }
}

#[derive(Default)]
struct Results {
    shown: Vec<String>,
    kept: usize,
    hidden: usize,
}

struct Matches {
    lines: Vec<String>,
    more: usize,
}

impl Results {
    fn entry(&mut self, entry: String, limit: usize) {
        if self.kept < limit {
            self.shown.push(entry);
            self.kept += 1;
        } else {
            self.hidden += 1;
        }
    }

    fn file(&mut self, name: &str, matches: Matches, limit: usize) {
        let room = limit.saturating_sub(self.kept).min(matches.lines.len());
        self.hidden += matches.lines.len() - room;
        if room == 0 {
            return;
        }
        self.kept += room;
        let mut group = format!("{name}\n{}", matches.lines[..room].join("\n"));
        if matches.more > 0 && room == matches.lines.len() {
            group.push_str(&format!("\n… [{} more matches in this file]", matches.more));
        }
        self.shown.push(group);
    }

    fn text(&self, between: &str) -> String {
        let mut text = self.shown.join(between);
        if self.hidden > 0 {
            text.push_str(&format!("\n… [{} more results; narrow the search or raise head_limit]", self.hidden));
        }
        text
    }
}

fn cut(line: &str) -> String {
    match line.char_indices().nth(LINE) {
        None => line.to_owned(),
        Some((at, _)) => format!("{}… [+{} characters]", &line[..at], line[at..].chars().count()),
    }
}

fn matches_glob(filter: Option<&globset::GlobMatcher>, path: &Path) -> bool {
    filter.is_none_or(|glob| path.file_name().is_some_and(|name| glob.is_match(name)) || glob.is_match(path))
}

fn lines(regex: &regex::Regex, text: &str, numbered: bool, (before, after): (usize, usize)) -> Matches {
    let all: Vec<&str> = text.lines().collect();
    let hits: Vec<usize> = all.iter().enumerate().filter(|(_, line)| regex.is_match(line)).map(|(at, _)| at).collect();
    let more = hits.len().saturating_sub(PER_FILE);
    let mut out = Vec::new();
    let mut shown_until = 0;
    for hit in hits.into_iter().take(PER_FILE) {
        let start = hit.saturating_sub(before).max(shown_until);
        let end = (hit + after + 1).min(all.len());
        if start > shown_until && shown_until > 0 && (before > 0 || after > 0) {
            out.push("--".to_owned());
        }
        for (at, line) in all.iter().enumerate().take(end).skip(start) {
            let mark = if at == hit { ':' } else { '-' };
            out.push(if numbered { format!("{}{mark}{}", at + 1, cut(line)) } else { cut(line) });
        }
        shown_until = end;
    }
    Matches { lines: out, more }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

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
        assert_eq!(content.content.replace('\\', "/"), "src/a.rs\n1-fn main() {\n2:    run();");
    }

    fn content_of(dir: &tempfile::TempDir, input: &Value) -> String {
        Grep.run(input, &Workspace::new(dir.path())).content
    }

    #[test]
    fn matching_lines_are_grouped_under_their_file() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("a.txt"), "hit one\nmiss\nhit two\n").expect("write");
        std::fs::write(dir.path().join("b.txt"), "hit three\n").expect("write");
        let found = content_of(&dir, &json!({"pattern": "hit", "output_mode": "content", "-n": true}));
        let mut groups: Vec<&str> = found.split("\n\n").collect();
        groups.sort_unstable();
        assert_eq!(groups, vec!["a.txt\n1:hit one\n3:hit two", "b.txt\n1:hit three"]);
    }

    #[test]
    fn a_long_line_is_cut_and_says_how_much_was_left_out() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("min.js"), format!("hit{}\n", "x".repeat(1_000))).expect("write");
        let found = content_of(&dir, &json!({"pattern": "hit", "output_mode": "content"}));
        assert_eq!(found, format!("min.js\nhit{}… [+703 characters]", "x".repeat(297)));
    }

    #[test]
    fn one_file_shows_its_first_matches_and_counts_the_rest() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("log.txt"), "hit\n".repeat(30)).expect("write");
        let found = content_of(&dir, &json!({"pattern": "hit", "output_mode": "content"}));
        assert_eq!(found, format!("log.txt\n{}… [5 more matches in this file]", "hit\n".repeat(25)));
    }

    #[test]
    fn results_stop_at_a_default_limit_that_head_limit_replaces() {
        let dir = tempfile::tempdir().expect("dir");
        for at in 0..205 {
            std::fs::write(dir.path().join(format!("f{at}.txt")), "hit\n").expect("write");
        }
        let listed = content_of(&dir, &json!({"pattern": "hit"}));
        assert_eq!(listed.lines().count(), 201);
        assert!(listed.ends_with("\n… [5 more results; narrow the search or raise head_limit]"), "{listed}");
        assert_eq!(content_of(&dir, &json!({"pattern": "hit", "head_limit": 300})).lines().count(), 205);
        let two = content_of(&dir, &json!({"pattern": "hit", "head_limit": 2}));
        assert!(two.ends_with("\n… [203 more results; narrow the search or raise head_limit]"), "{two}");
    }

    #[test]
    fn reports_no_match_and_bad_patterns() {
        let (_dir, workspace) = workspace();
        assert_eq!(Grep.run(&json!({"pattern": "absent"}), &workspace).content, "No matches found.");
        assert!(Grep.run(&json!({"pattern": "("}), &workspace).is_error);
    }
}
