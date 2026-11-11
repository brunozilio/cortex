//! Going back: `/rewind` picks an earlier prompt of this chat and restores
//! the files the agent edited since and the conversation before it (the
//! prompt comes back in the input to edit); `/retry` does that for the last
//! prompt and sends it again.

use domain::Rewound;

use super::app::App;
use super::choices::{Picker, PickerKind};
use super::transcript::Entry;

/// A prompt of this chat the engine can rewind to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    pub index: u32,
    pub prompt: String,
    /// Where its prompt sits in the transcript.
    pub entry: usize,
}

impl App {
    /// The running prompt opened checkpoint `index`.
    pub(super) fn checkpointed(&mut self, index: u32) {
        let Some((entry, prompt)) =
            self.transcript.entries.iter().enumerate().rev().find_map(|(at, entry)| match entry {
                Entry::User(text) => Some((at, text.clone())),
                _ => None,
            })
        else {
            return;
        };
        self.checkpoints.retain(|known| known.index < index);
        self.checkpoints.push(Checkpoint { index, prompt, entry });
    }

    pub(super) fn open_rewind(&mut self) {
        if self.busy() {
            return self.transcript.notice("Wait for the agent to finish, or press Esc, before rewinding.");
        }
        if self.checkpoints.is_empty() {
            return self.transcript.notice("Nothing to rewind to in this chat yet.");
        }
        let items =
            self.checkpoints.iter().rev().map(|checkpoint| (first_line(&checkpoint.prompt), String::new())).collect();
        let title = "Rewind to before…".into();
        self.picker = Some(Picker { kind: PickerKind::Rewind, title, items, cursor: 0 });
    }

    pub(super) fn rewind_picked(&mut self, cursor: usize) {
        let newest_first = self.checkpoints.len().checked_sub(cursor + 1);
        if let Some(checkpoint) = newest_first.and_then(|at| self.checkpoints.get(at)).cloned() {
            self.rewind_to(checkpoint.index, false);
        }
    }

    pub(super) fn retry(&mut self) {
        if self.busy() {
            return self.transcript.notice("The agent is still working: Esc stops it, then /retry.");
        }
        match self.checkpoints.last().map(|checkpoint| checkpoint.index) {
            Some(index) => self.rewind_to(index, true),
            None => match self.transcript.last_prompt() {
                Some(prompt) => self.send(prompt),
                None => self.transcript.notice("Nothing to retry yet."),
            },
        }
    }

    fn rewind_to(&mut self, index: u32, again: bool) {
        let Some(session) = &self.session else { return };
        let started = session.rewind(index);
        self.retrying = again;
        self.follow(started);
    }

    /// The rewind run ended with `rewound`.
    pub(super) fn rewound(&mut self, rewound: Rewound) {
        let at = self.checkpoints.iter().position(|known| known.index == rewound.checkpoint);
        if let Some(at) = at {
            let entry = self.checkpoints[at].entry;
            self.transcript.entries.truncate(entry);
            self.checkpoints.truncate(at);
        }
        let files = match rewound.restored.len() {
            0 => "no files to restore".to_owned(),
            1 => "1 file restored".to_owned(),
            count => format!("{count} files restored"),
        };
        let kept = if rewound.conversation { "" } else { "; the conversation was compacted since, so it is kept" };
        self.transcript.notice(format!("Rewound: {files}{kept}. Changes made with Bash are not undone."));
        if std::mem::take(&mut self.retrying) {
            self.send(rewound.prompt);
        } else {
            self.input.set(&rewound.prompt);
        }
    }
}

fn first_line(prompt: &str) -> String {
    let line = prompt.lines().next().unwrap_or_default();
    if line.chars().count() > 70 {
        format!("{}…", line.chars().take(69).collect::<String>())
    } else {
        line.to_owned()
    }
}
