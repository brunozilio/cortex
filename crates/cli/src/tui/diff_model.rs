#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: String,
    pub old_path: Option<String>,
    pub status: Status,
    pub hunks: Vec<Hunk>,
    pub added: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Modified,
    Added,
    Deleted,
    Renamed,
    Binary,
}

impl Status {
    pub const fn letter(self) -> char {
        match self {
            Self::Modified => 'M',
            Self::Added => 'A',
            Self::Deleted => 'D',
            Self::Renamed => 'R',
            Self::Binary => 'B',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub context: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: Kind,
    pub number: usize,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Context,
    Added,
    Removed,
}

pub fn parse(diff: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let (mut old, mut new) = (0, 0);
    for line in diff.lines() {
        if let Some(header) = line.strip_prefix("diff --git ") {
            let path = header.rsplit(" b/").next().unwrap_or(header).to_owned();
            files.push(FileDiff {
                path,
                old_path: None,
                status: Status::Modified,
                hunks: Vec::new(),
                added: 0,
                removed: 0,
            });
            continue;
        }
        let Some(file) = files.last_mut() else { continue };
        if let Some(hunk) = line.strip_prefix("@@ ") {
            let (ranges, context) = hunk.split_once(" @@").unwrap_or((hunk, ""));
            (old, new) = starts(ranges);
            file.hunks.push(Hunk { context: context.trim().to_owned(), lines: Vec::new() });
            continue;
        }
        let Some(hunk) = file.hunks.last_mut() else {
            header_line(file, line);
            continue;
        };
        let (kind, number) = match line.chars().next() {
            Some('+') => (Kind::Added, &mut new),
            Some('-') => (Kind::Removed, &mut old),
            Some(' ') => {
                old += 1;
                (Kind::Context, &mut new)
            }
            _ => continue,
        };
        hunk.lines.push(DiffLine { kind, number: *number, text: line[1..].to_owned() });
        *number += 1;
        match kind {
            Kind::Added => file.added += 1,
            Kind::Removed => file.removed += 1,
            Kind::Context => {}
        }
    }
    files
}

fn header_line(file: &mut FileDiff, line: &str) {
    if line.starts_with("new file mode") {
        file.status = Status::Added;
    } else if line.starts_with("deleted file mode") {
        file.status = Status::Deleted;
    } else if let Some(from) = line.strip_prefix("rename from ") {
        file.status = Status::Renamed;
        file.old_path = Some(from.to_owned());
    } else if let Some(to) = line.strip_prefix("rename to ") {
        file.path = to.to_owned();
    } else if line.starts_with("Binary files") {
        file.status = Status::Binary;
    }
}

fn starts(ranges: &str) -> (usize, usize) {
    let mut parts = ranges.split_whitespace();
    let mut start = |prefix: char| {
        parts
            .next()
            .and_then(|part| part.strip_prefix(prefix))
            .and_then(|range| range.split(',').next()?.parse().ok())
            .unwrap_or(1)
    };
    (start('-'), start('+'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(kind: Kind, number: usize, text: &str) -> DiffLine {
        DiffLine { kind, number, text: text.into() }
    }

    #[test]
    fn hunks_are_numbered_by_the_side_each_line_is_on() {
        let diff = "diff --git a/src/a.rs b/src/a.rs\nindex 1..2 100644\n--- a/src/a.rs\n+++ b/src/a.rs\n\
                    @@ -10,3 +10,3 @@ fn a()\n keep\n-old\n+new\n--- not a header\n@@ -40 +40,2 @@\n+added\n same\n\
                    \\ No newline at end of file\n";
        let files = parse(diff);
        assert_eq!(files.len(), 1);
        let file = &files[0];
        assert_eq!((file.path.as_str(), file.status, file.added, file.removed), ("src/a.rs", Status::Modified, 2, 2));
        assert_eq!(file.hunks[0].context, "fn a()");
        assert_eq!(
            file.hunks[0].lines,
            [
                line(Kind::Context, 10, "keep"),
                line(Kind::Removed, 11, "old"),
                line(Kind::Added, 11, "new"),
                line(Kind::Removed, 12, "-- not a header"),
            ]
        );
        assert_eq!(file.hunks[1].lines, [line(Kind::Added, 40, "added"), line(Kind::Context, 41, "same")]);
    }

    #[test]
    fn new_deleted_renamed_and_binary_files_say_so() {
        let diff = "diff --git a/n.txt b/n.txt\nnew file mode 100644\n--- /dev/null\n+++ b/n.txt\n@@ -0,0 +1 @@\n+hi\n\
                    diff --git a/gone.txt b/gone.txt\ndeleted file mode 100644\n--- a/gone.txt\n+++ /dev/null\n\
                    @@ -1 +0,0 @@\n-bye\n\
                    diff --git a/old name.rs b/new name.rs\nsimilarity index 90%\nrename from old name.rs\n\
                    rename to new name.rs\n\
                    diff --git a/logo.png b/logo.png\nBinary files a/logo.png and b/logo.png differ\n";
        let files = parse(diff);
        let summary: Vec<_> = files.iter().map(|f| (f.path.as_str(), f.old_path.as_deref(), f.status)).collect();
        assert_eq!(
            summary,
            [
                ("n.txt", None, Status::Added),
                ("gone.txt", None, Status::Deleted),
                ("new name.rs", Some("old name.rs"), Status::Renamed),
                ("logo.png", None, Status::Binary),
            ]
        );
        assert_eq!(files[1].hunks[0].lines, [line(Kind::Removed, 1, "bye")]);
    }

    #[test]
    fn nothing_changed_is_no_files() {
        assert!(parse("").is_empty());
    }
}
