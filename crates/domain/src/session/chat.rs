use serde_json::{Value, json};

use crate::catalog::Listed;
use crate::conversation::{SessionStatus, Transcript};
use crate::{Decision, PermissionMode};

pub const SHOWN_ENTRIES: usize = 300;
pub const SHOWN_CHATS: usize = 100;

pub fn decision(named: &str) -> Option<Decision> {
    match named {
        "allow" => Some(Decision::Allow),
        "always" => Some(Decision::AllowAlways),
        "deny" => Some(Decision::Deny),
        "approve" => Some(Decision::Approve(PermissionMode::AcceptEdits)),
        _ => None,
    }
}

pub fn takes_prompt(status: SessionStatus) -> bool {
    !matches!(status, SessionStatus::Running | SessionStatus::Thinking | SessionStatus::NeedsYou)
}

pub fn listed(rows: &[Listed]) -> Value {
    let mut rows: Vec<&Listed> = rows.iter().collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.updated));
    let cards = rows.into_iter().take(SHOWN_CHATS).map(|row| {
        json!({
            "session": row.session.0,
            "title": row.title,
            "status": row.status,
            "updated": row.updated,
            "folder": row.cwd,
        })
    });
    Value::Array(cards.collect())
}

pub fn view(session: &str, title: Option<&str>, transcript: &Transcript) -> Value {
    let entries = &transcript.entries;
    let shown = &entries[entries.len().saturating_sub(SHOWN_ENTRIES)..];
    json!({
        "session": session,
        "title": title,
        "status": transcript.status(),
        "entries": shown,
        "ask": transcript.ask().map(|pending| &pending.ask),
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::conversation::Entry;
    use crate::{AgentAsk, AskId, AskKind, EngineEvent, SessionId};

    fn row(id: &str, updated: u64, status: SessionStatus) -> Listed {
        Listed {
            session: SessionId(id.into()),
            cwd: PathBuf::from("/work"),
            title: Some(format!("chat {id}")),
            created: 0,
            updated,
            status,
            unread: false,
        }
    }

    #[test]
    fn a_phone_answers_with_a_few_named_decisions() {
        assert_eq!(decision("allow"), Some(Decision::Allow));
        assert_eq!(decision("always"), Some(Decision::AllowAlways));
        assert_eq!(decision("deny"), Some(Decision::Deny));
        assert_eq!(decision("approve"), Some(Decision::Approve(PermissionMode::AcceptEdits)));
        assert_eq!(decision("sure"), None);
    }

    #[test]
    fn a_chat_takes_a_new_prompt_only_when_nothing_runs_in_it() {
        assert!(takes_prompt(SessionStatus::Idle) && takes_prompt(SessionStatus::Done));
        assert!(takes_prompt(SessionStatus::Failed) && takes_prompt(SessionStatus::Cancelled));
        assert!(!takes_prompt(SessionStatus::Running) && !takes_prompt(SessionStatus::NeedsYou));
    }

    #[test]
    fn the_chat_list_puts_the_latest_first() {
        let shown = listed(&[row("old", 1, SessionStatus::Done), row("new", 9, SessionStatus::Running)]);
        assert_eq!(shown[0]["session"], "new");
        assert_eq!(shown[0]["status"], "running");
        assert_eq!(shown[1]["title"], "chat old");
        assert_eq!(shown[1]["folder"], "/work");
    }

    #[test]
    fn a_chat_shows_its_last_entries_and_the_question_waiting_for_the_owner() {
        let mut transcript = Transcript::default();
        for at in 0..SHOWN_ENTRIES + 5 {
            transcript.push(Entry::Notice(format!("n{at}")));
        }
        let ask = AgentAsk {
            id: AskId("a1".into()),
            kind: AskKind::Permission { tool: "Bash".into(), subject: "ls".into(), rememberable: true, rule: None },
        };
        transcript.begin();
        transcript.apply(&EngineEvent::Asked(ask));

        let shown = view("s1", Some("Fix it"), &transcript);
        let entries = shown["entries"].as_array().expect("entries");
        assert_eq!(entries.len(), SHOWN_ENTRIES);
        assert_eq!(entries[0], json!({ "kind": "notice", "data": "n5" }));
        assert_eq!(shown["status"], "needs_you");
        assert_eq!(shown["ask"]["id"], "a1");
        assert_eq!(shown["ask"]["kind"]["Permission"]["tool"], "Bash");
    }
}
