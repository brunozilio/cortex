use super::{Entry, SessionStatus, ToolState, Transcript};
use crate::EngineEvent;
use crate::trace::Record;

const ATTACHED: &str = "\n\nThe user attached these files; read them as needed:\n";

fn written(told: &str) -> (String, Vec<String>) {
    let Some((prompt, listed)) = told.rsplit_once(ATTACHED) else { return (told.to_owned(), Vec::new()) };
    let name = |line: &str| {
        let path = std::path::Path::new(line.strip_prefix("- ")?);
        Some(path.file_name()?.to_string_lossy().into_owned())
    };
    match listed.lines().map(name).collect::<Option<Vec<String>>>() {
        Some(names) if !names.is_empty() => (prompt.to_owned(), names),
        _ => (told.to_owned(), Vec::new()),
    }
}

impl Transcript {
    pub fn replay(&mut self, records: &[Record]) {
        self.follow(records);
        self.left_unfinished();
    }

    pub fn follow(&mut self, records: &[Record]) -> usize {
        let (mut shown, mut used) = (None, 0);
        for (at, record) in records.iter().enumerate() {
            match record {
                Record::Shown { text, attached } => {
                    shown = Some((text.clone(), attached.clone()));
                    continue;
                }
                Record::Prompted { prompt, .. } => {
                    self.left_unfinished();
                    let (prompt, names) = shown.take().unwrap_or_else(|| written(prompt));
                    if !names.is_empty() {
                        self.push(Entry::Attached { names });
                    }
                    self.push(Entry::User(prompt));
                    self.begin();
                }
                Record::Event { event, .. } => self.apply(event),
                _ => {}
            }
            used = at + 1;
        }
        used
    }

    fn left_unfinished(&mut self) {
        if matches!(self.status(), SessionStatus::Running | SessionStatus::Thinking | SessionStatus::NeedsYou) {
            self.observe(&EngineEvent::Cancelled);
            self.follow_agents(&EngineEvent::Cancelled);
            self.settle(ToolState::Failed);
        }
    }
}
