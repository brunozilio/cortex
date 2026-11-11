use super::app::{App, Msg};
use super::choices::{Picker, PickerKind};
use super::conversations::{self, Conversation};

pub fn open(app: &mut App, query: Option<String>) {
    if app.busy() {
        return app.transcript.notice("Wait for the agent to finish, or press Esc, before resuming another.");
    }
    let cwd = app.cwd.clone();
    app.spawn(move |sdk| match sdk.settings() {
        Ok(settings) => {
            let found = conversations::list(&settings, &cwd);
            let query = query.unwrap_or_default();
            Msg::Conversations(found.into_iter().filter(|c| titled(c, &query)).collect())
        }
        Err(error) => Msg::Error(error.to_string()),
    });
}

fn titled(conversation: &Conversation, query: &str) -> bool {
    let title = conversation.title.as_deref().unwrap_or_default().to_lowercase();
    query.split_whitespace().all(|word| title.contains(&word.to_lowercase()))
}

pub fn listed(app: &mut App, found: Vec<Conversation>) {
    let current = app.session.as_ref().and_then(sdk::Session::id);
    let found: Vec<Conversation> = found.into_iter().filter(|c| Some(&c.session) != current.as_ref()).collect();
    if found.is_empty() {
        return app.transcript.notice("No earlier conversation in this folder matches.");
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

#[cfg(test)]
mod tests {
    use super::*;

    fn titled_as(title: Option<&str>) -> Conversation {
        Conversation { session: sdk::SessionId("s".into()), title: title.map(str::to_owned), at: 0 }
    }

    #[test]
    fn every_word_of_the_search_is_in_the_title_whatever_its_case() {
        let fix = titled_as(Some("Fix the Login bug"));
        assert!(titled(&fix, ""));
        assert!(titled(&fix, "login FIX"));
        assert!(!titled(&fix, "login crash"));
        assert!(!titled(&titled_as(None), "login"), "an untitled one matches only an empty search");
    }
}
