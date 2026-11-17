use std::path::Path;

const OFFERED: usize = 8;

pub(super) fn offered(app: &super::app::App) -> Vec<String> {
    if app.prompt.input.browsing_history() {
        return Vec::new();
    }
    let Some((_, query)) = typing(app.prompt.input.text(), app.prompt.input.cursor()) else { return Vec::new() };
    app.catalog
        .files
        .as_deref()
        .map(|files| rank(query, files).into_iter().map(str::to_owned).collect())
        .unwrap_or_default()
}

pub(super) fn load(app: &mut super::app::App) {
    if app.catalog.files.is_none() && typing(app.prompt.input.text(), app.prompt.input.cursor()).is_some() {
        app.catalog.files = Some(Vec::new());
        let cwd = app.cwd.clone();
        app.spawn(move |client| super::app::Msg::Files(list(client, &cwd)));
    }
}

pub(super) fn complete(app: &mut super::app::App, file: &str) {
    complete_input(&mut app.prompt.input, file);
}

pub fn complete_input(input: &mut super::input::Input, file: &str) {
    if input.browsing_history() {
        return;
    }
    let Some((start, _)) = typing(input.text(), input.cursor()) else { return };
    input.complete_word(start, &format!("@{file}"));
}

pub fn typing(text: &str, cursor: usize) -> Option<(usize, &str)> {
    let before = &text[..cursor];
    let start = super::input::after_last_whitespace(before);
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

pub fn list(client: &app::Client, cwd: &Path) -> Vec<String> {
    client.project_files(cwd).unwrap_or_default()
}

pub fn names_a_file(prompt: &str) -> bool {
    prompt.contains('@')
}

pub fn with_files(client: &app::Client, cwd: &Path, prompt: &str) -> String {
    if !names_a_file(prompt) {
        return prompt.to_owned();
    }
    client.with_mentioned_files(cwd, prompt).unwrap_or_else(|_| prompt.to_owned())
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
    fn a_multibyte_space_before_the_at_word_still_ends_the_word_before_it() {
        assert_eq!(typing("olha\u{a0}@src", 10), Some((6, "src")));
        assert_eq!(typing("見て\u{3000}@src", 13), Some((9, "src")));
        assert_eq!(typing("olha\u{a0}src", 9), None);
    }

    #[test]
    fn completing_a_mention_keeps_the_rest_of_the_prompt_as_typed() {
        let rest = "  keep  two\n  indented";
        let mut input = crate::tui::input::Input::default();
        input.set(&format!("see @ma{rest}"));
        for _ in rest.chars() {
            input.left();
        }
        complete_input(&mut input, "src/main.rs");
        assert_eq!(input.text(), "see @src/main.rs keep  two\n  indented");
        assert_eq!(&input.text()[..input.cursor()], "see @src/main.rs ", "the cursor stays after the mention");
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
