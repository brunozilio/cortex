//! `/resume`: going back to an earlier conversation of this folder, picked
//! from the newest.

use super::app::{App, Msg};
use super::choices::{Picker, PickerKind};
use super::conversations::{self, Conversation};

/// Reads the folder's conversations, then offers them.
pub fn open(app: &mut App) {
    if app.busy() {
        return app.transcript.notice("Wait for the agent to finish, or press Esc, before resuming another.");
    }
    let cwd = app.cwd.clone();
    app.spawn(move |sdk| match sdk.settings() {
        Ok(settings) => Msg::Conversations(conversations::list(&settings, &cwd)),
        Err(error) => Msg::Error(error.to_string()),
    });
}

pub fn listed(app: &mut App, found: Vec<Conversation>) {
    let current = app.session.as_ref().and_then(sdk::Session::id);
    let found: Vec<Conversation> = found.into_iter().filter(|c| Some(&c.session) != current.as_ref()).collect();
    if found.is_empty() {
        return app.transcript.notice("No earlier conversation in this folder.");
    }
    let now = conversations::now();
    let items = found
        .iter()
        .map(|c| (c.title.clone().unwrap_or_else(|| "Untitled".to_owned()), conversations::ago(c.at, now)))
        .collect();
    app.conversations = found;
    app.picker = Some(Picker { kind: PickerKind::Resume, title: "Resume a conversation".into(), items, cursor: 0 });
}

impl App {
    pub(super) fn resume_picked(&mut self, cursor: usize) {
        let Some(picked) = self.conversations.get(cursor).cloned() else { return };
        self.resume = Some(picked.session);
        self.new_session();
        self.title = picked.title;
        self.transcript.notice("Continuing an earlier conversation.");
    }
}
