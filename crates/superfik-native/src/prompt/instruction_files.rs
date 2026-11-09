//! Ordered project instructions and bounded, local @imports. External imports
//! are not implicitly trusted by a project's Markdown file.
use super::{Instructions, Scope};
use std::{
    collections::HashSet,
    io::Read,
    path::{Path, PathBuf},
};
const MAX_FILE: usize = 1024 * 1024;
const MAX_TOTAL: usize = 4 * 1024 * 1024;
const NAMES: [&str; 7] = [
    "SUPERFIK.md",
    ".superfik/SUPERFIK.md",
    "CLAUDE.md",
    ".claude/CLAUDE.md",
    "AGENTS.md",
    "SUPERFIK.local.md",
    "CLAUDE.local.md",
];
fn network(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.starts_with("\\\\") || s.starts_with("//") || s.starts_with("/net/") || s.starts_with("/Network/")
}
fn trust_root(cwd: &Path) -> PathBuf {
    cwd.ancestors().find(|p| p.join(".git").exists()).unwrap_or(cwd).canonicalize().unwrap_or_else(|_| cwd.into())
}
struct Loader {
    seen: HashSet<PathBuf>,
    bytes: usize,
}
impl Loader {
    fn read(&mut self, path: &Path, allowed: &Path, depth: usize) -> Result<String, String> {
        if depth > 5 {
            return Err("instruction import exceeds five levels".into());
        }
        if network(path) {
            return Err("network instruction paths are not loaded".into());
        }
        let canonical = path.canonicalize().map_err(|_| "instruction file could not be read")?;
        if !canonical.starts_with(allowed) {
            return Err("instruction import is outside the trusted directory".into());
        }
        if !self.seen.insert(canonical.clone()) {
            return Ok(String::new());
        }
        let mut bytes = Vec::new();
        std::fs::File::open(&canonical)
            .map_err(|_| "instruction file could not be opened")?
            .take(MAX_FILE as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "instruction file could not be read")?;
        if bytes.len() > MAX_FILE || self.bytes + bytes.len() > MAX_TOTAL {
            return Err("instruction context exceeds its size budget".into());
        }
        self.bytes += bytes.len();
        let text = String::from_utf8(bytes).map_err(|_| "instruction file is not UTF-8")?;
        let import = regex::Regex::new(r#"(?:^|\s)@("[^"]+"|[^\s`]+)"#).expect("static import expression");
        let mut output = String::new();
        let mut fence: Option<(char, usize)> = None;
        let mut comment = false;
        for raw in text.lines() {
            if let Some((marker, length)) = fence {
                output.push_str(raw);
                output.push('\n');
                if let Some((closing_marker, closing_length, rest)) = fence_marker(raw)
                    && closing_marker == marker
                    && closing_length >= length
                    && rest.trim().is_empty()
                {
                    fence = None;
                }
                continue;
            }
            let clean = visible(raw, &mut comment, true);
            let line = clean.as_str();
            if let Some((marker, length, info)) = fence_marker(line)
                && (marker != '`' || !info.contains('`'))
            {
                fence = Some((marker, length));
                output.push_str(line);
                output.push('\n');
                continue;
            }
            output.push_str(line);
            output.push('\n');
            let importable = visible(line, &mut false, false);
            {
                for capture in import.captures_iter(&importable) {
                    let name = capture[1].trim_matches('"').trim_end_matches([',', ';', ')']);
                    let target = if let Some(home) = name.strip_prefix("~/") {
                        std::env::home_dir().unwrap_or_default().join(home)
                    } else {
                        path.parent().unwrap_or(allowed).join(name)
                    };
                    match self.read(&target, allowed, depth + 1) {
                        Ok(body) if !body.trim().is_empty() => {
                            output.push_str(&format!("\nImported instructions from {}:\n{}\n", target.display(), body))
                        }
                        Ok(_) => {}
                        Err(error) => {
                            output.push_str(&format!("\n[Instruction import unavailable: {name}: {error}]\n"))
                        }
                    }
                }
            }
        }
        Ok(output.trim_end().to_owned())
    }
}
pub fn load(cwd: &Path, home: Option<&Path>) -> Vec<Instructions> {
    let mut loader = Loader { seen: HashSet::new(), bytes: 0 };
    let mut result = Vec::new();
    if let Some(home) = home {
        for (dir, file) in [(".superfik", "SUPERFIK.md"), (".claude", "CLAUDE.md")] {
            let root = home.join(dir);
            let path = root.join(file);
            if let Ok(allowed) = root.canonicalize()
                && let Ok(text) = loader.read(&path, &allowed, 0)
                && !text.trim().is_empty()
            {
                result.push(Instructions { path, scope: Scope::User, text });
            }
        }
    }
    let trusted = trust_root(cwd);
    let mut ancestors: Vec<_> = cwd.ancestors().collect();
    ancestors.reverse();
    for directory in ancestors {
        for name in NAMES {
            let path = directory.join(name);
            if !path.is_file() || network(&path) {
                continue;
            }
            let allowed = if directory.starts_with(&trusted) {
                trusted.clone()
            } else {
                directory.canonicalize().unwrap_or_else(|_| directory.into())
            };
            match loader.read(&path, &allowed, 0) {
                Ok(text) if !text.trim().is_empty() => result.push(Instructions {
                    path,
                    scope: if name.ends_with(".local.md") { Scope::Local } else { Scope::Project },
                    text,
                }),
                _ => {}
            }
        }
    }
    result
}
pub fn nested(cwd: &Path) -> Vec<super::rules::Rule> {
    let trusted = trust_root(cwd);
    let mut paths: Vec<_> = ignore::WalkBuilder::new(cwd)
        .hidden(false)
        .max_depth(Some(32))
        .filter_entry(|e| {
            !e.file_type().is_some_and(|t| t.is_dir())
                || !matches!(e.file_name().to_str(), Some(".git" | "node_modules" | "target"))
        })
        .build()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_some_and(|t| t.is_file())
                && matches!(
                    e.file_name().to_str(),
                    Some("CLAUDE.md" | "CLAUDE.local.md" | "SUPERFIK.md" | "SUPERFIK.local.md" | "AGENTS.md")
                )
        })
        .map(|e| e.into_path())
        .collect();
    paths.sort();
    let mut result = Vec::new();
    for path in paths {
        let Some(mut base) = path.parent() else {
            continue;
        };
        if matches!(base.file_name().and_then(|s| s.to_str()), Some(".claude" | ".superfik")) {
            base = base.parent().unwrap_or(base);
        }
        if base == cwd {
            continue;
        }
        let Ok(relative) = base.strip_prefix(cwd) else {
            continue;
        };
        let mut loader = Loader { seen: HashSet::new(), bytes: 0 };
        let (text, error) = match loader.read(&path, &trusted, 0) {
            Ok(text) => (text, None),
            Err(error) => (String::new(), Some(error)),
        };
        let pattern = format!("{}/**", relative.to_string_lossy().replace('\\', "/"));
        result.push(super::rules::Rule { path, base: Some(cwd.to_path_buf()), text, patterns: vec![pattern], error });
    }
    result
}

fn visible(line: &str, comment: &mut bool, keep_code: bool) -> String {
    let mut output = String::new();
    let mut ticks = 0;
    let mut offset = 0;
    while offset < line.len() {
        let tail = &line[offset..];
        if *comment {
            if let Some(end) = tail.find("-->") {
                offset += end + 3;
                *comment = false;
            } else {
                break;
            }
        } else if ticks == 0 && tail.starts_with("<!--") {
            *comment = true;
            offset += 4;
        } else if tail.starts_with('`') {
            let count = tail.bytes().take_while(|c| *c == b'`').count();
            if ticks == 0 {
                ticks = count;
            } else if ticks == count {
                ticks = 0;
            }
            if keep_code {
                output.push_str(&tail[..count]);
            } else {
                output.push(' ');
            }
            offset += count;
        } else {
            let c = tail.chars().next().expect("nonempty remainder");
            if keep_code || ticks == 0 {
                output.push(c);
            } else {
                output.push(' ');
            }
            offset += c.len_utf8();
        }
    }
    output
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    #[test]
    fn inline_multiline_comments_and_code_never_expand_imports() {
        let repo = tempfile::tempdir().expect("repo");
        std::fs::write(repo.path().join("hidden.md"), "SHOULD_NOT_BE_IMPORTED").expect("hidden");
        std::fs::write(repo.path().join("SUPERFIK.md"),
            "Visible. <!-- @hidden.md --> More.\n<!-- multiline\n```\n@hidden.md\n-->\n``@hidden.md`` and `<!-- @hidden.md -->`\nAfter.").expect("instructions");
        let loaded = load(repo.path(), None);
        let text = loaded.iter().map(|item| item.text.as_str()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("Visible.  More."));
        assert!(text.contains("After."));
        assert!(text.contains("`<!-- @hidden.md -->`"));
        assert!(!text.contains("SHOULD_NOT_BE_IMPORTED"));
        assert!(!text.contains("Imported instructions"));
    }
}

fn fence_marker(line: &str) -> Option<(char, usize, &str)> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let marker = trimmed.chars().next()?;
    if !matches!(marker, '`' | '~') {
        return None;
    }
    let length = trimmed.chars().take_while(|c| *c == marker).count();
    (length >= 3).then(|| (marker, length, &trimmed[length..]))
}
#[cfg(test)]
mod fence_tests {
    use super::*;
    #[test]
    fn shorter_or_annotated_fences_do_not_end_code_or_expand_imports() {
        let repo = tempfile::tempdir().expect("repo");
        std::fs::write(repo.path().join("hidden.md"), "SHOULD_NOT_BE_IMPORTED").expect("hidden");
        std::fs::write(
            repo.path().join("SUPERFIK.md"),
            "````md\n```\n@hidden.md\n````not-a-closer\n@hidden.md\n````\nAfter.",
        )
        .expect("instructions");
        let text = load(repo.path(), None).into_iter().map(|item| item.text).collect::<Vec<_>>().join("\n");
        assert!(text.contains("After."));
        assert!(!text.contains("SHOULD_NOT_BE_IMPORTED"));
    }
}
