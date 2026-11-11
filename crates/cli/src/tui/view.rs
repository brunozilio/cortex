use domain::command::mode_label;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};
use sdk::PermissionMode;

use super::app::App;
use super::entries;
use super::motion::Scene;
use super::wrap::{layout, wrap};

const PROMPT_MARK: usize = 2;

pub fn draw(frame: &mut Frame, app: &App) {
    if let Some(diffs) = &app.diffs
        && app.ask.is_none()
        && app.secret.is_none()
    {
        return super::diff_view::draw(frame, diffs);
    }
    let area = frame.area();
    let panel = panel(app, area.width);
    let input_width = usize::from(area.width.saturating_sub(2)).saturating_sub(PROMPT_MARK).max(1);
    let (rows, _) = layout(app.input.text(), app.input.cursor(), input_width);
    let input_lines = u16::try_from(rows.len()).unwrap_or(u16::MAX).clamp(1, 10);
    let boxed = app.ask.is_some() || app.secret.is_some() || app.picker.is_some() || app.side.is_some();
    let frame_rows = if boxed { 2 } else { 0 };
    let panel_height = (u16::try_from(panel.len()).unwrap_or(u16::MAX) + frame_rows).min(area.height / 2);
    let [body, panel_area, input_area, status_area] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(panel_height),
        Constraint::Length(input_lines + 2),
        Constraint::Length(1),
    ])
    .areas(area);
    let shown = conversation(frame, app, body);
    if boxed {
        let color = if app.picker.is_some() {
            Color::DarkGray
        } else if app.ask.is_some() || app.secret.is_some() {
            Color::Yellow
        } else {
            Color::Cyan
        };
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
    let mut motion = app.motion.borrow_mut();
    motion.frame(Scene {
        conversation: body,
        entries: app.transcript.entries.len(),
        added: shown.added,
        panel: (panel_height > 0).then_some(panel_area),
        working: shown.working,
    });
    motion.apply(frame.buffer_mut());
}

struct Shown {
    added: Option<Rect>,
    working: Option<Rect>,
}

fn conversation(frame: &mut Frame, app: &App, area: Rect) -> Shown {
    let width = usize::from(area.width.saturating_sub(2)).max(10);
    let seen = app.motion.borrow().seen();
    let mut lines: Vec<Line> = Vec::new();
    if app.inline.is_none() {
        lines.extend(welcome(app, width));
    }
    let mut added = None;
    for (i, entry) in app.transcript.entries.iter().enumerate().skip(app.inline.unwrap_or(0)) {
        if i == seen {
            added = Some(lines.len());
        }
        entries::lines(entry, width, app.expanded, &mut lines);
    }
    let added = added.map(|first| first..lines.len());
    let mut working = None;
    if let Some(started) = app.started {
        lines.push(Line::from(""));
        working = Some(lines.len()..lines.len() + 1);
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
    let inner = area.inner(ratatui::layout::Margin { horizontal: 1, vertical: 0 });
    frame.render_widget(Paragraph::new(shown), inner);
    let on_screen = |rows: std::ops::Range<usize>| {
        let (top, bottom) = (rows.start.max(start), rows.end.min(end));
        (top < bottom).then(|| {
            let y = inner.y + u16::try_from(top - start).unwrap_or(0);
            Rect::new(inner.x, y, inner.width, u16::try_from(bottom - top).unwrap_or(0))
        })
    };
    Shown { added: added.and_then(on_screen), working: working.and_then(on_screen) }
}

use super::panel::panel;

fn tail(text: &str, columns: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= columns {
        return text.to_owned();
    }
    let kept: String = chars[chars.len() - columns.saturating_sub(1)..].iter().collect();
    format!("…{kept}")
}

pub(super) fn welcome(app: &App, width: usize) -> Vec<Line<'static>> {
    let home = std::env::var("HOME").unwrap_or_default();
    let cwd = app.cwd.display().to_string();
    let cwd = match cwd.strip_prefix(&home) {
        Some(rest) if !home.is_empty() => format!("~{rest}"),
        _ => cwd,
    };
    vec![
        Line::from(""),
        Line::from(vec![
            concat!(" ", brand::name!()).bold(),
            format!(" · {}", tail(&cwd, width.saturating_sub(18))).dim(),
        ]),
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
    if let Some(search) = &app.search {
        let found = search.shown(app.input.history()).unwrap_or("no match").replace('\n', " ");
        let line = Line::from(vec![
            format!("search '{}': ", search.query).fg(Color::Magenta),
            found.into(),
            "  ctrl+r older · enter takes · esc leaves".dim(),
        ]);
        frame.render_widget(Paragraph::new(line).block(block), area);
    } else if app.input.is_empty() && app.ask.is_none() && app.secret.is_none() {
        let hint = match &app.ghost {
            _ if app.busy() && !app.queued.is_empty() => "↑ edits what is queued · esc interrupts".to_owned(),
            _ if app.busy() => "Type to message the agent as it works · esc interrupts".to_owned(),
            Some(ghost) => format!("{ghost}  (tab)"),
            None => "Try \"explain this project\" · / for commands".to_owned(),
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
    if let Some(line) = &app.status.text {
        return Line::from(format!(" {line}")).dim();
    }
    let mut spans = Vec::new();
    if let Some(title) = &app.title {
        spans.extend([format!(" {title}").bold(), " ·".dim()]);
    }
    spans.extend([
        format!(" {model}").fg(Color::Magenta),
        " · ".dim(),
        Span::styled(mode_label(app.mode()), Style::new().fg(mode_color).add_modifier(Modifier::BOLD)),
        " · ".dim(),
        app.effort().cli_value().dim(),
    ]);
    if let Some(criteria) = app.session.as_ref().and_then(sdk::Session::verification) {
        spans.push(" · ".dim());
        spans.push(super::verification::marker(&criteria).fg(Color::Green));
    }
    if app.transcript.context > 0 {
        let window = app.window.or_else(|| app.model().and_then(|model| model.window));
        let shown = super::context::left(app.transcript.context, window)
            .unwrap_or_else(|| format!("{} context", entries::tokens(app.transcript.context)));
        spans.push(format!(" · {shown}").dim());
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
