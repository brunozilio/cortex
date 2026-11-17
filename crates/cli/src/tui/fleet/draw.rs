use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::super::app::App;
use super::{State, rows};

pub fn draw(frame: &mut Frame, app: &App) {
    let [title, main, hint] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());
    let rows = rows(app);
    frame.render_widget(Paragraph::new(Line::from(format!(" Sessions · {}", rows.len())).bold()), title);
    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let picked = i == app.fleet.cursor;
            let style = if picked { Style::new().fg(Color::Cyan).bold() } else { Style::new() };
            let marker = if picked { " ❯ " } else { "   " };
            let mut spans = vec![
                Span::styled(format!("{marker}{}", row.title), style),
                format!("  {}", row.detail).fg(tint(row.state)),
            ];
            if row.current {
                spans.push("  · here".dim());
            }
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), main);
    frame.render_widget(Paragraph::new(Line::from(" ↑↓ pick · enter opens · n new session · esc back").dim()), hint);
}

fn tint(state: State) -> Color {
    match state {
        State::Working => Color::Yellow,
        State::NeedsYou => Color::Magenta,
        State::Done => Color::Green,
        State::Idle | State::Earlier => Color::DarkGray,
    }
}
