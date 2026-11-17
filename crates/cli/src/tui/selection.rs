use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::style::Modifier;
use unicode_width::UnicodeWidthStr;

use super::app::App;

const WHEEL_ROWS: u16 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Cell {
    row: u16,
    column: u16,
}

#[derive(Debug, Default)]
pub struct Selection {
    dragged: Option<(Cell, Cell)>,
    released: bool,
}

impl Selection {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    fn span(&self) -> Option<(Cell, Cell)> {
        let (anchor, head) = self.dragged?;
        (anchor != head).then(|| (anchor.min(head), anchor.max(head)))
    }
}

pub fn mouse(app: &mut App, event: MouseEvent) {
    let cell = Cell { row: event.row, column: event.column };
    match event.kind {
        MouseEventKind::ScrollUp => {
            app.ui.scroll = app.ui.scroll.saturating_add(WHEEL_ROWS);
            app.ui.selection.clear();
        }
        MouseEventKind::ScrollDown => {
            app.ui.scroll = app.ui.scroll.saturating_sub(WHEEL_ROWS);
            app.ui.selection.clear();
        }
        MouseEventKind::Down(MouseButton::Left) => {
            app.ui.selection = Selection { dragged: Some((cell, cell)), released: false };
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if let Some((_, head)) = &mut app.ui.selection.dragged {
                *head = cell;
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if app.ui.selection.span().is_some() {
                app.ui.selection.released = true;
            } else {
                app.ui.selection.clear();
            }
        }
        _ => {}
    }
}

pub fn show(buffer: &mut Buffer, selection: &Selection) -> Option<String> {
    let span = selection.span()?;
    let text = selection.released.then(|| text(buffer, span));
    for (row, columns) in rows(buffer, span) {
        for column in columns {
            buffer[(column, row)].modifier.insert(Modifier::REVERSED);
        }
    }
    text
}

pub fn copied(app: &mut App, text: Option<String>) {
    let Some(text) = text else { return };
    app.ui.selection.released = false;
    super::export::copy_selection(app, text);
}

fn rows(buffer: &Buffer, (start, end): (Cell, Cell)) -> Vec<(u16, std::ops::Range<u16>)> {
    let area = buffer.area;
    let last_row = end.row.min(area.bottom().saturating_sub(1));
    (start.row..=last_row)
        .map(|row| {
            let from = if row == start.row { start.column } else { area.left() };
            let to = if row == end.row { end.column.saturating_add(1) } else { area.right() };
            (row, from.min(area.right())..to.min(area.right()))
        })
        .collect()
}

fn text(buffer: &Buffer, span: (Cell, Cell)) -> String {
    rows(buffer, span)
        .into_iter()
        .map(|(row, columns)| {
            let mut line = String::new();
            let mut column = columns.start;
            while column < columns.end {
                let symbol = buffer[(column, row)].symbol();
                line.push_str(symbol);
                column = column.saturating_add(u16::try_from(symbol.width()).unwrap_or(1).max(1));
            }
            line.trim_end().to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use agent_engine::fake::FakeEngine;
    use app::Model;
    use ratatui::crossterm::event::KeyModifiers;
    use ratatui::layout::Rect;

    use super::*;
    use crate::tui::tests::{client, type_text, until};

    fn at(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent { kind, column, row, modifiers: KeyModifiers::NONE }
    }

    fn app(data: &std::path::Path) -> App {
        let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], Vec::new());
        let mut app = App::new(client(&engine, data), data.into(), None, None);
        until(&mut app, |app| app.session.is_some());
        app
    }

    fn page() -> Buffer {
        Buffer::with_lines(["  first line   ", "  second 日本 x ", "  third line   "])
    }

    fn drag(app: &mut App, from: (u16, u16), to: (u16, u16)) {
        mouse(app, at(MouseEventKind::Down(MouseButton::Left), from.0, from.1));
        mouse(app, at(MouseEventKind::Drag(MouseButton::Left), to.0, to.1));
    }

    #[test]
    fn the_wheel_scrolls_the_conversation_and_leaves_the_prompt_alone() {
        let data = tempfile::tempdir().expect("data");
        let mut app = app(data.path());
        type_text(&mut app, "half written");
        mouse(&mut app, at(MouseEventKind::ScrollUp, 4, 4));
        mouse(&mut app, at(MouseEventKind::ScrollUp, 4, 4));
        assert_eq!(app.ui.scroll, 6);
        mouse(&mut app, at(MouseEventKind::ScrollDown, 4, 4));
        assert_eq!(app.ui.scroll, 3);
        assert_eq!(app.prompt.input.text(), "half written");
    }

    #[test]
    fn dragging_highlights_the_text_and_releasing_copies_it_once() {
        let data = tempfile::tempdir().expect("data");
        let mut app = app(data.path());
        drag(&mut app, (8, 0), (9, 1));
        let mut buffer = page();
        assert_eq!(show(&mut buffer, &app.ui.selection), None, "nothing is copied while the button is held");
        assert!(buffer[(8, 0)].modifier.contains(Modifier::REVERSED));
        assert!(buffer[(0, 1)].modifier.contains(Modifier::REVERSED));
        assert!(!buffer[(7, 0)].modifier.contains(Modifier::REVERSED));
        assert!(!buffer[(11, 1)].modifier.contains(Modifier::REVERSED));

        mouse(&mut app, at(MouseEventKind::Up(MouseButton::Left), 9, 1));
        assert_eq!(show(&mut page(), &app.ui.selection).as_deref(), Some("line\n  second 日"));
        app.ui.selection.released = false;
        assert_eq!(show(&mut page(), &app.ui.selection), None, "the highlight stays, the copy happens once");
    }

    #[test]
    fn a_selection_dragged_upwards_reads_in_reading_order() {
        let data = tempfile::tempdir().expect("data");
        let mut app = app(data.path());
        drag(&mut app, (6, 2), (2, 0));
        mouse(&mut app, at(MouseEventKind::Up(MouseButton::Left), 2, 0));
        assert_eq!(show(&mut page(), &app.ui.selection).as_deref(), Some("first line\n  second 日本 x\n  third"));
    }

    #[test]
    fn a_click_without_a_drag_and_a_turn_of_the_wheel_drop_the_selection() {
        let data = tempfile::tempdir().expect("data");
        let mut app = app(data.path());
        drag(&mut app, (2, 0), (6, 0));
        mouse(&mut app, at(MouseEventKind::Down(MouseButton::Left), 3, 1));
        mouse(&mut app, at(MouseEventKind::Up(MouseButton::Left), 3, 1));
        assert_eq!(show(&mut page(), &app.ui.selection), None);
        assert!(!page_after(&app)[(3, 1)].modifier.contains(Modifier::REVERSED));

        drag(&mut app, (2, 0), (6, 0));
        mouse(&mut app, at(MouseEventKind::ScrollUp, 0, 0));
        assert!(!page_after(&app)[(3, 0)].modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn a_selection_past_the_screen_after_a_resize_is_cut_to_it() {
        let data = tempfile::tempdir().expect("data");
        let mut app = app(data.path());
        drag(&mut app, (2, 0), (40, 9));
        mouse(&mut app, at(MouseEventKind::Up(MouseButton::Left), 40, 9));
        let mut small = Buffer::empty(Rect::new(0, 0, 4, 2));
        assert_eq!(show(&mut small, &app.ui.selection).as_deref(), Some("\n"));
    }

    fn page_after(app: &App) -> Buffer {
        let mut buffer = page();
        show(&mut buffer, &app.ui.selection);
        buffer
    }
}
