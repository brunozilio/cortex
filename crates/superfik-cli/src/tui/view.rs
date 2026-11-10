//! Drawing the chat: the conversation, what the agent asks, the picker or
//! completions, the prompt (wrapped as it grows), and a status line.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};
use superfik_core::command::mode_label;
use superfik_sdk::{AskKind, PermissionMode};

use super::app::App;
use super::entries;
use super::wrap::{layout, wrap};

/// Columns the prompt's `> ` takes.
const PROMPT_MARK: usize = 2;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let panel = panel(app, area.width);
    let input_width = usize::from(area.width.saturating_sub(2)).saturating_sub(PROMPT_MARK).max(1);
    let (rows, _) = layout(app.input.text(), app.input.cursor(), input_width);
    let input_lines = u16::try_from(rows.len()).unwrap_or(u16::MAX).clamp(1, 10);
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
        entries::lines(entry, width, &mut lines);
    }
    if let Some(started) = app.started {
        lines.push(Line::from(""));
        lines.push(entries::working(started, &app.transcript, width));
    }
    for queued in &app.queued {
        let text = wrap(queued, width.saturating_sub(4)).into_iter().next().unwrap_or_default();
        lines.push(Line::from(format!("  › {text}  (queued)")).dim());
    }
    let height = usize::from(area.height);
    let scroll = usize::from(app.scroll).min(lines.len().saturating_sub(height));
    let end = lines.len().saturating_sub(scroll);
    let start = end.saturating_sub(height);
    let shown: Vec<Line> = lines.drain(start..end).collect();
    frame.render_widget(Paragraph::new(shown), area.inner(ratatui::layout::Margin { horizontal: 1, vertical: 0 }));
}

/// What sits above the prompt: the question being asked, the picker, or
/// the completions of a `/word`.
fn panel(app: &App, width: u16) -> Vec<Line<'static>> {
    let width = usize::from(width).saturating_sub(4).max(10);
    let mut lines = Vec::new();
    if let Some(secret) = &app.secret {
        lines.push(Line::from(""));
        let dots = "•".repeat(secret.text.chars().count());
        lines.push(Line::from(vec![format!("{} API key: ", secret.provider.name()).bold(), dots.into()]));
        lines.push(Line::from("Paste or type it · Enter connects · Esc cancels").dim());
    } else if let Some(pending) = &app.ask {
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
    let border = if app.ask.is_some() || app.secret.is_some() { Color::Yellow } else { Color::DarkGray };
    let block = Block::bordered().border_type(BorderType::Rounded).border_style(Style::new().fg(border));
    let inner = block.inner(area);
    let width = usize::from(inner.width).saturating_sub(PROMPT_MARK).max(1);
    let (rows, (row, column)) = layout(app.input.text(), app.input.cursor(), width);
    let visible = usize::from(inner.height.max(1));
    let first = (row + 1).saturating_sub(visible);
    let shown: Vec<Line> = rows
        .into_iter()
        .enumerate()
        .skip(first)
        .map(|(i, text)| Line::from(vec![if i == 0 { "> " } else { "  " }.fg(Color::Magenta), text.into()]))
        .collect();
    frame.render_widget(Paragraph::new(shown).block(block), area);
    if app.ask.is_none() && app.picker.is_none() && app.secret.is_none() {
        let x = inner.x.saturating_add(u16::try_from(column + PROMPT_MARK).unwrap_or(u16::MAX));
        let y = inner.y.saturating_add(u16::try_from(row - first).unwrap_or(0));
        frame.set_cursor_position(Position::new(x.min(inner.right().saturating_sub(1)), y));
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
    if app.quitting() {
        return Line::from(" Press Ctrl+C again to quit").fg(Color::Yellow);
    }
    let mut spans = vec![
        format!(" {model}").fg(Color::Magenta),
        " · ".dim(),
        Span::styled(mode_label(app.mode()), Style::new().fg(mode_color).add_modifier(Modifier::BOLD)),
        " · ".dim(),
        app.effort().cli_value().dim(),
    ];
    if app.transcript.context > 0 {
        spans.push(format!(" · {} context", entries::tokens(app.transcript.context)).dim());
    }
    if app.transcript.cost > 0.0 {
        spans.push(format!(" · ${:.2}", app.transcript.cost).dim());
    }
    spans.push("  shift+tab mode · /help".dim());
    Line::from(spans)
}
