use std::cell::Cell;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use super::diff_model::{self, FileDiff, Status};
use super::diffs;
use super::git::{Changes, Scope};
use super::wrap::wrap;

pub struct DiffView {
    scope: Scope,
    files: Vec<FileDiff>,
    untracked: Vec<String>,
    error: Option<String>,
    loading: bool,
    selected: usize,
    scroll: usize,
    pane: Cell<(usize, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    PreviousFile,
    NextFile,
    LineUp,
    LineDown,
    PageUp,
    PageDown,
    NextHunk,
    PreviousHunk,
    Scope,
    Refresh,
    Close,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Stay,
    Close,
    Load(Scope),
}

impl DiffView {
    pub fn new(scope: Scope) -> Self {
        Self {
            scope,
            files: Vec::new(),
            untracked: Vec::new(),
            error: None,
            loading: true,
            selected: 0,
            scroll: 0,
            pane: Cell::new((80, 20)),
        }
    }

    pub fn loading(&self) -> bool {
        self.loading
    }

    pub fn loaded(&mut self, scope: Scope, changes: Result<Changes, String>) {
        if scope != self.scope {
            return;
        }
        let kept = self.names().nth(self.selected).map(str::to_owned);
        (self.files, self.untracked, self.error) = match changes {
            Ok(changes) => (diff_model::parse(&changes.diff), changes.untracked, None),
            Err(error) => (Vec::new(), Vec::new(), Some(error)),
        };
        self.loading = false;
        match kept.and_then(|name| self.names().position(|n| n == name)) {
            Some(index) => self.selected = index,
            None => (self.selected, self.scroll) = (0, 0),
        }
    }

    pub fn key(&mut self, key: Key) -> Action {
        let (width, height) = self.pane.get();
        let page = height.saturating_sub(2).max(1);
        let count = self.names().count();
        match key {
            Key::Close => return Action::Close,
            Key::Scope | Key::Refresh => {
                if key == Key::Scope {
                    self.scope = self.scope.next();
                }
                self.loading = true;
                return Action::Load(self.scope);
            }
            Key::PreviousFile if count > 0 => self.choose((self.selected + count - 1) % count),
            Key::NextFile if count > 0 => self.choose((self.selected + 1) % count),
            Key::LineUp => self.scroll = self.scroll.saturating_sub(1),
            Key::LineDown => self.scroll += 1,
            Key::PageUp => self.scroll = self.scroll.saturating_sub(page),
            Key::PageDown => self.scroll += page,
            Key::NextHunk => {
                let starts = self.hunks(width).1;
                self.scroll = starts.into_iter().find(|&start| start > self.scroll).unwrap_or(self.scroll);
            }
            Key::PreviousHunk => {
                let starts = self.hunks(width).1;
                self.scroll = starts.into_iter().rev().find(|&start| start < self.scroll).unwrap_or(0);
            }
            Key::PreviousFile | Key::NextFile => {}
        }
        self.scroll = self.scroll.min(self.hunks(width).0.len().saturating_sub(1));
        Action::Stay
    }

    fn choose(&mut self, index: usize) {
        self.selected = index;
        self.scroll = 0;
    }

    fn names(&self) -> impl Iterator<Item = &str> {
        self.files.iter().map(|f| f.path.as_str()).chain(self.untracked.iter().map(String::as_str))
    }

    fn hunks(&self, width: usize) -> (Vec<Line<'static>>, Vec<usize>) {
        let (mut lines, mut starts) = (Vec::new(), Vec::new());
        if self.loading {
            lines.push(Line::from("Loading…").dim());
        } else if let Some(error) = &self.error {
            lines.extend(wrap(&format!("git: {error}"), width).into_iter().map(|l| Line::from(l).fg(Color::Red)));
        } else if let Some(file) = self.files.get(self.selected) {
            lines.push(diffs::heading(file));
            if file.status == Status::Binary {
                lines.push(Line::from("    binary file changed").dim());
            }
            for hunk in &file.hunks {
                starts.push(lines.len());
                lines.push(Line::from(format!("  @@ {}", hunk.context)).fg(Color::Cyan).dim());
                for line in &hunk.lines {
                    diffs::wrapped(line, width, &mut lines);
                }
            }
        } else if let Some(name) = self.selected.checked_sub(self.files.len()).and_then(|i| self.untracked.get(i)) {
            lines.push(Line::from(vec![Span::styled("● ", Style::new().fg(Color::Cyan)), name.clone().into()]));
            lines.push(Line::from("    New file, not yet in git.").dim());
        } else {
            lines.push(Line::from(format!("No {} changes.", self.scope.label())).dim());
        }
        (lines, starts)
    }
}

pub fn draw(frame: &mut Frame, view: &DiffView) {
    let [title, main, hint] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());
    frame.render_widget(Paragraph::new(heading(view)), title);
    let list_width = if main.width >= 80 { (main.width / 3).clamp(24, 48) } else { 0 };
    let [list, pane] = Layout::horizontal([Constraint::Length(list_width), Constraint::Min(1)]).areas(main);
    if list_width > 0 {
        files(frame, view, list);
    }
    let pane = pane.inner(ratatui::layout::Margin { horizontal: 1, vertical: 0 });
    let (width, height) = (usize::from(pane.width).max(20), usize::from(pane.height));
    view.pane.set((width, height));
    let lines = view.hunks(width).0;
    let shown: Vec<Line> = lines.into_iter().skip(view.scroll).take(height).collect();
    frame.render_widget(Paragraph::new(shown), pane);
    let keys = "↑↓ file · j/k line · pgup/pgdn page · n/p hunk · tab scope · r refresh · q closes";
    frame.render_widget(Paragraph::new(Line::from(format!(" {keys}")).dim()), hint);
}

fn heading(view: &DiffView) -> Line<'static> {
    let count = view.files.len() + view.untracked.len();
    let added: usize = view.files.iter().map(|f| f.added).sum();
    let removed: usize = view.files.iter().map(|f| f.removed).sum();
    let mut spans = vec![
        format!(" Changes · {} · {count} files ", view.scope.label()).bold(),
        format!(" +{added}").fg(Color::Green),
        format!(" -{removed}").fg(Color::Red),
    ];
    if count > 0 {
        spans.push(format!("  ({} of {count})", view.selected + 1).dim());
    }
    if view.loading() {
        spans.push("  loading…".dim());
    }
    Line::from(spans)
}

fn files(frame: &mut Frame, view: &DiffView, area: Rect) {
    let width = usize::from(area.width.saturating_sub(4));
    let height = usize::from(area.height).max(1);
    let letters = view.files.iter().map(|f| (f.status.letter(), f.path.as_str()));
    let rows: Vec<Line> = letters
        .chain(view.untracked.iter().map(|name| ('?', name.as_str())))
        .enumerate()
        .skip((view.selected + 1).saturating_sub(height))
        .take(height)
        .map(|(i, (letter, name))| {
            let name = wrap(name, width.saturating_sub(2)).into_iter().next().unwrap_or_default();
            let marker = if i == view.selected { "❯" } else { " " };
            let style = if i == view.selected { Style::new().fg(Color::Cyan) } else { Style::new() };
            Line::styled(format!("{marker} {letter} {name}"), style)
        })
        .collect();
    let block = Block::new().borders(Borders::RIGHT).border_style(Style::new().fg(Color::DarkGray));
    frame.render_widget(Paragraph::new(rows).block(block), area);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn changes(diff: &str) -> Result<Changes, String> {
        Ok(Changes { diff: diff.into(), untracked: vec!["new.txt".into()] })
    }

    const TWO_HUNKS: &str = "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@ fn a\n-x\n+y\n\
                             @@ -20 +20 @@ fn b\n-p\n+q\ndiff --git a/b.rs b/b.rs\n--- a/b.rs\n+++ b/b.rs\n\
                             @@ -1 +1 @@\n-1\n+2\n";

    #[test]
    fn n_and_p_jump_between_hunks_and_moving_file_starts_at_the_top() {
        let mut view = DiffView::new(Scope::Uncommitted);
        view.loaded(Scope::Uncommitted, changes(TWO_HUNKS));
        assert_eq!(view.key(Key::NextHunk), Action::Stay);
        assert_eq!(view.scroll, 1, "the first hunk, under the file's name");
        view.key(Key::NextHunk);
        assert_eq!(view.scroll, 4, "after the name, @@, -x and +y comes the second @@");
        view.key(Key::NextHunk);
        assert_eq!(view.scroll, 4, "no hunk after the last");
        view.key(Key::PreviousHunk);
        assert_eq!(view.scroll, 1);
        view.key(Key::NextFile);
        assert_eq!((view.selected, view.scroll), (1, 0));
        view.key(Key::NextFile);
        view.key(Key::NextFile);
        assert_eq!(view.selected, 0, "past the untracked file, back to the first");
        view.key(Key::PreviousFile);
        assert_eq!(view.selected, 2);
    }

    #[test]
    fn a_new_scope_reloads_and_a_late_answer_for_the_old_one_is_dropped() {
        let mut view = DiffView::new(Scope::Uncommitted);
        view.loaded(Scope::Uncommitted, changes(TWO_HUNKS));
        view.key(Key::NextFile);
        assert_eq!(view.key(Key::Scope), Action::Load(Scope::Unstaged));
        view.loaded(Scope::Uncommitted, changes(""));
        assert!(view.loading() && view.files.len() == 2, "still waiting for the unstaged changes");
        view.loaded(Scope::Unstaged, changes(TWO_HUNKS));
        assert_eq!(view.selected, 1, "b.rs is still the chosen file");
        assert_eq!(view.key(Key::Close), Action::Close);
    }
}
