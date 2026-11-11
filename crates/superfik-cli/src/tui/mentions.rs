//! `@path` in a prompt: completing it from the project's files (fuzzy, the
//! file's own name first) and sending the files' text along with it.

use std::path::Path;

/// How many files the completion offers.
const OFFERED: usize = 8;
/// What of a mentioned file goes with the prompt, at most.
const KEPT: usize = 50_000;

/// The files offered for the `@word` being typed.
pub(super) fn offered(app: &super::app::App) -> Vec<String> {
    let Some((_, query)) = typing(app.input.text(), app.input.cursor()) else { return Vec::new() };
    app.files.as_deref().map(|files| rank(query, files).into_iter().map(str::to_owned).collect()).unwrap_or_default()
}

/// Asks for the project's files the first time an `@word` is typed.
pub(super) fn load(app: &mut super::app::App) {
    if app.files.is_none() && typing(app.input.text(), app.input.cursor()).is_some() {
        app.files = Some(Vec::new());
        let cwd = app.cwd.clone();
        app.spawn(move |_| super::app::Msg::Files(list(&cwd)));
    }
}

/// Puts the chosen file in place of the `@word` being typed.
pub(super) fn complete(app: &mut super::app::App, file: &str) {
    let (text, cursor) = (app.input.text().to_owned(), app.input.cursor());
    let Some((start, _)) = typing(&text, cursor) else { return };
    let rest = text[cursor..].trim_start();
    let joined = if rest.is_empty() { String::new() } else { format!(" {rest}") };
    app.input.set(&format!("{}@{file} {}", &text[..start], joined.trim_start()).replace("  ", " "));
}

/// The `@word` the cursor ends: where it starts (at the `@`) and what
/// follows the `@`.
pub fn typing(text: &str, cursor: usize) -> Option<(usize, &str)> {
    let before = &text[..cursor];
    let start = before.rfind(char::is_whitespace).map_or(0, |at| at + 1);
    let word = &before[start..];
    word.strip_prefix('@').map(|rest| (start, rest))
}

/// The files best matching `query`: its letters in order, best when they
/// fall in the file's name and close together, then shorter paths.
pub fn rank<'a>(query: &str, files: &'a [String]) -> Vec<&'a str> {
    let query = query.to_lowercase();
    let mut scored: Vec<(i64, &str)> =
        files.iter().filter_map(|file| score(&query, file).map(|score| (score, file.as_str()))).collect();
    scored.sort_by(|(a, path_a), (b, path_b)| b.cmp(a).then(path_a.len().cmp(&path_b.len())));
    scored.into_iter().take(OFFERED).map(|(_, file)| file).collect()
}

fn score(query: &str, file: &str) -> Option<i64> {
    let lower = file.to_lowercase();
    let name_at = lower.rfind('/').map_or(0, |at| at + 1);
    if lower[name_at..].contains(query) {
        return Some(1_000 - i64::try_from(lower.len()).unwrap_or(i64::MAX).min(999));
    }
    if lower.contains(query) {
        return Some(500 - i64::try_from(lower.len()).unwrap_or(i64::MAX).min(499));
    }
    let (mut at, mut gaps) = (0, 0i64);
    for wanted in query.chars() {
        let found = lower[at..].find(wanted)?;
        gaps += i64::try_from(found).unwrap_or(i64::MAX / 2);
        at += found + wanted.len_utf8();
    }
    Some(100 - gaps.min(99))
}

/// The project's files, as git sees them (tracked and new, not ignored).
pub fn list(cwd: &Path) -> Vec<String> {
    let output = std::process::Command::new("git")
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .current_dir(cwd)
        .output();
    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).lines().take(50_000).map(str::to_owned).collect()
        }
        _ => Vec::new(),
    }
}

/// `prompt` with the text of each `@file` it names after it, so the agent
/// has it without reading it first.
pub fn with_files(cwd: &Path, prompt: &str) -> String {
    let mut out = prompt.to_owned();
    let mut seen = Vec::new();
    for word in prompt.split_whitespace() {
        let Some(path) = word.strip_prefix('@').map(|p| p.trim_end_matches([',', '.', ':', ';', ')', '?', '!'])) else {
            continue;
        };
        if path.is_empty() || seen.contains(&path) || seen.len() >= 5 {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(cwd.join(path)) else { continue };
        seen.push(path);
        let kept: String = text.chars().take(KEPT).collect();
        let cut = if kept.len() < text.len() { "\n[… cut]" } else { "" };
        out.push_str(&format!("\n\n<file path=\"{path}\">\n{kept}{cut}\n</file>"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_at_word_at_the_cursor_is_completed() {
        assert_eq!(typing("look at @src/ma", 15), Some((8, "src/ma")));
        assert_eq!(typing("@", 1), Some((0, "")));
        assert_eq!(typing("mail me@home now", 16), None);
        assert_eq!(typing("plain words", 11), None);
    }

    #[test]
    fn the_file_s_own_name_ranks_first_then_letters_in_order() {
        let files: Vec<String> =
            ["docs/main-notes.md", "src/main.rs", "src/tui/mod.rs", "crates/x/src/domain.rs", "README.md"]
                .map(str::to_owned)
                .to_vec();
        assert_eq!(rank("main", &files)[..2], ["src/main.rs", "docs/main-notes.md"]);
        assert_eq!(rank("stm", &files)[0], "src/tui/mod.rs", "letters in order, the closest first");
        assert!(rank("zzz", &files).is_empty());
    }

    #[test]
    fn mentioned_files_go_with_the_prompt() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("a.rs"), "fn a() {}").expect("write");
        let sent = with_files(dir.path(), "why is @a.rs empty? and @missing.rs");
        assert_eq!(sent, "why is @a.rs empty? and @missing.rs\n\n<file path=\"a.rs\">\nfn a() {}\n</file>");
    }
}
