use std::path::Path;

const OFFERED: usize = 8;

pub(super) fn offered(app: &super::app::App) -> Vec<String> {
    let Some((_, query)) = typing(app.input.text(), app.input.cursor()) else { return Vec::new() };
    app.files.as_deref().map(|files| rank(query, files).into_iter().map(str::to_owned).collect()).unwrap_or_default()
}

pub(super) fn load(app: &mut super::app::App) {
    if app.files.is_none() && typing(app.input.text(), app.input.cursor()).is_some() {
        app.files = Some(Vec::new());
        let cwd = app.cwd.clone();
        app.spawn(move |sdk| super::app::Msg::Files(list(sdk, &cwd)));
    }
}

pub(super) fn complete(app: &mut super::app::App, file: &str) {
    let (text, cursor) = (app.input.text().to_owned(), app.input.cursor());
    let Some((start, _)) = typing(&text, cursor) else { return };
    let rest = text[cursor..].trim_start();
    let joined = if rest.is_empty() { String::new() } else { format!(" {rest}") };
    app.input.set(&format!("{}@{file} {}", &text[..start], joined.trim_start()).replace("  ", " "));
}

pub fn typing(text: &str, cursor: usize) -> Option<(usize, &str)> {
    let before = &text[..cursor];
    let start = before.rfind(char::is_whitespace).map_or(0, |at| at + 1);
    let word = &before[start..];
    word.strip_prefix('@').map(|rest| (start, rest))
}

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

pub fn list(sdk: &sdk::Sdk, cwd: &Path) -> Vec<String> {
    let listed = sdk.feature("workspace", serde_json::json!({ "op": "files", "path": cwd }));
    listed.ok().and_then(|value| serde_json::from_value(value).ok()).unwrap_or_default()
}

pub fn with_files(sdk: &sdk::Sdk, cwd: &Path, prompt: &str) -> String {
    if !prompt.contains('@') {
        return prompt.to_owned();
    }
    let asked = sdk.feature("workspace", serde_json::json!({ "op": "mentions", "path": cwd, "prompt": prompt }));
    asked.ok().and_then(|value| value.as_str().map(str::to_owned)).unwrap_or_else(|| prompt.to_owned())
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
}
