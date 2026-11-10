//! How each part of the conversation is drawn, and the line that says
//! what the agent is doing now.

use std::time::Instant;

use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};

use super::markdown;
use super::transcript::{Entry, ToolState, Transcript, call};
use super::wrap::wrap;

const SPINNER: [&str; 6] = ["·", "✢", "✳", "✶", "✻", "✽"];
/// Lines of reasoning shown while the model thinks.
const THINKING_LINES: usize = 3;
/// Steps of a sub-agent shown under it.
const AGENT_STEPS: usize = 3;
/// Lines of a tool's output shown under it.
const OUTPUT_LINES: usize = 3;
/// Lines of a file change shown under it.
const DIFF_LINES: usize = 20;
const ADDED: Color = Color::Indexed(22);
const REMOVED: Color = Color::Indexed(52);

pub fn lines(entry: &Entry, width: usize, out: &mut Vec<Line<'static>>) {
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
            if !done {
                let wrapped = wrap(text.trim(), width.saturating_sub(2));
                let skip = wrapped.len().saturating_sub(THINKING_LINES);
                for line in wrapped.into_iter().skip(skip) {
                    out.push(Line::styled(format!("  {line}"), dim.add_modifier(Modifier::ITALIC)));
                }
            }
        }
        Entry::Assistant(text) => {
            out.push(Line::from(""));
            let mut code = false;
            for (i, line) in wrap(text.trim(), width.saturating_sub(2)).into_iter().enumerate() {
                if line.trim_start().starts_with("```") {
                    code = !code;
                    continue;
                }
                let mut spans = vec![Span::raw(if i == 0 { "● " } else { "  " })];
                if code {
                    spans.push(Span::styled("│ ", dim));
                    spans.push(Span::raw(line).fg(Color::Cyan));
                } else {
                    spans.extend(markdown::line(&line));
                }
                out.push(Line::from(spans));
            }
        }
        Entry::Tool { name, summary, state, output, .. } => {
            let (mark, color) = match state {
                ToolState::Running => ("● ", Color::Yellow),
                ToolState::Done => ("● ", Color::Green),
                ToolState::Failed => ("● ", Color::Red),
            };
            out.push(Line::from(""));
            let first = wrap(&call(name, summary), width.saturating_sub(2)).into_iter().next().unwrap_or_default();
            out.push(Line::from(vec![mark.fg(color), first.bold()]));
            match output.as_deref().map(str::trim_end).filter(|o| !o.trim().is_empty()) {
                Some(output) if is_diff(output) => diff(output, width, out),
                Some(output) => {
                    let output = output.trim();
                    let shown: Vec<&str> = output.lines().take(OUTPUT_LINES).collect();
                    let more = output.lines().count().saturating_sub(shown.len());
                    for (i, line) in shown.iter().enumerate() {
                        let text = wrap(line, width.saturating_sub(4)).into_iter().next().unwrap_or_default();
                        out.push(Line::styled(format!("{}{text}", if i == 0 { "  ⎿ " } else { "    " }), dim));
                    }
                    if more > 0 {
                        out.push(Line::styled(format!("    … +{more} lines"), dim));
                    }
                }
                None if *state == ToolState::Running => out.push(Line::styled("  ⎿ Running…", dim)),
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
            let skip = steps.len().saturating_sub(AGENT_STEPS);
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
        Entry::Notice(text) => {
            out.push(Line::from(""));
            out.extend(wrap(text, width).into_iter().map(|line| Line::styled(line, dim)));
        }
        Entry::Error(text) => {
            out.push(Line::from(""));
            out.extend(wrap(text, width).into_iter().map(|line| Line::from(line).fg(Color::Red)));
        }
    }
}

/// A file change as its tool shows it: a heading, then `   12 + text`.
fn is_diff(output: &str) -> bool {
    output.lines().nth(1).is_some_and(|line| {
        let rest = line.trim_start();
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        digits > 0 && matches!(rest[digits..].get(..2), Some(" +" | " -" | "  "))
    })
}

fn diff(output: &str, width: usize, out: &mut Vec<Line<'static>>) {
    let dim = Style::new().fg(Color::DarkGray);
    let mut lines = output.lines();
    if let Some(heading) = lines.next() {
        out.push(Line::styled(format!("  ⎿ {heading}"), dim));
    }
    let body: Vec<&str> = lines.collect();
    for line in body.iter().take(DIFF_LINES) {
        let rest = line.trim_start();
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let (number, mark, text) =
            (&rest[..digits], rest[digits..].chars().nth(1).unwrap_or(' '), rest.get(digits + 3..).unwrap_or(""));
        let style = match mark {
            '+' => Style::new().bg(ADDED),
            '-' => Style::new().bg(REMOVED),
            _ => Style::new(),
        };
        let text = wrap(text, width.saturating_sub(12)).into_iter().next().unwrap_or_default();
        let fill = width.saturating_sub(12 + unicode_width::UnicodeWidthStr::width(text.as_str()));
        out.push(Line::from(vec![
            Span::styled(format!("    {number:>5} "), dim),
            Span::styled(format!("{mark} {text}{}", " ".repeat(fill)), style),
        ]));
    }
    if body.len() > DIFF_LINES {
        out.push(Line::styled(format!("          … +{} lines", body.len() - DIFF_LINES), dim));
    }
}

/// `✻ Read(src/main.rs)… (12s · ↓ 1.2k tokens · esc to interrupt)`
pub fn working(started: Instant, transcript: &Transcript, width: usize) -> Line<'static> {
    let tick = usize::try_from(started.elapsed().as_millis() / 120).unwrap_or(0);
    let doing = transcript.activity.clone().unwrap_or_else(|| "Working".into());
    let doing = wrap(&doing, width.saturating_sub(40).max(10)).into_iter().next().unwrap_or_default();
    let mut detail = format!("{}s", started.elapsed().as_secs());
    if transcript.written > 0 {
        detail.push_str(&format!(" · ↓ {} tokens", tokens(transcript.written)));
    }
    Line::from(vec![
        Span::raw(format!("{} ", SPINNER[tick % SPINNER.len()])).fg(Color::Magenta),
        Span::raw(format!("{doing}…")).fg(Color::Magenta),
        format!(" ({detail} · esc to interrupt)").dim(),
    ])
}

/// `950`, `1.2k`, `34k`.
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
    fn a_file_change_draws_as_a_diff() {
        let output = "Edited f.rs · Added 1 lines, removed 1\n    3 -     let x = 1;\n    3 +     let x = 2;";
        assert!(is_diff(output));
        assert!(!is_diff("Edited f.rs.") && !is_diff("line one\nline two"));
        let mut lines = Vec::new();
        diff(output, 60, &mut lines);
        let text: Vec<String> = lines.iter().map(|l| l.spans.iter().map(|s| s.content.to_string()).collect()).collect();
        assert!(text[1].contains("3 -     let x = 1;") && text[2].contains("3 +     let x = 2;"), "{text:?}");
        assert_eq!(lines[2].spans[1].style.bg, Some(ADDED));
    }

    #[test]
    fn token_counts_read_short() {
        assert_eq!([tokens(950), tokens(1_234), tokens(34_567)], ["950", "1.2k", "34k"]);
    }
}
