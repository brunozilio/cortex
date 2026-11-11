//! The conversations had in each folder, newest first, for `/resume` and
//! `superfik --resume`: their session, title and when they were last used.

use std::path::Path;

use serde_json::{Value, json};
use sdk::SessionId;

/// Conversations kept per folder.
const KEPT: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    pub session: SessionId,
    pub title: Option<String>,
    /// Seconds since the epoch.
    pub at: u64,
}

/// The conversations of `cwd`, newest first.
pub fn list(settings: &Value, cwd: &Path) -> Vec<Conversation> {
    settings["conversations"][cwd.to_string_lossy().as_ref()]
        .as_array()
        .map(|kept| {
            kept.iter()
                .filter_map(|c| {
                    Some(Conversation {
                        session: SessionId(c["session"].as_str()?.to_owned()),
                        title: c["title"].as_str().map(str::to_owned),
                        at: c["at"].as_u64().unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Notes that `session` was used in `cwd` at `at` (and its title, when
/// given), moving it to the front.
pub fn touch(settings: &mut Value, cwd: &Path, session: &SessionId, title: Option<&str>, at: u64) {
    let mut kept = list(settings, cwd);
    let title =
        title.map(str::to_owned).or_else(|| kept.iter().find(|c| &c.session == session).and_then(|c| c.title.clone()));
    kept.retain(|c| &c.session != session);
    kept.insert(0, Conversation { session: session.clone(), title, at });
    kept.truncate(KEPT);
    if !settings["conversations"].is_object() {
        settings["conversations"] = json!({});
    }
    settings["conversations"][cwd.to_string_lossy().as_ref()] = Value::Array(
        kept.into_iter().map(|c| json!({ "session": c.session.0, "title": c.title, "at": c.at })).collect(),
    );
}

/// When a conversation was last used, said shortly ("3h ago").
pub fn ago(at: u64, now: u64) -> String {
    let seconds = now.saturating_sub(at);
    match seconds {
        0..60 => "just now".to_owned(),
        60..3_600 => format!("{}m ago", seconds / 60),
        3_600..86_400 => format!("{}h ago", seconds / 3_600),
        _ => format!("{}d ago", seconds / 86_400),
    }
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversations_are_kept_per_folder_newest_first_with_their_titles() {
        let mut settings = json!({});
        let (a, b) = (SessionId("a".into()), SessionId("b".into()));
        touch(&mut settings, Path::new("/p"), &a, None, 10);
        touch(&mut settings, Path::new("/p"), &b, Some("Login page"), 20);
        touch(&mut settings, Path::new("/p"), &a, Some("Fix tests"), 30);
        touch(&mut settings, Path::new("/p"), &b, None, 40);
        let kept = list(&settings, Path::new("/p"));
        assert_eq!(kept.iter().map(|c| c.session.0.as_str()).collect::<Vec<_>>(), ["b", "a"]);
        assert_eq!(kept[0].title.as_deref(), Some("Login page"), "a title stays when touched without one");
        assert_eq!(kept[1].title.as_deref(), Some("Fix tests"));
        assert!(list(&settings, Path::new("/q")).is_empty());
        assert_eq!(ago(100, 100 + 7_200), "2h ago");
    }
}
