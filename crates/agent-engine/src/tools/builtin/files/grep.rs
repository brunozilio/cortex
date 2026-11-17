use std::path::{Path, PathBuf};

use serde_json::Value;

use super::ripgrep::{self, Asked};
use crate::model::ToolSpec;
use crate::tools::{Outcome, Tool, Workspace, clip, field, required, shown};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Engine {
    BuiltIn,
    Ripgrep(PathBuf),
}

impl Tool for Grep {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared("Grep", &serde_json::Value::Null)
    }

    fn effects(&self, input: &Value, workspace: &Workspace) -> Vec<crate::tools::Effect> {
        vec![crate::tools::Effect::ReadTree(workspace.place(field(input, "path").unwrap_or(".")))]
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "pattern").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let engine = ripgrep::program().map_or(Engine::BuiltIn, Engine::Ripgrep);
        search(input, workspace, &engine)
    }
}

struct Matcher {
    regex: regex::Regex,
    multiline: bool,
}

impl Matcher {
    fn any(&self, text: &str) -> bool {
        if self.multiline { self.regex.is_match(text) } else { text.lines().any(|line| self.regex.is_match(line)) }
    }

    fn count(&self, text: &str) -> usize {
        if self.multiline {
            self.regex.find_iter(text).count()
        } else {
            text.lines().map(|line| self.regex.find_iter(line).count()).sum()
        }
    }

    fn hit_lines(&self, text: &str, lines: usize) -> Vec<usize> {
        if !self.multiline {
            return text.lines().enumerate().filter(|(_, line)| self.regex.is_match(line)).map(|(at, _)| at).collect();
        }
        let starts: Vec<usize> = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(at, _)| at + 1))
            .filter(|at| *at < text.len().max(1))
            .collect();
        let line_of = |offset: usize| starts.partition_point(|start| *start <= offset).saturating_sub(1);
        let mut hits: Vec<usize> = Vec::new();
        for found in self.regex.find_iter(text) {
            let last = if found.end() > found.start() { found.end() - 1 } else { found.start() };
            hits.extend(line_of(found.start())..=line_of(last));
        }
        hits.retain(|line| *line < lines);
        hits.sort_unstable();
        hits.dedup();
        hits
    }
}

pub(super) fn search(input: &Value, workspace: &Workspace, engine: &Engine) -> Outcome {
    let pattern = match required(input, "pattern") {
        Ok(pattern) => pattern,
        Err(error) => return error,
    };
    let flag = |name: &str| input.get(name).and_then(Value::as_bool).unwrap_or(false);
    let number = |name: &str| input.get(name).and_then(Value::as_u64).map(|n| n as usize);
    let (multiline, ignore_case) = (flag("multiline"), flag("-i"));
    let regex = match regex::RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .multi_line(true)
        .dot_matches_new_line(multiline)
        .build()
    {
        Ok(regex) => regex,
        Err(error) => return Outcome::error(format!("Invalid pattern: {error}")),
    };
    let matcher = Matcher { regex, multiline };
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
    let around = (number("-B").unwrap_or(context), number("-A").unwrap_or(context));
    let root = field(input, "path").map_or_else(|| workspace.cwd.clone(), |path| workspace.resolve(path));
    let limit = number("head_limit").filter(|limit| *limit > 0).unwrap_or(RESULTS);

    let asked = Asked { pattern, ignore_case, multiline };
    let candidates = match engine {
        Engine::Ripgrep(program) => ripgrep::candidates(program, &root, &asked, &workspace.stop),
        Engine::BuiltIn => None,
    };
    let mut files = candidates.unwrap_or_else(|| walk(&root, workspace));
    files.retain(|path| matches_glob(filter.as_ref(), path));
    files.sort();
    files.dedup();

    let mut results = Results::default();
    for path in files {
        if workspace.stop.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        let Ok(bytes) = super::read::read_bytes(&path, &workspace.stop) else { continue };
        let Ok(text) = String::from_utf8(bytes) else { continue };
        let name = shown(&path, workspace);
        match mode {
            Mode::Files if matcher.any(&text) => results.entry(name, limit),
            Mode::Count => {
                let count = matcher.count(&text);
                if count > 0 {
                    results.entry(format!("{name}:{count}"), limit);
                }
            }
            Mode::Content => results.file(&name, lines(&matcher, &text, flag("-n"), around), limit),
            Mode::Files => {}
        }
    }
    if results.shown.is_empty() {
        return Outcome::ok("No matches found.");
    }
    let between = if mode == Mode::Content { "\n\n" } else { "\n" };
    Outcome::ok(clip(&results.text(between), OUTPUT))
}

fn walk(root: &Path, workspace: &Workspace) -> Vec<PathBuf> {
    let walk = ignore::WalkBuilder::new(root)
        .hidden(false)
        .add_custom_ignore_filename(".rgignore")
        .filter_entry(|entry| entry.file_name() != ".git")
        .build();
    let mut files = Vec::new();
    for entry in walk.filter_map(Result::ok) {
        if workspace.stop.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        if entry.file_type().is_some_and(|kind| kind.is_file()) {
            files.push(entry.into_path());
        }
    }
    files
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

fn lines(matcher: &Matcher, text: &str, numbered: bool, (before, after): (usize, usize)) -> Matches {
    let all: Vec<&str> = text.lines().collect();
    let hits = matcher.hit_lines(text, all.len());
    let more = hits.len().saturating_sub(PER_FILE);
    let mut out = Vec::new();
    let mut shown_until = 0;
    for &hit in hits.iter().take(PER_FILE) {
        let start = hit.saturating_sub(before).max(shown_until);
        let end = (hit + after + 1).min(all.len()).max(start);
        if start > shown_until && shown_until > 0 && (before > 0 || after > 0) {
            out.push("--".to_owned());
        }
        for (at, line) in all.iter().enumerate().take(end).skip(start) {
            let mark = if hits.binary_search(&at).is_ok() { ':' } else { '-' };
            out.push(if numbered { format!("{}{mark}{}", at + 1, cut(line)) } else { cut(line) });
        }
        shown_until = shown_until.max(end);
    }
    Matches { lines: out, more }
}

#[cfg(test)]
#[path = "grep_tests.rs"]
mod tests;
