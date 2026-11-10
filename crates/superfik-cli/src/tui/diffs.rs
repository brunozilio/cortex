//! File changes drawn with their line numbers on green and red: an edit as
//! its tool shows it (`   12 + text`), and `git diff` output for `/diff`.
//! Folded to a few lines until Ctrl+O expands them.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use super::wrap::wrap;

/// Lines of a file change shown while folded.
const FOLDED: usize = 20;
const ADDED: Color = Color::Indexed(22);
const REMOVED: Color = Color::Indexed(52);
pub const EXPAND: &str = "ctrl+o to expand";

/// Whether a tool's output is a file change: a heading, then `   12 + text`.
pub fn is_edit(output: &str) -> bool {
    output.lines().nth(1).is_some_and(|line| {
        let rest = line.trim_start();
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        digits > 0 && matches!(rest[digits..].get(..2), Some(" +" | " -" | "  "))
    })
}

pub fn edit(output: &str, width: usize, expanded: bool, out: &mut Vec<Line<'static>>) {
    let mut lines = output.lines();
    if let Some(heading) = lines.next() {
        out.push(Line::styled(format!("  ⎿ {heading}"), dim()));
    }
    let body: Vec<&str> = lines.collect();
    for line in body.iter().take(if expanded { usize::MAX } else { FOLDED }) {
        let rest = line.trim_start();
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let mark = rest[digits..].chars().nth(1).unwrap_or(' ');
        out.push(row(&rest[..digits], mark, rest.get(digits + 3..).unwrap_or(""), width));
    }
    more(body.len(), expanded, out);
}

/// `git diff` output: each file's name, then its hunks numbered by the
/// lines they touch.
pub fn unified(diff: &str, width: usize, expanded: bool, out: &mut Vec<Line<'static>>) {
    let (mut old, mut new, mut shown) = (0usize, 0usize, 0usize);
    let mut file_lines = 0;
    for line in diff.lines() {
        if let Some(header) = line.strip_prefix("diff --git ") {
            more(file_lines.max(shown), expanded, out);
            (file_lines, shown) = (0, 0);
            let name = header.split(" b/").last().unwrap_or(header);
            out.push(Line::from(""));
            out.push(Line::from(vec![Span::styled("● ", Style::new().fg(Color::Cyan)), Span::raw(name.to_owned())]));
            continue;
        }
        if let Some(hunk) = line.strip_prefix("@@ ") {
            (old, new) = starts(hunk);
            if file_lines > 0 && (expanded || shown < FOLDED) {
                out.push(Line::styled("          ⋮", dim()));
            }
            continue;
        }
        let (mark, number) = match line.chars().next() {
            Some('+') if !line.starts_with("+++ ") => ('+', &mut new),
            Some('-') if !line.starts_with("--- ") => ('-', &mut old),
            Some(' ') => {
                old += 1;
                (' ', &mut new)
            }
            Some('B') if line.starts_with("Binary files") => {
                out.push(Line::styled("    binary file changed", dim()));
                continue;
            }
            _ => continue,
        };
        file_lines += 1;
        if expanded || shown < FOLDED {
            out.push(row(&number.to_string(), mark, &line[1..], width));
            shown += 1;
        }
        *number += 1;
    }
    more(file_lines.max(shown), expanded, out);
}

/// Where a hunk starts in the old and new file: `-12,3 +12,4 @@ fn x`.
fn starts(hunk: &str) -> (usize, usize) {
    let mut parts = hunk.split_whitespace();
    let mut start = |prefix: char| {
        parts
            .next()
            .and_then(|part| part.strip_prefix(prefix))
            .and_then(|range| range.split(',').next()?.parse().ok())
            .unwrap_or(1)
    };
    (start('-'), start('+'))
}

fn row(number: &str, mark: char, text: &str, width: usize) -> Line<'static> {
    let style = match mark {
        '+' => Style::new().bg(ADDED),
        '-' => Style::new().bg(REMOVED),
        _ => Style::new(),
    };
    let text = wrap(text, width.saturating_sub(12)).into_iter().next().unwrap_or_default();
    let fill = width.saturating_sub(12 + unicode_width::UnicodeWidthStr::width(text.as_str()));
    Line::from(vec![
        Span::styled(format!("    {number:>5} "), dim()),
        Span::styled(format!("{mark} {text}{}", " ".repeat(fill)), style),
    ])
}

fn more(total: usize, expanded: bool, out: &mut Vec<Line<'static>>) {
    if !expanded && total > FOLDED {
        out.push(Line::styled(format!("          … +{} lines ({EXPAND})", total - FOLDED), dim()));
    }
}

fn dim() -> Style {
    Style::new().fg(Color::DarkGray)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().trim_end().to_owned())
            .collect()
    }

    #[test]
    fn an_edit_shows_numbered_lines_on_their_colours() {
        let output = "Edited f.rs · Added 1 lines, removed 1\n    3 -     let x = 1;\n    3 +     let x = 2;";
        assert!(is_edit(output));
        assert!(!is_edit("Read 3 lines\nfn main() {}"));
        let mut lines = Vec::new();
        edit(output, 60, false, &mut lines);
        assert_eq!(
            text(&lines),
            ["  ⎿ Edited f.rs · Added 1 lines, removed 1", "        3 -     let x = 1;", "        3 +     let x = 2;"]
        );
        assert_eq!(lines[1].spans[1].style.bg, Some(REMOVED));
        assert_eq!(lines[2].spans[1].style.bg, Some(ADDED));
    }

    #[test]
    fn a_long_change_folds_until_expanded() {
        let output = format!(
            "Created a · Wrote 30 lines\n{}",
            (1..=30).map(|n| format!("{n:>5} + x")).collect::<Vec<_>>().join("\n")
        );
        let (mut folded, mut expanded) = (Vec::new(), Vec::new());
        edit(&output, 60, false, &mut folded);
        edit(&output, 60, true, &mut expanded);
        assert_eq!(folded.len(), 1 + FOLDED + 1);
        assert!(text(&folded).last().is_some_and(|l| l.contains("+10 lines (ctrl+o to expand)")));
        assert_eq!(expanded.len(), 31);
    }

    #[test]
    fn git_diff_is_drawn_per_file_with_the_numbers_of_each_side() {
        let diff = "diff --git a/src/a.rs b/src/a.rs\nindex 1..2 100644\n--- a/src/a.rs\n+++ b/src/a.rs\n\
                    @@ -10,3 +10,3 @@ fn a()\n keep\n-old\n+new\n@@ -40 +40,2 @@\n+added\n same\n\
                    diff --git a/logo.png b/logo.png\nBinary files a/logo.png and b/logo.png differ\n";
        let mut lines = Vec::new();
        unified(diff, 60, false, &mut lines);
        assert_eq!(
            text(&lines),
            [
                "",
                "● src/a.rs",
                "       10   keep",
                "       11 - old",
                "       11 + new",
                "          ⋮",
                "       40 + added",
                "       41   same",
                "",
                "● logo.png",
                "    binary file changed",
            ]
        );
    }
}
