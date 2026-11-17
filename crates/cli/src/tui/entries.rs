use std::time::Instant;

use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};

use super::diffs;
use super::markdown;
use super::stopwatch::Stopwatch;
use super::transcript::{Entry, ToolState, Transcript, call};
use super::wrap::wrap;

const SPINNER: [&str; 6] = ["·", "✢", "✳", "✶", "✻", "✽"];
const THINKING_LINES: usize = 3;
const AGENT_STEPS: usize = 3;
const OUTPUT_LINES: usize = 3;

#[allow(
    clippy::too_many_lines,
    reason = "split planned: one match over every entry kind, to become a renderer per kind"
)]
pub fn lines(entry: &Entry, width: usize, expanded: bool, out: &mut Vec<Line<'static>>) {
    let dim = Style::new().fg(Color::DarkGray);
    match entry {
        Entry::User(text) => {
            out.push(Line::from(""));
            for (i, line) in wrap(text, width.saturating_sub(2)).into_iter().enumerate() {
                out.push(Line::from(vec![if i == 0 { "> " } else { "  " }.fg(Color::Magenta), line.bold()]));
            }
        }
        Entry::Thinking { text, done } => {
            out.push(Line::from(""));
            let header = if *done { "✻ Thought" } else { "✻ Thinking…" };
            out.push(Line::styled(header, dim.add_modifier(Modifier::ITALIC)));
            if !done || expanded {
                let wrapped = wrap(text.trim(), width.saturating_sub(2));
                let skip = if expanded { 0 } else { wrapped.len().saturating_sub(THINKING_LINES) };
                for line in wrapped.into_iter().skip(skip) {
                    out.push(Line::styled(format!("  {line}"), dim.add_modifier(Modifier::ITALIC)));
                }
            }
        }
        Entry::Assistant(text) => {
            out.push(Line::from(""));
            for (i, line) in markdown::render(text.trim(), width.saturating_sub(2)).into_iter().enumerate() {
                let mut spans = vec![Span::raw(if i == 0 { "● " } else { "  " })];
                let style = line.style;
                spans.extend(line.spans.into_iter().map(|span| Span::styled(span.content, style.patch(span.style))));
                out.push(Line::from(spans));
            }
        }
        Entry::Tool { name, summary, state, output, .. } => {
            let (mark, color) = match state {
                ToolState::Running => ("○ ", Color::Cyan),
                ToolState::Done => ("● ", Color::Green),
                ToolState::Failed => ("● ", Color::Red),
            };
            out.push(Line::from(""));
            let first = wrap(&call(name, summary), width.saturating_sub(2)).into_iter().next().unwrap_or_default();
            out.push(Line::from(vec![mark.fg(color), first.bold()]));
            match output.as_deref().map(str::trim_end).filter(|o| !o.trim().is_empty()) {
                Some(output) if diffs::is_edit(output) => diffs::edit(output, width, expanded, out),
                Some(output) => {
                    let output = output.trim();
                    let shown: Vec<&str> =
                        output.lines().take(if expanded { usize::MAX } else { OUTPUT_LINES }).collect();
                    let more = output.lines().count().saturating_sub(shown.len());
                    for (i, line) in shown.iter().enumerate() {
                        let text = wrap(line, width.saturating_sub(4)).into_iter().next().unwrap_or_default();
                        out.push(Line::styled(format!("{}{text}", if i == 0 { "  ⎿ " } else { "    " }), dim));
                    }
                    if more > 0 {
                        out.push(Line::styled(format!("    … +{more} lines ({})", diffs::EXPAND), dim));
                    }
                }
                None => {}
            }
        }
        Entry::Agent { kind, summary, done, steps, .. } => {
            let color = match done {
                None => Color::Yellow,
                Some(true) => Color::Green,
                Some(false) => Color::Red,
            };
            out.push(Line::from(""));
            out.push(Line::from(vec!["● ".fg(color), kind.clone().bold(), format!("({summary})").into()]));
            let skip = if expanded { 0 } else { steps.len().saturating_sub(AGENT_STEPS) };
            for (i, step) in steps.iter().skip(skip).enumerate() {
                let text = wrap(step, width.saturating_sub(4)).into_iter().next().unwrap_or_default();
                out.push(Line::styled(format!("{}{text}", if i == 0 { "  ⎿ " } else { "    " }), dim));
            }
            match done {
                Some(_) if !steps.is_empty() => {
                    out.push(Line::styled(format!("    Done ({} tool uses)", steps.len()), dim));
                }
                None if skip > 0 => out.push(Line::styled(format!("    +{skip} more tool uses"), dim)),
                _ => {}
            }
        }
        Entry::Diff { diff, untracked } => {
            diffs::unified(diff, width, expanded, out);
            if !untracked.is_empty() {
                out.push(Line::from(""));
                out.push(Line::styled(format!("● New, not yet in git: {}", untracked.join(", ")), dim));
            }
        }
        Entry::Plan { plan, outcome } => super::plan::lines(plan, *outcome, width, out),
        Entry::Refused { .. }
        | Entry::Repeated { .. }
        | Entry::Attached { .. }
        | Entry::Scheduled { .. }
        | Entry::Rewound { .. }
        | Entry::Branched
        | Entry::HeadsUp { .. }
        | Entry::Recap(_)
        | Entry::Lesson { .. }
        | Entry::Held { .. } => {}
        Entry::Notice(text) | Entry::Passing(text) => said(text, width, dim, out),
        Entry::Finished { duration_ms, cost_usd } => {
            said(&domain::conversation::done(*duration_ms, *cost_usd), width, dim, out);
        }
        Entry::Interrupted => said(domain::conversation::INTERRUPTED, width, dim, out),
        Entry::Compacted { freed } => said(&domain::conversation::compacted(*freed), width, dim, out),
        Entry::Error(text) => {
            out.push(Line::from(""));
            for (i, line) in wrap(text, width.saturating_sub(2)).into_iter().enumerate() {
                let mut spans = vec![if i == 0 { "✗ ".fg(Color::Red).bold() } else { "  ".into() }];
                match line.split_once(": ").filter(|(label, _)| i == 0 && label.len() <= 48) {
                    Some((label, detail)) => {
                        spans.extend([format!("{label}:").fg(Color::Red).bold(), format!(" {detail}").into()]);
                    }
                    None => spans.push(line.into()),
                }
                out.push(Line::from(spans));
            }
        }
    }
}

fn said(text: &str, width: usize, dim: Style, out: &mut Vec<Line<'static>>) {
    out.push(Line::from(""));
    out.extend(wrap(text, width).into_iter().map(|line| notice(line, dim)));
}

fn notice(line: String, dim: Style) -> Line<'static> {
    if let Some(rest) = line.strip_prefix("✓ ") {
        return Line::from(vec!["✓ ".green(), Span::styled(rest.to_owned(), dim)]);
    }
    match line.split_once(' ') {
        Some((command, rest)) if command.len() > 1 && command.starts_with('/') && !command[1..].contains('/') => {
            Line::from(vec![command.to_owned().cyan(), Span::styled(format!(" {rest}"), dim)])
        }
        _ => Line::styled(line, dim),
    }
}

pub fn working(started: Stopwatch, transcript: &Transcript, width: usize) -> Line<'static> {
    let elapsed = started.elapsed(Instant::now());
    let tick = usize::try_from(elapsed.as_millis() / 120).unwrap_or(0);
    let shown_above = matches!(
        transcript.entries.last(),
        Some(Entry::Tool { name, summary, state: ToolState::Running, .. })
            if transcript.activity.as_deref() == Some(call(name, summary).as_str())
    );
    let doing = transcript.activity.clone().filter(|_| !shown_above).unwrap_or_else(|| "Working".into());
    let doing = wrap(&doing, width.saturating_sub(40).max(10)).into_iter().next().unwrap_or_default();
    let mut detail = format!("{}s", elapsed.as_secs());
    if transcript.written > 0 {
        detail.push_str(&format!(" · ↓ {} tokens", tokens(transcript.written)));
    }
    Line::from(vec![
        Span::raw(format!("{} ", SPINNER[tick % SPINNER.len()])).fg(Color::Magenta),
        Span::raw(format!("{doing}…")).fg(Color::Magenta),
        format!(" ({detail} · esc to interrupt)").dim(),
    ])
}

pub fn tokens(count: u64) -> String {
    match count {
        0..1_000 => count.to_string(),
        1_000..10_000 => format!("{:.1}k", count as f64 / 1_000.0),
        _ => format!("{}k", count / 1_000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_counts_read_short() {
        assert_eq!([tokens(950), tokens(1_234), tokens(34_567)], ["950", "1.2k", "34k"]);
    }

    #[test]
    fn a_notice_that_opens_with_a_command_shows_the_command_apart() {
        let dim = Style::new().fg(Color::DarkGray);
        let line = notice("/retry sends it again".into(), dim);
        assert_eq!(line.spans[0].content, "/retry");
        assert_eq!(line.spans[0].style.fg, Some(Color::Cyan));
        assert_eq!(notice("/usr/bin is a path".into(), dim).spans.len(), 1, "a path is not a command");
        assert_eq!(notice("plain words".into(), dim).spans.len(), 1);
    }
}
