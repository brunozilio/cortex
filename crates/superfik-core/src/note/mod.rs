//! Notes: what the user (or an agent) writes down for a project.
//!
//! A note belongs to its project, private to its owner, like memory. A
//! session does not own notes: it *links* them (`SessionRecord::notes`),
//! so one note can serve several sessions or none. A linked note is on
//! that session's canvas and in every prompt it sends.
//!
//! The body is plain text. Its first non-empty line is the title, and
//! lines written `- [ ] item` / `- [x] item` are a checklist, which keeps
//! notes readable and writable by agents without a rich-text model.

mod tool;

use serde::{Deserialize, Serialize};

pub use crate::memory::Author;
pub use tool::*;

pub const MAX_NOTE_CHARS: usize = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NoteId(pub u32);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Paper {
    Plain,
    #[default]
    Yellow,
    Blue,
    Pink,
    Green,
}

impl Paper {
    pub const ALL: [Paper; 5] = [Paper::Plain, Paper::Yellow, Paper::Blue, Paper::Pink, Paper::Green];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: NoteId,
    pub body: String,
    #[serde(default)]
    pub paper: Paper,
    pub author: Author,
    pub updated: i64,
    #[serde(default)]
    pub revision: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line<'a> {
    Text(&'a str),
    Check { done: bool, text: &'a str },
}

impl Note {
    pub fn title(&self) -> &str {
        self.body.lines().find(|line| !line.trim().is_empty()).map_or("", |line| match parse_line(line) {
            Line::Text(text) | Line::Check { text, .. } => text.trim(),
        })
    }

    pub fn lines(&self) -> Vec<Line<'_>> {
        self.body.lines().map(parse_line).collect()
    }

    pub fn slug(&self) -> String {
        slug(self.title())
    }

    pub fn brief(&self) -> NoteBrief {
        NoteBrief { id: self.id, title: self.title().to_owned(), body: self.body.clone() }
    }
}

pub fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}

pub fn parse_line(line: &str) -> Line<'_> {
    let trimmed = line.trim_start();
    for (mark, done) in [("- [ ] ", false), ("- [x] ", true), ("- [X] ", true)] {
        if let Some(text) = trimmed.strip_prefix(mark) {
            return Line::Check { done, text };
        }
    }
    for (mark, done) in [("- [ ]", false), ("- [x]", true), ("- [X]", true)] {
        if trimmed == mark {
            return Line::Check { done, text: "" };
        }
    }
    Line::Text(line)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteBrief {
    pub id: NoteId,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum NoteEdit {
    Create {
        body: String,
        #[serde(default)]
        paper: Paper,
    },
    Revise {
        id: NoteId,
        body: String,
        #[serde(default)]
        base: Option<u32>,
    },
    Check {
        id: NoteId,
        item: usize,
        done: bool,
    },
    Recolor {
        id: NoteId,
        paper: Paper,
    },
    Delete {
        id: NoteId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub author: Author,
    pub now: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    pub created: Vec<NoteId>,
    pub changed: Vec<NoteId>,
    pub refused: Vec<NoteId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteBook {
    #[serde(default)]
    pub next: u32,
    #[serde(default)]
    pub notes: Vec<Note>,
}

impl NoteBook {
    pub fn get(&self, id: NoteId) -> Option<&Note> {
        self.notes.iter().find(|n| n.id == id)
    }

    pub fn apply(&mut self, edits: &[NoteEdit], by: Stamp) -> Outcome {
        let mut outcome = Outcome::default();
        for edit in edits {
            match edit {
                NoteEdit::Create { body, paper } => {
                    let body = clean_body(body);
                    if body.trim().is_empty() {
                        continue;
                    }
                    let id = NoteId(self.next.max(self.after_highest()));
                    self.next = id.0 + 1;
                    self.notes.push(Note { id, body, paper: *paper, author: by.author, updated: by.now, revision: 0 });
                    outcome.created.push(id);
                }
                NoteEdit::Revise { id, body, base } => {
                    let Some(note) = self.notes.iter_mut().find(|n| n.id == *id) else {
                        outcome.refused.push(*id);
                        continue;
                    };
                    if base.is_some_and(|base| base != note.revision) {
                        outcome.refused.push(*id);
                        continue;
                    }
                    let body = clean_body(body);
                    if body != note.body {
                        note.body = body;
                        touch(note, by, &mut outcome);
                    }
                }
                NoteEdit::Check { id, item, done } => {
                    let Some(note) = self.notes.iter_mut().find(|n| n.id == *id) else {
                        outcome.refused.push(*id);
                        continue;
                    };
                    match check(&note.body, *item, *done) {
                        Some(body) if body != note.body => {
                            note.body = body;
                            touch(note, by, &mut outcome);
                        }
                        Some(_) => {}
                        None => outcome.refused.push(*id),
                    }
                }
                NoteEdit::Recolor { id, paper } => match self.notes.iter_mut().find(|n| n.id == *id) {
                    Some(note) if note.paper != *paper => {
                        note.paper = *paper;
                        touch(note, by, &mut outcome);
                    }
                    Some(_) => {}
                    None => outcome.refused.push(*id),
                },
                NoteEdit::Delete { id } => {
                    let before = self.notes.len();
                    self.notes.retain(|n| n.id != *id);
                    if self.notes.len() == before {
                        outcome.refused.push(*id);
                    } else {
                        outcome.changed.push(*id);
                    }
                }
            }
        }
        outcome
    }

    fn after_highest(&self) -> u32 {
        self.notes.iter().map(|n| n.id.0 + 1).max().unwrap_or(0)
    }
}

fn touch(note: &mut Note, by: Stamp, outcome: &mut Outcome) {
    note.revision += 1;
    note.updated = by.now;
    note.author = by.author;
    if !outcome.changed.contains(&note.id) {
        outcome.changed.push(note.id);
    }
}

fn check(body: &str, item: usize, done: bool) -> Option<String> {
    let mut seen = 0;
    let mut found = false;
    let lines: Vec<String> = body
        .split('\n')
        .map(|line| {
            if let Line::Check { text, .. } = parse_line(line) {
                if seen == item {
                    found = true;
                    seen += 1;
                    let indent = &line[..line.len() - line.trim_start().len()];
                    let mark = if done { "- [x]" } else { "- [ ]" };
                    return if text.is_empty() { format!("{indent}{mark}") } else { format!("{indent}{mark} {text}") };
                }
                seen += 1;
            }
            line.to_owned()
        })
        .collect();
    found.then(|| lines.join("\n"))
}

fn clean_body(body: &str) -> String {
    body.replace("\r\n", "\n").chars().take(MAX_NOTE_CHARS).collect()
}

#[cfg(test)]
mod tests;
