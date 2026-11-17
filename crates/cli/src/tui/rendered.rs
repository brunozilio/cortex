use std::rc::Rc;

use ratatui::text::Line;

use super::entries;
use super::transcript::Entry;

pub type Lines = Rc<Vec<Line<'static>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shape {
    pub width: usize,
    pub expanded: bool,
    pub light: bool,
}

#[derive(Debug)]
struct Kept {
    entry: Entry,
    shape: Shape,
    lines: Lines,
}

#[derive(Debug, Default)]
pub struct Rendered {
    kept: Vec<Option<Kept>>,
    #[cfg(test)]
    renders: usize,
}

impl Rendered {
    pub fn lines(&mut self, at: usize, entry: &Entry, shape: Shape) -> Lines {
        if let Some(Some(kept)) = self.kept.get(at)
            && kept.shape == shape
            && kept.entry == *entry
        {
            return Rc::clone(&kept.lines);
        }
        let mut lines = Vec::new();
        entries::lines(entry, shape.width, shape.expanded, &mut lines);
        #[cfg(test)]
        {
            self.renders += 1;
        }
        let lines = Rc::new(lines);
        if self.kept.len() <= at {
            self.kept.resize_with(at + 1, || None);
        }
        self.kept[at] = Some(Kept { entry: entry.clone(), shape, lines: Rc::clone(&lines) });
        lines
    }

    pub fn forget_after(&mut self, entries: usize) {
        self.kept.truncate(entries);
    }

    #[cfg(test)]
    pub fn renders(&self) -> usize {
        self.renders
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHAPE: Shape = Shape { width: 40, expanded: false, light: false };

    #[test]
    fn an_entry_is_rendered_once_until_it_or_the_shape_changes() {
        let mut rendered = Rendered::default();
        let said = Entry::Assistant("one".into());
        let first = rendered.lines(0, &said, SHAPE);
        assert!(Rc::ptr_eq(&first, &rendered.lines(0, &said, SHAPE)));
        assert_eq!(rendered.renders(), 1);

        let grown = Entry::Assistant("one two".into());
        assert!(!Rc::ptr_eq(&first, &rendered.lines(0, &grown, SHAPE)), "the text changed");
        rendered.lines(0, &grown, Shape { width: 20, ..SHAPE });
        rendered.lines(0, &grown, Shape { width: 20, expanded: true, ..SHAPE });
        rendered.lines(0, &grown, Shape { width: 20, expanded: true, light: true });
        assert_eq!(rendered.renders(), 5, "the width, the expansion and the theme each render again");
    }

    #[test]
    fn drawing_a_long_transcript_again_renders_only_the_entry_that_changed() {
        use crate::tui::tests::{client, screen, until};
        let data = tempfile::tempdir().expect("data");
        let engine = agent_engine::fake::FakeEngine::new(vec![app::Model::new("keyed:scripted", "Scripted")], vec![]);
        let mut app = crate::tui::app::App::new(client(&engine, data.path()), data.path().into(), None, None);
        until(&mut app, |app| app.session.is_some());
        for number in 0..300 {
            app.transcript.push(Entry::Assistant(format!("answer {number}")));
        }
        let entries = app.transcript.entries.len();
        let renders = |app: &crate::tui::app::App| app.ui.rendered.borrow().renders();

        screen(&app);
        assert_eq!(renders(&app), entries, "the first draw renders each entry once");
        screen(&app);
        assert_eq!(renders(&app), entries, "nothing changed: nothing is rendered again");

        if let Some(Entry::Assistant(text)) = app.transcript.entries.last_mut() {
            text.push_str(" and what streamed in since");
        }
        let shown = screen(&app);
        assert!(shown.contains("answer 299 and what streamed in since"), "{shown}");
        assert_eq!(renders(&app), entries + 1, "only the entry that grew");

        app.ui.scroll = 120;
        let above = screen(&app);
        assert!(above.contains("answer 2") && !above.contains("answer 299"), "{above}");
        assert_eq!(renders(&app), entries + 1, "scrolling renders nothing");

        app.transcript.entries.truncate(10);
        app.transcript.push(Entry::Notice("after a rewind".into()));
        screen(&app);
        assert_eq!(renders(&app), entries + 2, "the kept prefix is not rendered again");
    }

    #[test]
    fn the_same_text_at_another_place_is_another_entry() {
        let mut rendered = Rendered::default();
        let said = Entry::Notice("same".into());
        rendered.lines(0, &said, SHAPE);
        rendered.lines(3, &said, SHAPE);
        assert_eq!(rendered.renders(), 2);
        rendered.forget_after(1);
        rendered.lines(0, &said, SHAPE);
        rendered.lines(3, &said, SHAPE);
        assert_eq!(rendered.renders(), 3, "only what was forgotten is rendered again");
    }
}
