use app::PermissionMode;
use domain::command::mode_label;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

use std::rc::Rc;

use super::app::App;
use super::entries;
use super::motion::Scene;
use super::rendered::{Lines, Shape};
use super::transcript::Entry;
use super::wrap::{layout, wrap};

const PROMPT_MARK: usize = 2;

pub fn draw(frame: &mut Frame, app: &App) {
    if let Some(diffs) = &app.overlays.diffs
        && app.ask.is_none()
        && app.overlays.secret.is_none()
    {
        return super::diff_view::draw(frame, diffs);
    }
    if app.fleet.open && app.ask.is_none() && app.overlays.secret.is_none() {
        return super::fleet::draw(frame, app);
    }
    let area = frame.area();
    let panel = panel(app, area.width);
    let input_width = usize::from(area.width.saturating_sub(2)).saturating_sub(PROMPT_MARK).max(1);
    let (rows, _) = layout(app.prompt.input.text(), app.prompt.input.cursor(), input_width);
    let input_lines = u16::try_from(rows.len()).unwrap_or(u16::MAX).clamp(1, 10);
    let boxed = app.ask.is_some()
        || app.overlays.secret.is_some()
        || app.overlays.picker.is_some()
        || app.overlays.side.is_some();
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
        let color = if app.overlays.picker.is_some() {
            Color::DarkGray
        } else if app.ask.is_some() || app.overlays.secret.is_some() {
            Color::Yellow
        } else {
            Color::Cyan
        };
        let mut block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(color))
            .padding(ratatui::widgets::Padding::horizontal(1));
        if let (Some(_), Some(started)) = (&app.ask, app.turn.started) {
            let waited = started.elapsed(std::time::Instant::now()).as_secs();
            block =
                block.title_top(Line::from(format!(" the agent waits for your answer · {waited}s ")).right_aligned());
        }
        frame.render_widget(Paragraph::new(panel).block(block), panel_area);
    } else {
        frame.render_widget(Paragraph::new(panel), panel_area);
    }
    prompt(frame, app, input_area);
    status_bar(frame, app, status_area);
    let mut motion = app.ui.motion.borrow_mut();
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
    let mut pieces: Vec<Lines> = Vec::new();
    if app.ui.inline.is_none() {
        pieces.push(Rc::new(welcome(app, width)));
    }
    let before: usize = pieces.iter().map(|piece| piece.len()).sum();
    let (said, added) = transcript(app, width, before, &mut pieces);
    let (after, working) = below_the_transcript(app, width, said);
    pieces.push(Rc::new(after));
    let total: usize = pieces.iter().map(|piece| piece.len()).sum();
    let height = usize::from(area.height);
    let scroll = usize::from(app.ui.scroll).min(total.saturating_sub(height));
    let end = total.saturating_sub(scroll);
    let start = end.saturating_sub(height);
    let shown = window(&pieces, start, end);
    let mut inner = area.inner(ratatui::layout::Margin { horizontal: 1, vertical: 0 });
    let empty = u16::try_from(height.saturating_sub(shown.len())).unwrap_or(0);
    inner.y += empty;
    inner.height -= empty;
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

type Rows = std::ops::Range<usize>;

fn transcript(app: &App, width: usize, before: usize, pieces: &mut Vec<Lines>) -> (usize, Option<Rows>) {
    let seen = app.ui.motion.borrow().seen();
    let shape = Shape { width, expanded: app.ui.expanded, light: super::theme::current().light() };
    let mut rendered = app.ui.rendered.borrow_mut();
    rendered.forget_after(app.transcript.entries.len());
    let (mut rows, mut added) = (before, None);
    for (i, entry) in app.transcript.entries.iter().enumerate().skip(app.ui.inline.unwrap_or(0)) {
        if i == seen {
            added = Some(rows);
        }
        let lines = rendered.lines(i, entry, shape);
        rows += lines.len();
        pieces.push(lines);
    }
    (rows, added.map(|first| first..rows))
}

fn below_the_transcript(app: &App, width: usize, above: usize) -> (Vec<Line<'static>>, Option<Rows>) {
    let mut lines = Vec::new();
    let mut working = None;
    if let Some(started) = app.turn.started
        && app.ask.is_none()
    {
        lines.push(Line::from(""));
        working = Some(above + 1..above + 2);
        lines.push(entries::working(started, &app.transcript, width));
    }
    if let Some(at) = app.turn.auto_retry {
        lines.push(Line::from(""));
        lines.push(super::auto_retry::line(at, std::time::Instant::now()));
    }
    for queued in app.turn.waiting() {
        let text = wrap(queued, width.saturating_sub(4)).into_iter().next().unwrap_or_default();
        lines.push(Line::from(format!("  › {text}  (queued)")).dim());
    }
    (lines, working)
}

fn window(pieces: &[Lines], start: usize, end: usize) -> Vec<Line<'static>> {
    let mut shown = Vec::with_capacity(end.saturating_sub(start));
    let mut at = 0;
    for piece in pieces {
        let (from, to) = (start.max(at), end.min(at + piece.len()));
        if from < to {
            shown.extend_from_slice(&piece[from - at..to - at]);
        }
        at += piece.len();
        if at >= end {
            break;
        }
    }
    shown
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
    let cwd = super::hints::home_relative(&app.cwd, std::env::home_dir().as_deref());
    vec![
        Line::from(""),
        Line::from(vec![brand::name!().bold(), format!(" · {}", tail(&cwd, width.saturating_sub(18))).dim()]),
    ]
}

fn prompt(frame: &mut Frame, app: &App, area: Rect) {
    let shell = Color::Rgb(255, 105, 180);
    let border = if app.prompt.shell { shell } else { Color::DarkGray };
    let (mark, color) = if app.prompt.shell { ("! ", shell) } else { ("> ", Color::Magenta) };
    let block = Block::bordered().border_type(BorderType::Rounded).border_style(Style::new().fg(border));
    let inner = block.inner(area);
    let width = usize::from(inner.width).saturating_sub(PROMPT_MARK).max(1);
    let (rows, (row, column)) = layout(app.prompt.input.text(), app.prompt.input.cursor(), width);
    let visible = usize::from(inner.height.max(1));
    let first = (row + 1).saturating_sub(visible);
    let shown: Vec<Line> = rows
        .into_iter()
        .enumerate()
        .skip(first)
        .map(|(i, text)| Line::from(vec![if i == 0 { mark } else { "  " }.fg(color), text.into()]))
        .collect();
    if let Some(search) = &app.overlays.search {
        let found = search.shown(app.prompt.input.history()).unwrap_or("no match").replace('\n', " ");
        let line = Line::from(vec![
            format!("search '{}': ", search.query).fg(Color::Magenta),
            found.into(),
            "  ctrl+r older · enter takes · esc leaves".dim(),
        ]);
        frame.render_widget(Paragraph::new(line).block(block), area);
    } else if app.prompt.input.is_empty() && app.ask.is_some() {
        let waiting = Line::from(vec![mark.dark_gray(), "Answer the question above first".dark_gray()]);
        frame.render_widget(Paragraph::new(waiting).block(block), area);
    } else if app.prompt.input.is_empty() && app.ask.is_none() && app.overlays.secret.is_none() {
        let hint = match &app.prompt.ghost {
            _ if app.prompt.shell => {
                "Run a shell command · its output goes with your next message · esc leaves".to_owned()
            }
            _ if app.busy() && !app.turn.queued.is_empty() => "↑ edits what is queued · esc interrupts".to_owned(),
            _ if app.busy() => "Type to message the agent as it works".to_owned(),
            Some(ghost) => format!("{ghost}  (tab)"),
            None if failed_last(app) => "Say what to do next, or /retry".to_owned(),
            None if app.transcript.entries.is_empty() => "Try \"explain this project\"".to_owned(),
            None => "Reply to go on · / for commands".to_owned(),
        };
        let placeholder = Line::from(vec![mark.fg(color), hint.dim()]);
        frame.render_widget(Paragraph::new(placeholder).block(block), area);
    } else {
        frame.render_widget(Paragraph::new(shown).block(block), area);
    }
    if app.ask.is_none() && app.overlays.picker.is_none() && app.overlays.secret.is_none() {
        let x = inner.x.saturating_add(u16::try_from(column + PROMPT_MARK).unwrap_or(u16::MAX));
        let y = inner.y.saturating_add(u16::try_from(row - first).unwrap_or(0));
        frame.set_cursor_position(Position::new(x.min(inner.right().saturating_sub(1)), y));
    }
}

fn status(app: &App) -> Line<'static> {
    let model = match app.model() {
        Some(model) => model.label.clone(),
        None if app.catalog.loading => "loading models…".to_owned(),
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
    let armed = app.prompt.clear_armed.map(|at| at.elapsed());
    if let Some(hint) = armed.and_then(|armed_for| super::hints::second_esc(app.prompt.input.is_empty(), armed_for)) {
        return Line::from(format!(" {hint}")).fg(Color::Yellow);
    }
    if let Some(line) = &app.ui.status.text {
        return Line::from(format!(" {line}")).dim();
    }
    let mut spans = Vec::new();
    if let Some(title) = &app.session.title {
        spans.extend([format!(" {title}").bold(), " ·".dim()]);
    }
    spans.extend([
        format!(" {model}").into(),
        " · ".dim(),
        Span::styled(mode_label(app.mode()), Style::new().fg(mode_color).add_modifier(Modifier::BOLD)),
        " · ".dim(),
        format!("{} effort", app.effort_label()).dim(),
    ]);
    if !app.busy() && failed_last(app) {
        spans.extend([" · ".dim(), "✗ last run failed".fg(Color::Red)]);
    }
    for marker in super::features::status(app) {
        spans.push(" · ".dim());
        spans.push(marker.fg(Color::Green));
    }
    if app.transcript.context > 0 {
        let window = app.session.window.or_else(|| app.model().and_then(|model| model.window));
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
    Line::from(spans)
}

fn failed_last(app: &App) -> bool {
    app.transcript.entries.iter().rev().take(2).any(|entry| matches!(entry, Entry::Error(_)))
}

const HINT: &str = "shift+tab mode · /help ";

fn status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let line = status(app);
    let room = usize::from(area.width).saturating_sub(line.width());
    frame.render_widget(Paragraph::new(line), area);
    if room >= HINT.len() + 2 && app.ui.status.text.is_none() {
        frame.render_widget(Paragraph::new(Line::from(HINT).dim().right_aligned()), area);
    }
}
