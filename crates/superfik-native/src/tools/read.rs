//! `Read`: a file's lines, numbered.

use serde_json::{Value, json};

use super::{Access, Outcome, Tool, Workspace, field, required, shown};
use crate::model::ToolSpec;

const DEFAULT_LINES: usize = 2000;
const LINE_CHARS: usize = 2000;

#[derive(Debug)]
pub struct Read;

impl Tool for Read {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "Read".into(),
            description: format!(
                "Read a text file or a PNG, JPEG, GIF or WebP image from the machine. Images are sent visually to the model. Give an absolute path, or one relative to the working \
                 directory. Returns up to {DEFAULT_LINES} lines from the start, each prefixed with its line number \
                 and a tab; pass `offset` (first line, from 1) and `limit` to read a part of a long file. Lines \
                 longer than {LINE_CHARS} characters are cut. When you know which part of a file you need, read \
                 only that part. Read a file before editing or overwriting it, but do not read it again to check \
                 an edit: Edit and Write fail when a change does not apply. To find files, use Glob; to search \
                 inside them, Grep. Several reads can be requested at once."
            ),
            schema: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "The file to read." },
                    "offset": { "type": "integer", "description": "First line to read, from 1." },
                    "limit": { "type": "integer", "description": "How many lines to read." }
                },
                "required": ["file_path"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
    }

    fn reaches_outside(&self, input: &Value, workspace: &Workspace) -> Option<std::path::PathBuf> {
        workspace.outside(field(input, "file_path"))
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "file_path").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let path = match required(input, "file_path") {
            Ok(path) => workspace.resolve(path),
            Err(error) => return error,
        };
        let offset = input.get("offset").and_then(Value::as_u64).unwrap_or(1).max(1) as usize;
        let limit = input.get("limit").and_then(Value::as_u64).map_or(DEFAULT_LINES, |limit| limit as usize);
        if path.is_dir() {
            return Outcome::error(format!(
                "{} is a directory; use Glob or Bash `ls` to list it.",
                shown(&path, workspace)
            ));
        }
        let bytes = match read_bytes(&path, &workspace.stop) {
            Ok(bytes) => bytes,
            Err(error) => return Outcome::error(format!("Could not read {}: {error}", shown(&path, workspace))),
        };
        match crate::image::Image::from_bytes(&bytes) {
            Ok(Some(image)) => {
                workspace.mark_read(&path);
                return Outcome::image(image);
            }
            Err(error) => return Outcome::error(error),
            Ok(None) => {}
        }
        let Ok(text) = String::from_utf8(bytes) else {
            return Outcome::error(format!("{} is not a text file.", shown(&path, workspace)));
        };
        workspace.mark_read(&path);
        if text.is_empty() {
            return Outcome::ok("(the file is empty)");
        }
        let total = text.lines().count();
        if offset > total {
            return Outcome::error(format!("The file has {total} lines; offset {offset} is past its end."));
        }
        let mut out = String::new();
        for (number, line) in text.lines().enumerate().skip(offset - 1).take(limit) {
            let line = match line.char_indices().nth(LINE_CHARS) {
                Some((at, _)) => format!("{}… [line cut]", &line[..at]),
                None => line.to_owned(),
            };
            out.push_str(&format!("{:>6}\t{line}\n", number + 1));
        }
        let last = (offset - 1 + limit).min(total);
        if last < total {
            out.push_str(&format!("… [{} more lines; read on from offset {}]\n", total - last, last + 1));
        }
        Outcome::ok(out)
    }
}

pub(crate) fn read_bytes(path: &std::path::Path, stop: &std::sync::atomic::AtomicBool) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let mut file = options.open(path)?;
    let metadata = file.metadata()?;
    const MAX: u64 = 64 * 1024 * 1024;
    if !metadata.is_file() {
        return Err(std::io::Error::other("Read requires a regular file, not a pipe or device"));
    }
    if metadata.len() > MAX {
        return Err(std::io::Error::other("file exceeds the 64 MiB read limit"));
    }
    let mut result = Vec::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(std::io::Error::other("file read cancelled"));
        }
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        result.extend_from_slice(&buffer[..count]);
        if result.len() as u64 > MAX {
            return Err(std::io::Error::other("file exceeds the 64 MiB read limit"));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_lines_reads_parts_and_marks_the_file_read() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("a.txt"), "one\ntwo\nthree\n").expect("write");
        let workspace = Workspace::new(dir.path());

        let whole = Read.run(&json!({"file_path": "a.txt"}), &workspace);
        assert_eq!(whole, Outcome::ok("     1\tone\n     2\ttwo\n     3\tthree\n"));
        assert!(workspace.has_read(&dir.path().join("a.txt")));

        let part = Read.run(&json!({"file_path": "a.txt", "offset": 2, "limit": 1}), &workspace);
        assert_eq!(part.content, "     2\ttwo\n… [1 more lines; read on from offset 3]\n");
    }

    #[test]
    fn explains_what_it_cannot_read() {
        let dir = tempfile::tempdir().expect("dir");
        let workspace = Workspace::new(dir.path());
        assert!(Read.run(&json!({"file_path": "missing"}), &workspace).is_error);
        assert!(Read.run(&json!({"file_path": "."}), &workspace).content.contains("directory"));
        assert!(Read.run(&json!({}), &workspace).content.contains("file_path"));
        std::fs::write(dir.path().join("b"), [0xff, 0xfe]).expect("write");
        assert!(Read.run(&json!({"file_path": "b"}), &workspace).content.contains("not a text file"));
    }
}
