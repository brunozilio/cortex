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
    let boxed = app.ask.is_some() || app.secret.is_some() || app.picker.is_some();
    let frame_rows = if boxed { 2 } else { 0 };
    let panel_height = (u16::try_from(panel.len()).unwrap_or(u16::MAX) + frame_rows).min(area.height / 2);
    let [body, panel_area, input_area, status_area] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(panel_height),
        Constraint::Length(input_lines + 2),
        Constraint::Length(1),
    ])
    .areas(area);
    conversation(frame, app, body);
    if boxed {
        let color = if app.picker.is_some() { Color::DarkGray } else { Color::Yellow };
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(color))
            .padding(ratatui::widgets::Padding::horizontal(1));
        frame.render_widget(Paragraph::new(panel).block(block), panel_area);
    } else {
        frame.render_widget(Paragraph::new(panel), panel_area);
    }
    prompt(frame, app, input_area);
    frame.render_widget(Paragraph::new(status(app)), status_area);
}

fn conversation(frame: &mut Frame, app: &App, area: Rect) {
    let width = usize::from(area.width.saturating_sub(2)).max(10);
    let mut lines: Vec<Line> = Vec::new();
    lines.extend(welcome(app, width));
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
        let found = super::commands::suggestions(typed, &app.skills);
        let chosen = app.suggestion % found.len().max(1);
        for (i, (name, about)) in found.into_iter().enumerate() {
            let style = if i == chosen { Style::new().fg(Color::Cyan) } else { Style::new() };
            let about = wrap(&about, width.saturating_sub(24)).into_iter().next().unwrap_or_default();
            lines.push(Line::from(vec![Span::styled(format!("  /{name:<20}"), style), about.dim()]));
        }
    }
    lines
}

/// The end of `text` that fits in `columns`, marked `…` when cut.
fn tail(text: &str, columns: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= columns {
        return text.to_owned();
    }
    let kept: String = chars[chars.len() - columns.saturating_sub(1)..].iter().collect();
    format!("…{kept}")
}

/// The box the conversation starts with.
fn welcome(app: &App, width: usize) -> Vec<Line<'static>> {
    let inner = width.clamp(30, 64) - 2;
    let home = std::env::var("HOME").unwrap_or_default();
    let cwd = app.cwd.display().to_string();
    let cwd = match cwd.strip_prefix(&home) {
        Some(rest) if !home.is_empty() => format!("~{rest}"),
        _ => cwd,
    };
    let row = |text: String, style: Style| {
        let text = wrap(&text, inner - 2).into_iter().next().unwrap_or_default();
        let fill = inner.saturating_sub(2 + unicode_width::UnicodeWidthStr::width(text.as_str()));
        Line::from(vec![
            "│ ".fg(Color::Magenta),
            Span::styled(text, style),
            Span::raw(" ".repeat(fill)),
            " │".fg(Color::Magenta),
        ])
    };
    let border = |left: &str, right: &str| Line::from(format!("{left}{}{right}", "─".repeat(inner))).fg(Color::Magenta);
    vec![
        border("╭", "╮"),
        row("✻ Welcome to Superfik".into(), Style::new().add_modifier(Modifier::BOLD)),
        row(String::new(), Style::new()),
        row("/help for commands, /login to connect a provider".into(), Style::new().fg(Color::DarkGray)),
        row(format!("cwd: {}", tail(&cwd, inner.saturating_sub(8))), Style::new().fg(Color::DarkGray)),
        border("╰", "╯"),
        Line::from(""),
        Line::from(" Tips: Shift+Tab changes the mode · ↑ recalls earlier prompts · superfik -c continues").dim(),
    ]
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
    if app.input.is_empty() && app.ask.is_none() && app.secret.is_none() {
        let hint = if app.busy() {
            "Type to queue a message · esc interrupts"
        } else {
            "Try \"explain this project\" · / for commands"
        };
        let placeholder = Line::from(vec!["> ".fg(Color::Magenta), hint.dim()]);
        frame.render_widget(Paragraph::new(placeholder).block(block), area);
    } else {
        frame.render_widget(Paragraph::new(shown).block(block), area);
    }
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
    if app.clear_armed.is_some_and(|at| at.elapsed() < std::time::Duration::from_secs(2)) {
        return Line::from(" Press Esc again to clear the prompt").fg(Color::Yellow);
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
    if !app.transcript.background.is_empty() {
        let count = app.transcript.background.len();
        spans.push(format!(" · {count} background").fg(Color::Cyan));
    }
    if app.transcript.cost > 0.0 {
        spans.push(format!(" · ${:.2}", app.transcript.cost).dim());
    }
    spans.push("  shift+tab mode · /help".dim());
    Line::from(spans)
}
