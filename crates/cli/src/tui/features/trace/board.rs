use std::path::Path;

use domain::SessionId;
use domain::catalog::Listed;
use domain::conversation::SessionStatus;
use serde_json::{Value, json};

pub fn shown(listed: &[Listed], cwd: &Path, current: Option<&SessionId>) -> Value {
    let mut here: Vec<&Listed> = listed.iter().filter(|entry| entry.cwd == cwd && !entry.archived).collect();
    here.sort_by_key(|entry| std::cmp::Reverse(entry.updated));
    let cards = here.into_iter().map(|entry| {
        json!({
            "session": entry.session.0,
            "title": entry.title,
            "status": entry.status,
            "updated": entry.updated,
            "current": current == Some(&entry.session),
        })
    });
    Value::Array(cards.collect())
}

pub fn active(listed: &[Listed], session: &str) -> bool {
    listed.iter().any(|entry| {
        entry.session.0 == session
            && matches!(entry.status, SessionStatus::Running | SessionStatus::Thinking | SessionStatus::NeedsYou)
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn listed(id: &str, cwd: &str, updated: u64, status: SessionStatus) -> Listed {
        Listed {
            session: SessionId(id.into()),
            cwd: PathBuf::from(cwd),
            title: Some(format!("about {id}")),
            created: 1,
            updated,
            status,
            archived: false,
            unread: false,
        }
    }

    #[test]
    fn the_board_shows_this_folder_s_sessions_newest_first_and_marks_the_terminal_s_own() {
        let mut shelved = listed("old", "/app", 9, SessionStatus::Done);
        shelved.archived = true;
        let all = [
            listed("a", "/app", 1, SessionStatus::Done),
            listed("b", "/app", 5, SessionStatus::NeedsYou),
            listed("c", "/other", 7, SessionStatus::Running),
            shelved,
        ];
        let board = shown(&all, Path::new("/app"), Some(&SessionId("a".into())));
        assert_eq!(
            board,
            json!([
                { "session": "b", "title": "about b", "status": "needs_you", "updated": 5, "current": false },
                { "session": "a", "title": "about a", "status": "done", "updated": 1, "current": true },
            ])
        );
    }

    #[test]
    fn a_session_is_active_while_it_runs_or_waits_for_an_answer() {
        let all = [listed("a", "/app", 1, SessionStatus::Running), listed("b", "/app", 1, SessionStatus::Done)];
        assert!(active(&all, "a"));
        assert!(!active(&all, "b"));
        assert!(!active(&all, "missing"));
    }
}
