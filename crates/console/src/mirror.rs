use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tokio::sync::broadcast;

const KEPT_PRINTS: usize = 256;
const MOST_TYPED: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenSize {
    pub cols: u16,
    pub rows: u16,
}

impl ScreenSize {
    pub fn json(self) -> String {
        format!(r#"{{"cols":{},"rows":{}}}"#, self.cols, self.rows)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shown {
    Printed(Vec<u8>),
    Resized(ScreenSize),
}

#[derive(Debug, Default)]
struct Screen {
    session: Option<String>,
    viewers: usize,
    repaint: bool,
    size: Option<ScreenSize>,
    typed: VecDeque<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct Mirror {
    screen: Arc<Mutex<Screen>>,
    shown: broadcast::Sender<Shown>,
}

impl Default for Mirror {
    fn default() -> Self {
        Self { screen: Arc::default(), shown: broadcast::channel(KEPT_PRINTS).0 }
    }
}

#[derive(Debug)]
pub struct Viewer {
    mirror: Mirror,
    pub size: Option<ScreenSize>,
    pub shown: broadcast::Receiver<Shown>,
}

impl Drop for Viewer {
    fn drop(&mut self) {
        let mut screen = self.mirror.screen();
        screen.viewers = screen.viewers.saturating_sub(1);
    }
}

impl Mirror {
    fn screen(&self) -> MutexGuard<'_, Screen> {
        self.screen.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn showing(&self, session: Option<&str>) {
        let mut screen = self.screen();
        if screen.session.as_deref() != session {
            screen.session = session.map(str::to_owned);
        }
    }

    pub(crate) fn shows(&self, session: &str) -> bool {
        self.screen().session.as_deref() == Some(session)
    }

    pub fn watched(&self) -> bool {
        self.screen().viewers > 0
    }

    pub fn take_repaint(&self) -> bool {
        std::mem::take(&mut self.screen().repaint)
    }

    pub fn resized(&self, size: ScreenSize) {
        let mut screen = self.screen();
        if screen.size != Some(size) {
            screen.size = Some(size);
            let _ = self.shown.send(Shown::Resized(size));
        }
    }

    pub fn print(&self, bytes: Vec<u8>) {
        if !bytes.is_empty() {
            let _ = self.shown.send(Shown::Printed(bytes));
        }
    }

    pub fn typed(&self) -> Vec<Vec<u8>> {
        self.screen().typed.drain(..).collect()
    }

    pub fn join(&self) -> Viewer {
        let mut screen = self.screen();
        screen.viewers += 1;
        screen.repaint = true;
        Viewer { mirror: self.clone(), size: screen.size, shown: self.shown.subscribe() }
    }

    pub(crate) fn ask_repaint(&self) {
        self.screen().repaint = true;
    }

    pub(crate) fn type_in(&self, bytes: Vec<u8>) {
        let mut screen = self.screen();
        if screen.typed.len() < MOST_TYPED {
            screen.typed.push_back(bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: ScreenSize = ScreenSize { cols: 120, rows: 40 };

    #[test]
    fn a_viewer_is_counted_while_it_watches_and_asks_for_a_whole_screen() {
        let mirror = Mirror::default();
        assert!(!mirror.watched());
        let viewer = mirror.join();
        assert!(mirror.watched());
        assert!(mirror.take_repaint());
        assert!(!mirror.take_repaint(), "asked once");
        drop(viewer);
        assert!(!mirror.watched());
    }

    #[test]
    fn a_viewer_learns_the_size_first_then_what_is_printed() {
        let mirror = Mirror::default();
        mirror.resized(SIZE);
        let mut viewer = mirror.join();
        assert_eq!(viewer.size, Some(SIZE));
        mirror.resized(SIZE);
        mirror.print(b"hello".to_vec());
        mirror.print(Vec::new());
        assert_eq!(viewer.shown.try_recv(), Ok(Shown::Printed(b"hello".to_vec())), "an unchanged size is not sent");
        assert!(viewer.shown.try_recv().is_err(), "nothing empty is sent");
    }

    #[test]
    fn what_the_page_types_waits_for_the_screen_and_is_bounded() {
        let mirror = Mirror::default();
        for _ in 0..MOST_TYPED + 10 {
            mirror.type_in(b"a".to_vec());
        }
        assert_eq!(mirror.typed().len(), MOST_TYPED);
        assert!(mirror.typed().is_empty());
    }

    #[test]
    fn the_mirror_shows_only_the_session_it_was_told() {
        let mirror = Mirror::default();
        assert!(!mirror.shows("abc"));
        mirror.showing(Some("abc"));
        assert!(mirror.shows("abc"));
        assert!(!mirror.shows("other"));
        mirror.showing(None);
        assert!(!mirror.shows("abc"));
    }

    #[test]
    fn the_size_is_sent_as_the_json_the_page_reads() {
        assert_eq!(SIZE.json(), r#"{"cols":120,"rows":40}"#);
    }
}
