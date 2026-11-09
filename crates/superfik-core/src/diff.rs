//! Unified diffs (`git diff`, `gh pr diff --patch`) parsed into files, hunks
//! and numbered lines. Pure; tolerant of what git adds around hunks
//! (`index`, mode and rename headers, "\\ No newline at end of file").

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FileStatus {
    Added,
    Removed,
    Modified,
    Renamed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: LineKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hunk {
    pub old_start: u32,
    pub new_start: u32,
    pub heading: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDiff {
    pub old_path: String,
    pub new_path: String,
    pub status: FileStatus,
    pub hunks: Vec<Hunk>,
    pub binary: bool,
}

impl FileDiff {
    pub fn path(&self) -> &str {
        if self.status == FileStatus::Removed { &self.old_path } else { &self.new_path }
    }

    pub fn additions(&self) -> usize {
        self.hunks.iter().flat_map(|h| &h.lines).filter(|l| l.kind == LineKind::Added).count()
    }

    pub fn deletions(&self) -> usize {
        self.hunks.iter().flat_map(|h| &h.lines).filter(|l| l.kind == LineKind::Removed).count()
    }
}

pub fn parse(patch: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut hunk: Option<(u32, u32)> = None;
    for raw in patch.split_inclusive('\n') {
        let line = raw.strip_suffix('\n').unwrap_or(raw).strip_suffix('\r').unwrap_or(raw.trim_end_matches('\n'));
        if let Some(rest) = line.strip_prefix("diff --git ") {
            let (old, new) = split_git_paths(rest);
            files.push(FileDiff {
                old_path: old,
                new_path: new,
                status: FileStatus::Modified,
                hunks: Vec::new(),
                binary: false,
            });
            hunk = None;
            continue;
        }
        let Some(file) = files.last_mut() else { continue };
        if let Some(header) = line.strip_prefix("@@ ") {
            if let Some((old_start, new_start, heading)) = parse_hunk_header(header) {
                file.hunks.push(Hunk { old_start, new_start, heading, lines: Vec::new() });
                hunk = Some((old_start, new_start));
            }
            continue;
        }
        match hunk {
            None => {
                if line.starts_with("new file mode") {
                    file.status = FileStatus::Added;
                } else if line.starts_with("deleted file mode") {
                    file.status = FileStatus::Removed;
                } else if line.starts_with("rename from") || line.starts_with("similarity index") {
                    file.status = FileStatus::Renamed;
                } else if line.starts_with("Binary files") || line.starts_with("GIT binary patch") {
                    file.binary = true;
                } else if let Some(path) = line.strip_prefix("--- ")
                    && path != "/dev/null"
                {
                    file.old_path = strip_prefix(path);
                } else if let Some(path) = line.strip_prefix("+++ ")
                    && path != "/dev/null"
                {
                    file.new_path = strip_prefix(path);
                }
            }
            Some((old, new)) => {
                let Some(current) = file.hunks.last_mut() else { continue };
                let (kind, text) = match line.chars().next() {
                    Some('+') => (LineKind::Added, &line[1..]),
                    Some('-') => (LineKind::Removed, &line[1..]),
                    Some(' ') => (LineKind::Context, &line[1..]),
                    Some('\\') => continue,
                    None => (LineKind::Context, ""),
                    _ => continue,
                };
                let (old_no, new_no) = match kind {
                    LineKind::Context => (Some(old), Some(new)),
                    LineKind::Added => (None, Some(new)),
                    LineKind::Removed => (Some(old), None),
                };
                current.lines.push(DiffLine { kind, old_no, new_no, text: text.to_owned() });
                hunk = Some((old + u32::from(old_no.is_some()), new + u32::from(new_no.is_some())));
            }
        }
    }
    files
}

fn split_git_paths(rest: &str) -> (String, String) {
    let rest = rest.trim();
    if let Some(index) = rest.find(" b/") {
        let old = rest[..index].strip_prefix("a/").unwrap_or(&rest[..index]);
        let new = &rest[index + 3..];
        return (old.to_owned(), new.to_owned());
    }
    (rest.to_owned(), rest.to_owned())
}

fn strip_prefix(path: &str) -> String {
    let path = path.split('\t').next().unwrap_or(path);
    path.strip_prefix("a/").or_else(|| path.strip_prefix("b/")).unwrap_or(path).to_owned()
}

fn parse_hunk_header(header: &str) -> Option<(u32, u32, String)> {
    let (ranges, heading) = header.split_once(" @@")?;
    let mut parts = ranges.split(' ');
    let old = parts.next()?.strip_prefix('-')?;
    let new = parts.next()?.strip_prefix('+')?;
    let start = |range: &str| range.split(',').next().and_then(|n| n.parse::<u32>().ok());
    Some((start(old)?, start(new)?, heading.trim().to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH: &str = "diff --git a/src/sky.rs b/src/sky.rs\nindex 1111..2222 100644\n--- a/src/sky.rs\n+++ b/src/sky.rs\n@@ -1,4 +1,5 @@ fn sky()\n use stars;\n-let moon = 1;\n+let moon = 2;\n+let haze = 3;\n let end = 0;\n\\ No newline at end of file\ndiff --git a/new.txt b/new.txt\nnew file mode 100644\n--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1,2 @@\n+hello\n+world\ndiff --git a/img.png b/img.png\nBinary files a/img.png and b/img.png differ\n";

    #[test]
    fn files_hunks_and_numbered_lines() {
        let files = parse(PATCH);
        assert_eq!(files.len(), 3);
        let sky = &files[0];
        assert_eq!((sky.path(), sky.status), ("src/sky.rs", FileStatus::Modified));
        assert_eq!(sky.hunks[0].heading, "fn sky()");
        let lines = &sky.hunks[0].lines;
        assert_eq!(lines.len(), 5);
        assert_eq!((lines[0].kind, lines[0].old_no, lines[0].new_no), (LineKind::Context, Some(1), Some(1)));
        assert_eq!((lines[1].kind, lines[1].old_no, lines[1].new_no), (LineKind::Removed, Some(2), None));
        assert_eq!((lines[2].kind, lines[2].old_no, lines[2].new_no), (LineKind::Added, None, Some(2)));
        assert_eq!(
            (lines[3].kind, lines[3].new_no, lines[3].text.as_str()),
            (LineKind::Added, Some(3), "let haze = 3;")
        );
        assert_eq!((lines[4].old_no, lines[4].new_no), (Some(3), Some(4)));
        assert_eq!((sky.additions(), sky.deletions()), (2, 1));
        let new = &files[1];
        assert_eq!((new.path(), new.status, new.hunks[0].lines.len()), ("new.txt", FileStatus::Added, 2));
        assert!(files[2].binary && files[2].hunks.is_empty());
    }

    #[test]
    fn renames_and_deletions_are_recognised() {
        let patch = "diff --git a/a.rs b/b.rs\nsimilarity index 90%\nrename from a.rs\nrename to b.rs\ndiff --git a/gone.rs b/gone.rs\ndeleted file mode 100644\n--- a/gone.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n-bye\n";
        let files = parse(patch);
        assert_eq!(
            (files[0].status, files[0].old_path.as_str(), files[0].new_path.as_str()),
            (FileStatus::Renamed, "a.rs", "b.rs")
        );
        assert_eq!((files[1].status, files[1].path()), (FileStatus::Removed, "gone.rs"));
        assert!(parse("").is_empty());
        assert!(parse("garbage\n@@ -1 +1 @@\n").is_empty());
    }
}
