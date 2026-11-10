//! Drawing the chat: the conversation, what the agent asks, the picker or
//! completions, the prompt, and a status line.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};
use superfik_sdk::{AskKind, PermissionMode};
use unicode_width::UnicodeWidthStr;

use superfik_core::command::mode_label;

use super::app::App;
use super::transcript::{Entry, ToolState};
use super::wrap::wrap;

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let panel = panel(app, area.width);
    let input_lines = u16::try_from(app.input.text().split('\n').count()).unwrap_or(u16::MAX).clamp(1, 8);
    let panel_height = u16::try_from(panel.len()).unwrap_or(u16::MAX).min(area.height / 2);
    let [body, panel_area, input_area, status_area] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(panel_height),
        Constraint::Length(input_lines + 2),
        Constraint::Length(1),
    ])
    .areas(area);
    conversation(frame, app, body);
    frame.render_widget(Paragraph::new(panel), panel_area);
    prompt(frame, app, input_area);
    frame.render_widget(Paragraph::new(status(app)), status_area);
}

fn conversation(frame: &mut Frame, app: &App, area: Rect) {
    let width = usize::from(area.width.saturating_sub(2)).max(10);
    let mut lines: Vec<Line> = Vec::new();
    if app.transcript.entries.is_empty() {
        lines.push(Line::from(vec!["✻ ".fg(Color::Magenta), "Superfik".bold()]));
        lines.push(Line::from(format!("  {}", app.cwd.display())).dim());
        lines.push(Line::from("  /help for commands · Shift+Tab changes the mode").dim());
    }
    for entry in &app.transcript.entries {
        entry_lines(entry, width, &mut lines);
    }
    if let Some(started) = app.started {
        let tick = usize::try_from(started.elapsed().as_millis() / 100).unwrap_or(0);
        let seconds = started.elapsed().as_secs();
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::raw(SPINNER[tick % SPINNER.len()]).fg(Color::Magenta),
            format!(" Working… {seconds}s · esc to interrupt").dim(),
        ]));
    }
    let height = usize::from(area.height);
    let scroll = usize::from(app.scroll).min(lines.len().saturating_sub(height));
    let end = lines.len().saturating_sub(scroll);
    let start = end.saturating_sub(height);
    let shown: Vec<Line> = lines.drain(start..end).collect();
    frame.render_widget(Paragraph::new(shown), area.inner(ratatui::layout::Margin { horizontal: 1, vertical: 0 }));
}

fn entry_lines(entry: &Entry, width: usize, lines: &mut Vec<Line<'static>>) {
    match entry {
        Entry::User(text) => {
            lines.push(Line::from(""));
            for (i, line) in wrap(text, width.saturating_sub(2)).into_iter().enumerate() {
                let mark = if i == 0 { "> " } else { "  " };
                lines.push(Line::from(vec![mark.fg(Color::Magenta), line.bold()]));
            }
        }
        Entry::Assistant(text) => {
            lines.push(Line::from(""));
            let mut code = false;
            for (i, line) in wrap(text.trim(), width.saturating_sub(2)).into_iter().enumerate() {
                let mark = if i == 0 { "● " } else { "  " };
                if line.trim_start().starts_with("```") {
                    code = !code;
                    continue;
                }
                let span = if code {
                    Span::raw(line).fg(Color::Cyan)
                } else if line.starts_with('#') {
                    Span::raw(line.trim_start_matches('#').trim().to_owned()).bold()
                } else {
                    Span::raw(line)
                };
                lines.push(Line::from(vec![Span::raw(mark), span]));
            }
        }
        Entry::Tool { name, summary, state, output, .. } => {
            let (mark, color) = match state {
                ToolState::Running => ("○ ", Color::Yellow),
                ToolState::Done => ("● ", Color::Green),
                ToolState::Failed => ("✗ ", Color::Red),
            };
            let call = if summary.is_empty() { name.clone() } else { format!("{name}({summary})") };
            let first = wrap(&call, width.saturating_sub(2)).into_iter().next().unwrap_or_default();
            lines.push(Line::from(vec![mark.fg(color), first.bold()]));
            if let Some(output) = output.as_ref().filter(|o| !o.trim().is_empty()) {
                let shown: Vec<&str> = output.trim().lines().take(3).collect();
                let more = output.trim().lines().count().saturating_sub(shown.len());
                for (i, line) in shown.iter().enumerate() {
                    let mark = if i == 0 { "  ⎿ " } else { "    " };
                    let text = wrap(line, width.saturating_sub(4)).into_iter().next().unwrap_or_default();
                    lines.push(Line::from(format!("{mark}{text}")).dim());
                }
                if more > 0 {
                    lines.push(Line::from(format!("    … {more} more lines")).dim());
                }
            }
        }
        Entry::Agent { kind, summary, done, .. } => {
            let (mark, color) = match done {
                None => ("◇ ", Color::Yellow),
                Some(true) => ("◆ ", Color::Green),
                Some(false) => ("◆ ", Color::Red),
            };
            lines.push(Line::from(vec![mark.fg(color), kind.clone().bold(), format!(": {summary}").into()]));
        }
        Entry::Notice(text) => {
            for line in wrap(text, width) {
                lines.push(Line::from(line).dim());
            }
        }
        Entry::Error(text) => {
            for line in wrap(text, width) {
                lines.push(Line::from(line).fg(Color::Red));
            }
        }
    }
}

/// What sits above the prompt: the question being asked, the picker, or
/// the completions of a `/word`.
fn panel(app: &App, width: u16) -> Vec<Line<'static>> {
    let width = usize::from(width).saturating_sub(4).max(10);
    let mut lines = Vec::new();
    if let Some(pending) = &app.ask {
        let title = match &pending.ask.kind {
            AskKind::Permission { tool, subject, .. } => format!("Allow {tool}: {subject}?"),
            AskKind::Questions(questions) => {
                questions.get(pending.question).map(|q| q.text.clone()).unwrap_or_default()
            }
        };
        lines.push(Line::from(""));
        for line in wrap(&title, width) {
            lines.push(Line::from(line).bold().fg(Color::Yellow));
        }
        let picked = pending.picked.get(pending.question);
        for (i, choice) in pending.choices().into_iter().enumerate() {
            let check = if picked.is_some_and(|p| p.contains(&i)) { "[x] " } else { "" };
            let line = format!("{} {}. {check}{choice}", if i == pending.cursor { "❯" } else { " " }, i + 1);
            let style = if i == pending.cursor { Style::new().fg(Color::Cyan) } else { Style::new() };
            lines.push(Line::styled(line, style));
        }
        let hint = match &pending.ask.kind {
            AskKind::Permission { .. } => "y yes · a always · n no · esc denies",
            AskKind::Questions(_) => "number or ↑↓ Enter · space toggles a multiple choice · esc skips",
        };
        lines.push(Line::from(hint).dim());
    } else if let Some(picker) = &app.picker {
        lines.push(Line::from(picker.title.clone()).bold());
        for (i, (name, about)) in picker.items.iter().enumerate() {
            let marker = if i == picker.cursor { "❯ " } else { "  " };
            let style = if i == picker.cursor { Style::new().fg(Color::Cyan) } else { Style::new() };
            lines.push(Line::from(vec![Span::styled(format!("{marker}{name}"), style), format!("  {about}").dim()]));
        }
        lines.push(Line::from("↑↓ Enter picks · esc closes").dim());
    } else if let Some(typed) = app.input.slash_word() {
        for (i, (name, about)) in super::commands::suggestions(typed, &app.skills).into_iter().enumerate() {
            let style = if i == 0 { Style::new().fg(Color::Cyan) } else { Style::new() };
            let about = wrap(&about, width.saturating_sub(24)).into_iter().next().unwrap_or_default();
            lines.push(Line::from(vec![Span::styled(format!("  /{name:<20}"), style), about.dim()]));
        }
    }
    lines
}

fn prompt(frame: &mut Frame, app: &App, area: Rect) {
    let border = if app.ask.is_some() { Color::Yellow } else { Color::DarkGray };
    let block = Block::bordered().border_type(BorderType::Rounded).border_style(Style::new().fg(border));
    let inner = block.inner(area);
    let text: Vec<Line> = app.input.text().split('\n').map(|line| Line::from(format!("> {line}"))).collect();
    let before = &app.input.text()[..app.input.cursor()];
    let row = before.matches('\n').count();
    let column = before.rsplit('\n').next().unwrap_or_default().width() + 2;
    let visible = usize::from(inner.height.max(1));
    let first = (row + 1).saturating_sub(visible);
    let shown: Vec<Line> = text.into_iter().skip(first).collect();
    frame.render_widget(Paragraph::new(shown).block(block), area);
    if app.ask.is_none() && app.picker.is_none() {
        let x = inner.x.saturating_add(u16::try_from(column).unwrap_or(u16::MAX)).min(inner.right().saturating_sub(1));
        let y = inner.y.saturating_add(u16::try_from(row - first).unwrap_or(0));
        frame.set_cursor_position(Position::new(x, y));
    }
}

fn status(app: &App) -> Line<'static> {
    let model = match app.model() {
        Some(model) => model.label.clone(),
        None if app.loading => "loading models…".to_owned(),
        None => "no model".to_owned(),
    };
    let mode_color = match app.mode() {
        PermissionMode::Ask => Color::Gray,
        PermissionMode::AcceptEdits => Color::Yellow,
        PermissionMode::Plan => Color::Blue,
        PermissionMode::Bypass => Color::Red,
    };
    Line::from(vec![
        format!(" {model}").fg(Color::Magenta),
        " · ".dim(),
        Span::styled(mode_label(app.mode()), Style::new().fg(mode_color).add_modifier(Modifier::BOLD)),
        " · ".dim(),
        app.effort().cli_value().dim(),
        format!(" · ${:.4}", app.transcript.cost).dim(),
        "  shift+tab mode · /help".dim(),
    ])
}
