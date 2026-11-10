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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_move_to_the_next_line_and_long_ones_break() {
        assert_eq!(wrap("the quick brown fox", 9), ["the quick", "brown fox"]);
        assert_eq!(wrap("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        assert_eq!(wrap("one\n\ntwo", 10), ["one", "", "two"]);
        assert_eq!(wrap("日本語の文", 4), ["日本", "語の", "文"], "wide characters take two columns");
    }
}
