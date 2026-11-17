use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget, Wrap};
use ratatui::{Terminal, TerminalOptions, Viewport};

use super::Tty;
use super::app::App;
use super::entries;
use super::mirror::Tee;

pub const HEIGHT: u16 = 16;

pub fn terminal(inline: bool) -> Tty {
    restore_on_panic();
    let _ = enable_raw_mode();
    let viewport = if inline {
        Viewport::Inline(HEIGHT)
    } else {
        let _ = execute!(std::io::stdout(), EnterAlternateScreen);
        Viewport::Fullscreen
    };
    let backend = Tee::new(CrosstermBackend::new(std::io::stdout()));
    match Terminal::with_options(backend, TerminalOptions { viewport }) {
        Ok(terminal) => terminal,
        Err(error) => panic!("the terminal could not be set up: {error}"),
    }
}

fn restore_on_panic() {
    let earlier = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        earlier(info);
    }));
}

pub fn print<B: Backend>(terminal: &mut Terminal<B>, lines: Vec<Line<'static>>) -> Result<(), B::Error> {
    if lines.is_empty() {
        return Ok(());
    }
    let height = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    terminal
        .insert_before(height, |buffer| Paragraph::new(lines).wrap(Wrap { trim: false }).render(buffer.area, buffer))
}

pub fn flush<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<(), B::Error> {
    let Some(printed) = app.ui.inline else { return Ok(()) };
    let entries = &app.transcript.entries;
    if entries.len() < printed {
        app.ui.inline = Some(entries.len());
        return Ok(());
    }
    if app.busy() || entries.len() == printed {
        return Ok(());
    }
    let width = usize::from(terminal.size()?.width.saturating_sub(2)).max(10);
    let mut lines = Vec::new();
    for entry in &entries[printed..] {
        entries::lines(entry, width, app.ui.expanded, &mut lines);
    }
    app.ui.inline = Some(entries.len());
    print(terminal, lines)
}

#[cfg(test)]
mod tests {
    use agent_engine::fake::FakeEngine;
    use app::{EngineEvent, Model, SessionId};
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::tui::tests::{client, type_text, until};

    fn text(buffer: &ratatui::buffer::Buffer) -> String {
        let area = buffer.area;
        (0..area.height)
            .map(|y| (0..area.width).map(|x| buffer[(x, y)].symbol().to_owned()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn a_finished_exchange_goes_to_the_scrollback_and_the_chat_stays_at_the_bottom() {
        let data = tempfile::tempdir().expect("data");
        let script = vec![
            EngineEvent::SessionStarted(SessionId("s".into())),
            EngineEvent::TextDelta("Answered.".into()),
            EngineEvent::Completed { duration_ms: 1, cost_usd: None },
        ];
        let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script);
        let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
        app.ui.inline = Some(0);
        until(&mut app, |app| app.session.is_some());
        let options = TerminalOptions { viewport: Viewport::Inline(8) };
        let mut terminal = Terminal::with_options(TestBackend::new(60, 20), options).expect("terminal");

        type_text(&mut app, "a question");
        crate::tui::tests::press(&mut app, ratatui::crossterm::event::KeyCode::Enter);
        until(&mut app, |app| !app.busy() && app.transcript.entries.len() >= 2);
        flush(&mut terminal, &mut app).expect("flush");
        terminal.draw(|frame| crate::tui::view::draw(frame, &app)).expect("draw");

        let printed = format!("{}\n{}", text(terminal.backend().scrollback()), text(terminal.backend().buffer()));
        let asked = printed.find("a question").expect("the question above");
        let answered = printed.find("Answered.").expect("the answer above");
        assert!(asked < answered, "{printed}");
        assert_eq!(app.ui.inline, Some(app.transcript.entries.len()));
        flush(&mut terminal, &mut app).expect("again");
        let again = format!("{}\n{}", text(terminal.backend().scrollback()), text(terminal.backend().buffer()));
        assert_eq!(again.matches("Answered.").count(), 1, "printed once: {again}");
    }
}
