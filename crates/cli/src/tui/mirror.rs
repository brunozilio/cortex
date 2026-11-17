use std::io::Write;

use console::{Mirror, ScreenSize};
use ratatui::backend::{Backend, ClearType, CrosstermBackend, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Size};
use ratatui::{Frame, Terminal};

const PASTES_WRAPPED: &[u8] = b"\x1b[?2004h";

#[derive(Debug)]
struct Outlet {
    mirror: Mirror,
    pending: Vec<u8>,
}

impl Write for Outlet {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.pending.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.mirror.print(std::mem::take(&mut self.pending));
        Ok(())
    }
}

#[derive(Debug)]
struct Copy {
    backend: CrosstermBackend<Outlet>,
    size: Size,
}

#[derive(Debug)]
pub struct Tee<B> {
    real: B,
    copy: Option<Copy>,
    cursor_shown: bool,
    cursor_at: Position,
}

impl<B: Backend> Tee<B> {
    pub fn new(real: B) -> Self {
        Self { real, copy: None, cursor_shown: false, cursor_at: Position::ORIGIN }
    }

    #[cfg(test)]
    pub fn real(&self) -> &B {
        &self.real
    }

    fn copy(&mut self, step: impl FnOnce(&mut CrosstermBackend<Outlet>) -> std::io::Result<()>) {
        if let Some(copy) = &mut self.copy {
            let _ = step(&mut copy.backend);
        }
    }

    fn start_copying(&mut self, mirror: Mirror, size: Size, whole: &Buffer) {
        let outlet = Outlet { mirror, pending: PASTES_WRAPPED.to_vec() };
        self.copy = Some(Copy { backend: CrosstermBackend::new(outlet), size });
        let (shown, at) = (self.cursor_shown, self.cursor_at);
        let blank = Buffer::empty(whole.area);
        self.copy(|backend| {
            backend.clear()?;
            backend.draw(blank.diff(whole).into_iter())?;
            if shown {
                backend.set_cursor_position(at)?;
                backend.show_cursor()?;
            } else {
                backend.hide_cursor()?;
            }
            Backend::flush(backend)
        });
    }
}

impl<B: Backend> Backend for Tee<B> {
    type Error = B::Error;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        if self.copy.is_none() {
            return self.real.draw(content);
        }
        let cells: Vec<(u16, u16, &Cell)> = content.collect();
        self.real.draw(cells.iter().copied())?;
        self.copy(|backend| backend.draw(cells.into_iter()));
        Ok(())
    }

    fn append_lines(&mut self, lines: u16) -> Result<(), Self::Error> {
        self.real.append_lines(lines)?;
        self.copy(|backend| backend.append_lines(lines));
        Ok(())
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        self.real.hide_cursor()?;
        self.cursor_shown = false;
        self.copy(Backend::hide_cursor);
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        self.real.show_cursor()?;
        self.cursor_shown = true;
        self.copy(Backend::show_cursor);
        Ok(())
    }

    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        self.real.get_cursor_position()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error> {
        let position = position.into();
        self.real.set_cursor_position(position)?;
        self.cursor_at = position;
        self.copy(|backend| backend.set_cursor_position(position));
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.real.clear()?;
        self.copy(Backend::clear);
        Ok(())
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        self.real.clear_region(clear_type)?;
        self.copy(|backend| backend.clear_region(clear_type));
        Ok(())
    }

    fn size(&self) -> Result<Size, Self::Error> {
        self.real.size()
    }

    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        self.real.window_size()
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.real.flush()?;
        self.copy(Backend::flush);
        Ok(())
    }
}

pub fn draw<B: Backend>(
    terminal: &mut Terminal<Tee<B>>,
    mirror: Option<&Mirror>,
    render: impl FnOnce(&mut Frame),
) -> Result<(), B::Error> {
    let Some(mirror) = mirror.filter(|mirror| mirror.watched()) else {
        terminal.backend_mut().copy = None;
        terminal.draw(render)?;
        return Ok(());
    };
    let size = terminal.backend().size()?;
    let asked = mirror.take_repaint();
    let whole = asked || terminal.backend().copy.as_ref().map(|copy| copy.size) != Some(size);
    if whole {
        terminal.backend_mut().copy = None;
    }
    let completed = terminal.draw(render)?;
    if whole {
        let buffer = completed.buffer.clone();
        mirror.resized(ScreenSize { cols: size.width, rows: size.height });
        terminal.backend_mut().start_copying(mirror.clone(), size, &buffer);
    }
    Ok(())
}

#[cfg(test)]
#[path = "mirror_tests.rs"]
mod tests;
