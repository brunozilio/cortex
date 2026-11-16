use domain::command::mode_label;
use domain::conversation::Outcome;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::markdown;

pub fn lines(plan: &str, outcome: Outcome, width: usize, out: &mut Vec<Line<'static>>) {
    let (mark, header) = match outcome {
        Outcome::Waiting => (Color::Cyan, "Plan ready for review".to_owned()),
        Outcome::Approved(mode) => (Color::Green, format!("User approved the plan ({} mode)", mode_label(mode))),
        Outcome::KeptPlanning => (Color::Yellow, "Kept planning: the plan was not approved".to_owned()),
    };
    out.push(Line::from(""));
    out.push(Line::from(vec!["● ".fg(mark), header.bold()]));
    let border = Style::new().fg(if outcome == Outcome::Waiting { Color::Cyan } else { Color::DarkGray });
    let inner = width.saturating_sub(6).max(10);
    out.push(Line::styled(format!("  ╭{}╮", "─".repeat(inner + 2)), border));
    for line in markdown::render(plan.trim(), inner) {
        let used: usize = line.spans.iter().map(|span| span.content.width()).sum();
        let style = line.style;
        let mut spans = vec![Span::styled("  │ ", border)];
        spans.extend(line.spans.into_iter().map(|span| Span::styled(span.content, style.patch(span.style))));
        spans.push(Span::raw(" ".repeat(inner.saturating_sub(used))));
        spans.push(Span::styled(" │", border));
        out.push(Line::from(spans));
    }
    out.push(Line::styled(format!("  ╰{}╯", "─".repeat(inner + 2)), border));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plan_is_boxed_with_even_sides() {
        let mut out = Vec::new();
        lines("## Steps\n1. A step long enough to wrap inside the box twice over", Outcome::Waiting, 40, &mut out);
        let rows: Vec<String> =
            out.iter().map(|line| line.spans.iter().map(|span| span.content.as_ref()).collect()).collect();
        assert!(rows[1].contains("Plan ready for review"));
        assert!(rows[2].starts_with("  ╭") && rows.last().is_some_and(|row| row.starts_with("  ╰")));
        let widths: Vec<usize> = rows[2..].iter().map(|row| row.width()).collect();
        assert!(widths.iter().all(|&w| w == widths[0]), "{rows:#?}");
        assert!(rows.iter().any(|row| row.contains("Steps")));
    }
}
