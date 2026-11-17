use domain::Rewound;

use super::app::App;
use super::picker::{Picker, PickerKind};
use super::transcript::Entry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    pub index: u32,
    pub prompt: String,
    pub entry: usize,
}

impl App {
    pub(super) fn checkpointed(&mut self, index: u32) {
        let Some((entry, prompt)) =
            self.transcript.entries.iter().enumerate().rev().find_map(|(at, entry)| match entry {
                Entry::User(text) => Some((at, text.clone())),
                _ => None,
            })
        else {
            return;
        };
        self.turn.checkpoints.retain(|known| known.index < index);
        self.turn.checkpoints.push(Checkpoint { index, prompt, entry });
    }

    pub(super) fn open_rewind(&mut self) {
        if self.busy() {
            return self.transcript.notice("Wait for the agent to finish, or press Esc, before rewinding.");
        }
        if self.turn.checkpoints.is_empty() {
            return self.transcript.notice("Nothing to rewind to in this chat yet.");
        }
        let items = self
            .turn
            .checkpoints
            .iter()
            .rev()
            .map(|checkpoint| (first_line(&checkpoint.prompt), String::new()))
            .collect();
        let title = "Rewind to before…".into();
        self.overlays.picker = Some(Picker { kind: PickerKind::Rewind, title, items, cursor: 0 });
    }

    pub(super) fn rewind_picked(&mut self, cursor: usize) {
        let newest_first = self.turn.checkpoints.len().checked_sub(cursor + 1);
        if let Some(checkpoint) = newest_first.and_then(|at| self.turn.checkpoints.get(at)).cloned() {
            self.rewind_to(checkpoint.index, false);
        }
    }

    pub(super) fn retry(&mut self) {
        if self.busy() {
            return self.transcript.notice("The agent is still working: Esc stops it, then /retry.");
        }
        match self.turn.checkpoints.last().map(|checkpoint| checkpoint.index) {
            Some(index) => self.rewind_to(index, true),
            None => match self.transcript.last_prompt() {
                Some(prompt) => self.send(prompt),
                None => self.transcript.notice("Nothing to retry yet."),
            },
        }
    }

    fn rewind_to(&mut self, index: u32, again: bool) {
        let Some(session) = self.session.live() else { return };
        let started = session.rewind(index);
        self.turn.retrying = again;
        self.follow(started);
    }

    pub(super) fn branched(&mut self, checkpoint: u32, prompt: &str) {
        let at = self.turn.checkpoints.iter().position(|known| known.index == checkpoint);
        if let Some(at) = at {
            let entry = self.turn.checkpoints[at].entry;
            self.transcript.entries.truncate(entry);
            self.turn.checkpoints.truncate(at);
        }
        self.transcript.notice(
            "Branched: this is a new conversation from before that prompt; the files are as they were. The \
             original stays in /resume.",
        );
        self.prompt.input.set(prompt);
    }

    pub(super) fn rewound(&mut self, rewound: Rewound) {
        let at = self.turn.checkpoints.iter().position(|known| known.index == rewound.checkpoint);
        if let Some(at) = at {
            let entry = self.turn.checkpoints[at].entry;
            self.transcript.entries.truncate(entry);
            self.turn.checkpoints.truncate(at);
        }
        let files = match rewound.restored.len() {
            0 => "no files to restore".to_owned(),
            1 => "1 file restored".to_owned(),
            count => format!("{count} files restored"),
        };
        let kept = if rewound.conversation { "" } else { "; the conversation was compacted since, so it is kept" };
        self.transcript.notice(format!("Rewound: {files}{kept}. Changes made with Bash are not undone."));
        if !rewound.skipped.is_empty() {
            let named: Vec<String> = rewound
                .skipped
                .iter()
                .map(|skipped| format!("{} ({})", skipped.path.display(), skipped.why.said()))
                .collect();
            self.transcript.notice(format!("Left as they are: {}", named.join("; ")));
        }
        if std::mem::take(&mut self.turn.retrying) {
            self.send(rewound.prompt);
        } else {
            self.prompt.input.set(&rewound.prompt);
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
