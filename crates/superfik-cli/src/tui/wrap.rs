//! Breaking text into lines that fit a width, by words, counting what a
//! character takes on screen (wide characters take two columns).

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        let mut used = 0;
        for word in paragraph.split_inclusive(' ') {
            let size = word.trim_end().width();
            if used > 0 && used + size > width {
                lines.push(std::mem::take(&mut line).trim_end().to_owned());
                used = 0;
            }
            if size > width {
                for c in word.chars() {
                    let w = c.width().unwrap_or(0);
                    if used + w > width {
                        lines.push(std::mem::take(&mut line));
                        used = 0;
                    }
                    line.push(c);
                    used += w;
                }
            } else {
                line.push_str(word);
                used += word.width();
            }
        }
        lines.push(line.trim_end().to_owned());
    }
    lines
}

/// The prompt being typed broken into rows of `width` columns, by
/// character, and the row and column of the byte offset `cursor`.
pub fn layout(text: &str, cursor: usize, width: usize) -> (Vec<String>, (usize, usize)) {
    let width = width.max(1);
    let mut rows = vec![String::new()];
    let mut used = 0;
    let mut at = (0, 0);
    for (offset, c) in text.char_indices() {
        if offset == cursor {
            at = (rows.len() - 1, used);
        }
        if c == '\n' {
            rows.push(String::new());
            used = 0;
            continue;
        }
        let w = c.width().unwrap_or(0);
        if used + w > width {
            rows.push(String::new());
            used = 0;
            if offset == cursor {
                at = (rows.len() - 1, 0);
            }
        }
        if let Some(row) = rows.last_mut() {
            row.push(c);
        }
        used += w;
    }
    if cursor >= text.len() {
        at = if used >= width { (rows.len(), 0) } else { (rows.len() - 1, used) };
        if used >= width {
            rows.push(String::new());
        }
    }
    (rows, at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_prompt_wraps_and_the_cursor_follows() {
        assert_eq!(layout("", 0, 5), (vec![String::new()], (0, 0)));
        assert_eq!(layout("abcdefg", 7, 5), (vec!["abcde".into(), "fg".into()], (1, 2)));
        assert_eq!(layout("abcde", 5, 5), (vec!["abcde".into(), String::new()], (1, 0)), "a full row moves on");
        assert_eq!(layout("ab\ncd", 1, 5), (vec!["ab".into(), "cd".into()], (0, 1)));
        assert_eq!(layout("abcdefg", 5, 5).1, (1, 0), "on the first character of a wrapped row");
    }

    #[test]
    fn words_move_to_the_next_line_and_long_ones_break() {
        assert_eq!(wrap("the quick brown fox", 9), ["the quick", "brown fox"]);
        assert_eq!(wrap("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        assert_eq!(wrap("one\n\ntwo", 10), ["one", "", "two"]);
        assert_eq!(wrap("日本語の文", 4), ["日本", "語の", "文"], "wide characters take two columns");
    }
}
