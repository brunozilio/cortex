use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use super::diff_model::{self, DiffLine, FileDiff, Kind, Status};
use super::wrap::wrap;

const FOLDED: usize = 20;
const ADDED: Color = Color::Indexed(22);
const REMOVED: Color = Color::Indexed(52);
pub const EXPAND: &str = "ctrl+o to expand";

pub fn is_edit(output: &str) -> bool {
    output.lines().nth(1).is_some_and(|line| {
        let rest = line.trim_start();
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        digits > 0 && matches!(rest[digits..].get(..2), Some(" +" | " -" | "  "))
    })
}

pub fn edit(output: &str, width: usize, expanded: bool, out: &mut Vec<Line<'static>>) {
    let mut lines = output.lines();
    let heading = lines.next();
    if let Some(heading) = heading {
        out.push(Line::styled(format!("  ⎿ {heading}"), dim()));
    }
    let ext = heading.and_then(super::highlight::extension);
    let body: Vec<&str> = lines.collect();
    for line in body.iter().take(if expanded { usize::MAX } else { FOLDED }) {
        let rest = line.trim_start();
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let mark = rest[digits..].chars().nth(1).unwrap_or(' ');
        out.push(row(&rest[..digits], mark, rest.get(digits + 3..).unwrap_or(""), width, ext));
    }
    more(body.len(), expanded, out);
}

pub fn unified(diff: &str, width: usize, expanded: bool, out: &mut Vec<Line<'static>>) {
    for file in diff_model::parse(diff) {
        out.push(Line::from(""));
        out.push(heading(&file));
        if file.status == Status::Binary {
            out.push(Line::styled("    binary file changed", dim()));
        }
        let total: usize = file.hunks.iter().map(|hunk| hunk.lines.len()).sum();
        let mut shown = 0;
        for (i, hunk) in file.hunks.iter().enumerate() {
            if i > 0 && (expanded || shown < FOLDED) {
                out.push(Line::styled("          ⋮", dim()));
            }
            let ext = file.path.rsplit_once('.').map(|(_, ext)| ext);
            for line in &hunk.lines {
                if expanded || shown < FOLDED {
                    out.push(row(&line.number.to_string(), mark(line.kind), &line.text, width, ext));
                    shown += 1;
                }
            }
        }
        more(total, expanded, out);
    }
}

pub fn heading(file: &FileDiff) -> Line<'static> {
    let name = match &file.old_path {
        Some(old) => format!("{} (renamed from {old})", file.path),
        None => file.path.clone(),
    };
    Line::from(vec![Span::styled("● ", Style::new().fg(Color::Cyan)), Span::raw(name)])
}

pub fn wrapped(line: &DiffLine, width: usize, out: &mut Vec<Line<'static>>) {
    let mark = mark(line.kind);
    for (i, text) in wrap(&line.text, width.saturating_sub(12)).iter().enumerate() {
        let (number, shown) = if i == 0 { (line.number.to_string(), mark) } else { (String::new(), ' ') };
        out.push(Line::from(vec![
            Span::styled(format!("    {number:>5} "), dim()),
            Span::styled(format!("{shown} {}", fill(text, width)), colour(mark)),
        ]));
    }
}

const fn mark(kind: Kind) -> char {
    match kind {
        Kind::Added => '+',
        Kind::Removed => '-',
        Kind::Context => ' ',
    }
}

fn colour(mark: char) -> Style {
    match mark {
        '+' => Style::new().bg(ADDED),
        '-' => Style::new().bg(REMOVED),
        _ => Style::new(),
    }
}

fn row(number: &str, mark: char, text: &str, width: usize, ext: Option<&str>) -> Line<'static> {
    let text = wrap(text, width.saturating_sub(12)).into_iter().next().unwrap_or_default();
    let filled = fill(&text, width);
    let mut spans =
        vec![Span::styled(format!("    {number:>5} "), dim()), Span::styled(format!("{mark} "), colour(mark))];
    spans.extend(super::highlight::spans(ext, &text, colour(mark)));
    spans.push(Span::styled(filled[text.len()..].to_owned(), colour(mark)));
    Line::from(spans)
}

fn fill(text: &str, width: usize) -> String {
    let pad = width.saturating_sub(12 + unicode_width::UnicodeWidthStr::width(text));
    format!("{text}{}", " ".repeat(pad))
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
