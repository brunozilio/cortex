use domain::SessionId;
use domain::catalog::Listed;
use domain::conversation::SessionStatus;
use serde_json::{Value, json};

pub fn shown(listed: &[Listed], current: Option<&SessionId>) -> Value {
    let own = |entry: &&Listed| current == Some(&entry.session);
    let mut kept: Vec<&Listed> = listed.iter().filter(|entry| own(entry) || is_active(entry)).collect();
    kept.sort_by_key(|entry| std::cmp::Reverse(entry.updated));
    let cards = kept.into_iter().map(|entry| {
        json!({
            "session": entry.session.0,
            "title": entry.title,
            "status": entry.status,
            "updated": entry.updated,
            "cwd": entry.cwd,
            "current": current == Some(&entry.session),
        })
    });
    Value::Array(cards.collect())
}

pub fn active(listed: &[Listed]) -> impl Iterator<Item = &Listed> {
    listed.iter().filter(|entry| is_active(entry))
}

fn is_active(entry: &Listed) -> bool {
    matches!(entry.status, SessionStatus::Running | SessionStatus::Thinking | SessionStatus::NeedsYou)
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
            unread: false,
        }
    }

    #[test]
    fn the_board_shows_the_terminal_s_session_and_the_active_ones_newest_first() {
        let all = [
            listed("a", "/app", 1, SessionStatus::Done),
            listed("b", "/app", 5, SessionStatus::NeedsYou),
            listed("c", "/other", 7, SessionStatus::Running),
            listed("finished", "/app", 8, SessionStatus::Done),
        ];
        let board = shown(&all, Some(&SessionId("a".into())));
        assert_eq!(
            board,
            json!([
                { "session": "c", "title": "about c", "status": "running", "updated": 7, "cwd": "/other", "current": false },
                { "session": "b", "title": "about b", "status": "needs_you", "updated": 5, "cwd": "/app", "current": false },
                { "session": "a", "title": "about a", "status": "done", "updated": 1, "cwd": "/app", "current": true },
            ])
        );
    }

    #[test]
    fn a_session_is_active_while_it_runs_or_waits_for_an_answer() {
        let all = [
            listed("a", "/app", 1, SessionStatus::Running),
            listed("b", "/app", 1, SessionStatus::Done),
            listed("c", "/x", 1, SessionStatus::NeedsYou),
        ];
        let ids: Vec<&str> = active(&all).map(|entry| entry.session.0.as_str()).collect();
        assert_eq!(ids, ["a", "c"]);
    }
}
