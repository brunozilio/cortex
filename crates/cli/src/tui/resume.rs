use super::app::{App, Msg};
use super::conversations::{self, Conversation};
use super::picker::{Picker, PickerKind};

pub fn open(app: &mut App, query: Option<String>) {
    if app.busy() {
        return app.transcript.notice("Wait for the agent to finish, or press Esc, before resuming another.");
    }
    let cwd = app.cwd.clone();
    app.spawn(move |client| match here(client, &cwd) {
        Ok(found) => {
            let query = query.unwrap_or_default();
            Msg::Conversations(found.into_iter().filter(|c| titled(c, &query)).collect())
        }
        Err(error) => Msg::Error(error.to_string()),
    });
}

pub fn here(client: &app::Client, cwd: &std::path::Path) -> Result<Vec<Conversation>, app::Error> {
    let real = |path: &std::path::Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let folder = real(cwd);
    let listed = client.sessions()?;
    let here = listed.into_iter().filter(|row| real(&row.cwd) == folder);
    Ok(here.map(|row| Conversation { session: row.session, title: row.title, at: row.updated }).collect())
}

pub fn fork(app: &mut App) {
    match app.session.live().filter(|session| session.id().is_some()) {
        Some(session) => {
            session.fork_next();
            app.transcript.notice("The next message goes on in a copy of this conversation; this one stays as it is.");
        }
        None => app.transcript.notice("Nothing to fork yet: send a message first."),
    }
}

fn titled(conversation: &Conversation, query: &str) -> bool {
    let title = conversation.title.as_deref().unwrap_or_default().to_lowercase();
    query.split_whitespace().all(|word| title.contains(&word.to_lowercase()))
}

pub fn listed(app: &mut App, found: Vec<Conversation>) {
    let current = app.session.id();
    let found: Vec<Conversation> = found.into_iter().filter(|c| Some(&c.session) != current.as_ref()).collect();
    if found.is_empty() {
        return app.transcript.notice("No earlier conversation in this folder matches.");
    }
    let now = conversations::now();
    let items = found
        .iter()
        .map(|c| (c.title.clone().unwrap_or_else(|| "Untitled".to_owned()), conversations::ago(c.at, now)))
        .collect();
    app.catalog.conversations = found;
    app.overlays.picker =
        Some(Picker { kind: PickerKind::Resume, title: "Resume a conversation".into(), items, cursor: 0 });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titled_as(title: Option<&str>) -> Conversation {
        Conversation { session: app::SessionId("s".into()), title: title.map(str::to_owned), at: 0 }
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
