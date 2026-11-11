use domain::command::{level_named, mode_named};
use std::path::Path;

use sdk::{Effort, PermissionMode, Sdk, SessionId};

#[derive(Debug, Default, PartialEq)]
pub struct Saved {
    pub model: Option<String>,
    pub mode: Option<PermissionMode>,
    pub effort: Option<Effort>,
    pub problems: Vec<String>,
}

pub fn read(settings: &serde_json::Value) -> Saved {
    let mut problems = Vec::new();
    let mut named = |key: &str, choices: &str, known: bool| {
        if let Some(value) = settings[key].as_str().filter(|_| !known) {
            problems.push(format!("The saved {key} {value:?} is not {choices}; using the default."));
        }
    };
    let mode = settings["mode"].as_str().and_then(mode_named);
    let effort = settings["effort"].as_str().and_then(level_named);
    named("mode", "ask, edits, plan or bypass", mode.is_some());
    named("effort", "low, medium, high, xhigh or max", effort.is_some());
    Saved { model: settings["model"].as_str().map(str::to_owned), mode, effort, problems }
}

pub fn last_session(settings: &serde_json::Value, cwd: &Path) -> Option<SessionId> {
    settings["sessions"][cwd.to_string_lossy().as_ref()].as_str().map(|id| SessionId(id.to_owned()))
}

pub fn title(settings: &serde_json::Value, cwd: &Path, session: &SessionId) -> Option<String> {
    let kept = &settings["titles"][cwd.to_string_lossy().as_ref()];
    (kept["session"].as_str() == Some(session.0.as_str())).then(|| kept["title"].as_str().map(str::to_owned))?
}

const HISTORY: usize = 100;

pub fn history(settings: &serde_json::Value, cwd: &Path) -> Vec<String> {
    settings["history"][cwd.to_string_lossy().as_ref()]
        .as_array()
        .map(|sent| sent.iter().filter_map(|p| p.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

type Change = Box<dyn FnOnce(&mut serde_json::Value) + Send>;

#[derive(Clone)]
pub struct Saver(crossbeam_channel::Sender<Change>);

impl Saver {
    pub fn start(sdk: &Sdk) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded::<Change>();
        let sdk = sdk.clone();
        std::thread::spawn(move || {
            while let Ok(change) = rx.recv() {
                let mut settings = sdk.settings().unwrap_or_else(|_| serde_json::json!({}));
                if !settings.is_object() {
                    settings = serde_json::json!({});
                }
                change(&mut settings);
                while let Ok(more) = rx.try_recv() {
                    more(&mut settings);
                }
                let _ = sdk.save_settings(settings);
            }
        });
        Self(tx)
    }

    fn change(&self, change: impl FnOnce(&mut serde_json::Value) + Send + 'static) {
        let _ = self.0.send(Box::new(change));
    }

    pub fn remember(&self, key: &str, value: &str) {
        let (key, value) = (key.to_owned(), value.to_owned());
        self.change(move |settings| settings[key] = serde_json::Value::String(value));
    }

    pub fn remember_prompt(&self, cwd: &Path, prompt: &str) {
        let (cwd, prompt) = (cwd.to_owned(), prompt.to_owned());
        self.change(move |settings| add_prompt(settings, &cwd, prompt));
    }

    pub fn remember_session(&self, cwd: &Path, session: &SessionId) {
        let (cwd, session) = (cwd.to_string_lossy().into_owned(), session.0.clone());
        self.change(move |settings| {
            if !settings["sessions"].is_object() {
                settings["sessions"] = serde_json::json!({});
            }
            settings["sessions"][cwd.as_str()] = serde_json::Value::String(session.clone());
            let now = super::conversations::now();
            super::conversations::touch(settings, Path::new(&cwd), &SessionId(session), None, now);
        });
    }

    pub fn remember_title(&self, cwd: &Path, session: &SessionId, title: &str) {
        let (cwd, session, title) = (cwd.to_string_lossy().into_owned(), session.0.clone(), title.to_owned());
        self.change(move |settings| {
            if !settings["titles"].is_object() {
                settings["titles"] = serde_json::json!({});
            }
            settings["titles"][cwd.as_str()] = serde_json::json!({ "session": session, "title": title });
            let now = super::conversations::now();
            super::conversations::touch(settings, Path::new(&cwd), &SessionId(session), Some(&title), now);
        });
    }
}

fn add_prompt(settings: &mut serde_json::Value, cwd: &Path, prompt: String) {
    let mut sent = history(settings, cwd);
    sent.retain(|known| *known != prompt);
    sent.push(prompt);
    let drop = sent.len().saturating_sub(HISTORY);
    sent.drain(..drop);
    if !settings["history"].is_object() {
        settings["history"] = serde_json::json!({});
    }
    settings["history"][cwd.to_string_lossy().as_ref()] = serde_json::json!(sent);
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn saved_choices_read_back_and_unknown_ones_are_ignored() {
        let saved = read(&json!({"model": "keyed:x", "mode": "plan", "effort": "max", "theme": "dark"}));
        assert_eq!(
            saved,
            Saved {
                model: Some("keyed:x".into()),
                mode: Some(PermissionMode::Plan),
                effort: Some(Effort::Max),
                problems: Vec::new()
            }
        );
        assert_eq!(read(&json!(null)), Saved::default());
    }

    #[test]
    fn a_saved_mode_or_effort_that_does_not_exist_is_reported() {
        let saved = read(&json!({"mode": "sideways", "effort": "ultra"}));
        assert_eq!((saved.mode, saved.effort), (None, None));
        assert_eq!(
            saved.problems,
            [
                "The saved mode \"sideways\" is not ask, edits, plan or bypass; using the default.",
                "The saved effort \"ultra\" is not low, medium, high, xhigh or max; using the default.",
            ]
        );
    }

    #[test]
    fn prompts_are_kept_per_folder_newest_last_without_repeats() {
        let mut settings = json!({"history": {"/a": ["one", "two"]}});
        add_prompt(&mut settings, Path::new("/a"), "one".into());
        assert_eq!(history(&settings, Path::new("/a")), ["two", "one"]);
        add_prompt(&mut settings, Path::new("/a"), "two".into());
        assert_eq!(history(&settings, Path::new("/a")), ["one", "two"]);
        assert!(history(&settings, Path::new("/b")).is_empty());
    }

    #[test]
    fn a_title_belongs_to_its_session() {
        let settings = json!({"titles": {"/a": {"session": "s-1", "title": "Login page"}}});
        let named = |cwd: &str, id: &str| title(&settings, Path::new(cwd), &SessionId(id.into()));
        assert_eq!(named("/a", "s-1").as_deref(), Some("Login page"));
        assert_eq!(named("/a", "s-2"), None, "another conversation");
        assert_eq!(named("/b", "s-1"), None);
    }

    #[test]
    fn the_last_conversation_is_kept_per_folder() {
        let settings = json!({"sessions": {"/a": "s-1"}});
        assert_eq!(last_session(&settings, Path::new("/a")), Some(SessionId("s-1".into())));
        assert_eq!(last_session(&settings, Path::new("/b")), None);
    }
}
