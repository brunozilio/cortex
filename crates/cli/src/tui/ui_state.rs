use std::cell::RefCell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::motion::Motion;
use super::rendered::Rendered;
use super::selection::Selection;
use super::statusline::Status;

const QUIT_WINDOW: Duration = Duration::from_secs(2);
const PAGE: u16 = 10;

pub struct Ui {
    pub scroll: u16,
    pub expanded: bool,
    pub show_tasks: bool,
    pub bell: bool,
    pub redraw: bool,
    pub focused: bool,
    pub motion: RefCell<Motion>,
    pub rendered: RefCell<Rendered>,
    pub selection: Selection,
    pub inline: Option<usize>,
    pub status: Status,
    pub external_edit: bool,
    pub edit_file: Option<PathBuf>,
    pub quit: bool,
    pub exit_chore: Option<Box<dyn FnOnce() -> String>>,
    quit_armed: Option<Instant>,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            scroll: 0,
            expanded: false,
            show_tasks: true,
            bell: false,
            redraw: false,
            focused: true,
            motion: RefCell::new(Motion::from_env()),
            rendered: RefCell::default(),
            selection: Selection::default(),
            inline: None,
            status: Status::default(),
            external_edit: false,
            edit_file: None,
            quit: false,
            exit_chore: None,
            quit_armed: None,
        }
    }
}

impl Ui {
    pub fn page_up(&mut self) {
        self.scroll = self.scroll.saturating_add(PAGE);
    }

    pub fn page_down(&mut self) {
        self.scroll = self.scroll.saturating_sub(PAGE);
    }

    pub fn quitting(&self) -> bool {
        self.quit_armed.is_some_and(|at| at.elapsed() < QUIT_WINDOW)
    }

    pub fn quit_or_arm(&mut self) {
        if self.quitting() {
            self.quit = true;
        } else {
            self.quit_armed = Some(Instant::now());
        }
    }
}
