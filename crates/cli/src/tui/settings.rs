use domain::command::{level_named, mode_named};
use std::path::Path;

use app::{Client, Effort, PermissionMode, SessionId, SettingsChange};

use super::store::{Store, apply};

#[derive(Debug, Default, PartialEq)]
pub struct Saved {
    pub model: Option<String>,
    pub mode: Option<PermissionMode>,
    pub effort: Option<Effort>,
    pub auto: bool,
    pub problems: Vec<String>,
}

pub const AUTO_EFFORT: &str = "auto";

pub fn read(settings: &serde_json::Value) -> Saved {
    let mut problems = Vec::new();
    let mut named = |key: &str, choices: &str, known: bool| {
        if let Some(value) = settings[key].as_str().filter(|_| !known) {
            problems.push(format!("The saved {key} {value:?} is not {choices}; using the default."));
        }
    };
    let mode = settings["mode"].as_str().and_then(mode_named);
    let effort = settings["effort"].as_str().and_then(level_named);
    let auto = settings["effort"].as_str() == Some(AUTO_EFFORT);
    named("mode", "ask, edits, plan or bypass", mode.is_some());
    named("effort", "low, medium, high, xhigh, max or auto", effort.is_some() || auto);
    Saved { model: settings["model"].as_str().map(str::to_owned), mode, effort, auto, problems }
}

pub fn last_session(settings: &serde_json::Value, cwd: &Path) -> Option<SessionId> {
    settings["sessions"][cwd.to_string_lossy().as_ref()].as_str().map(|id| SessionId(id.to_owned()))
}

pub fn title(settings: &serde_json::Value, cwd: &Path, session: &SessionId) -> Option<String> {
    let kept = &settings["titles"][cwd.to_string_lossy().as_ref()];
    (kept["session"].as_str() == Some(session.0.as_str())).then(|| kept["title"].as_str().map(str::to_owned))?
}

pub fn or_reported(read: Result<serde_json::Value, String>) -> (serde_json::Value, Option<String>) {
    match read {
        Ok(settings) => (settings, None),
        Err(error) => {
            tracing::warn!(%error, "could not read the settings at startup");
            let told = format!(
                "Could not read your settings ({error}): this chat starts with the defaults. \
                 What is saved stays as it is."
            );
            (serde_json::json!({}), Some(told))
        }
    }
}

const HISTORY: usize = 100;

pub fn history(settings: &serde_json::Value, cwd: &Path) -> Vec<String> {
    settings["history"][cwd.to_string_lossy().as_ref()]
        .as_array()
        .map(|sent| sent.iter().filter_map(|p| p.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

enum Job {
    Change(SettingsChange),
    Flush(crossbeam_channel::Sender<()>),
}

const FLUSH_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

#[derive(Clone)]
pub struct Saver(crossbeam_channel::Sender<Job>);

impl Saver {
    pub fn start(client: &Client) -> Self {
        Self::over(client.clone())
    }

    pub fn over(store: impl Store) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded::<Job>();
        std::thread::spawn(move || {
            let mut waiting: Vec<SettingsChange> = Vec::new();
            while let Ok(job) = rx.recv() {
                let mut flushes = Vec::new();
                for job in std::iter::once(job).chain(rx.try_iter()) {
                    match job {
                        Job::Change(change) => waiting.push(change),
                        Job::Flush(done) => flushes.push(done),
                    }
                }
                apply(&store, &mut waiting);
                for done in flushes {
                    let _ = done.send(());
                }
            }
        });
        Self(tx)
    }

    pub fn flush(&self) {
        let (done, saved) = crossbeam_channel::bounded(1);
        if self.0.send(Job::Flush(done)).is_ok() {
            let _ = saved.recv_timeout(FLUSH_WAIT);
        }
    }

    fn change(&self, change: SettingsChange) {
        let _ = self.0.send(Job::Change(change));
    }

    pub fn remember_value(&self, key: &str, value: serde_json::Value) {
        self.change(SettingsChange::set([key], value));
    }

    pub fn remember(&self, key: &str, value: &str) {
        self.remember_value(key, serde_json::Value::String(value.to_owned()));
    }

    pub fn remember_prompt(&self, cwd: &Path, prompt: &str) {
        let sent = serde_json::Value::String(prompt.to_owned());
        self.change(SettingsChange::latest(["history", cwd.to_string_lossy().as_ref()], sent, HISTORY));
    }

    pub fn remember_session(&self, cwd: &Path, session: &SessionId) {
        let last = serde_json::Value::String(session.0.clone());
        self.change(SettingsChange::set(["sessions", cwd.to_string_lossy().as_ref()], last));
    }

    pub fn remember_title(&self, cwd: &Path, session: &SessionId, title: &str) {
        let named = serde_json::json!({ "session": session.0, "title": title });
        self.change(SettingsChange::set(["titles", cwd.to_string_lossy().as_ref()], named));
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_flush_returns_once_every_queued_change_is_saved() {
        let data = tempfile::tempdir().expect("data");
        let engine = agent_engine::fake::FakeEngine::new(Vec::new(), Vec::new());
        let client = super::super::tests::client(&engine, data.path());
        let saver = Saver::start(&client);
        for at in 0..50 {
            saver.remember("last", &at.to_string());
        }
        saver.flush();
        assert_eq!(client.settings().expect("settings")["last"], json!("49"));
    }

    #[test]
    fn settings_that_cannot_be_read_at_startup_are_reported_not_taken_as_empty_silently() {
        assert_eq!(or_reported(Ok(json!({"mode": "plan"}))), (json!({"mode": "plan"}), None));
        let (settings, told) = or_reported(Err("timed out".into()));
        assert_eq!(settings, json!({}));
        assert!(told.is_some_and(|told| told.contains("timed out") && told.contains("stays as it is")));
    }

    #[test]
    fn saved_choices_read_back_and_unknown_ones_are_ignored() {
        let saved = read(&json!({"model": "keyed:x", "mode": "plan", "effort": "max", "theme": "dark"}));
        assert_eq!(
            saved,
            Saved {
                model: Some("keyed:x".into()),
                mode: Some(PermissionMode::Plan),
                effort: Some(Effort::Max),
                auto: false,
                problems: Vec::new()
            }
        );
        assert_eq!(read(&json!(null)), Saved::default());
    }

    #[test]
    fn a_saved_auto_effort_reads_back_as_auto_without_a_level() {
        let saved = read(&json!({"effort": "auto"}));
        assert_eq!((saved.effort, saved.auto, saved.problems.len()), (None, true, 0));
    }

    #[test]
    fn a_saved_mode_or_effort_that_does_not_exist_is_reported() {
        let saved = read(&json!({"mode": "sideways", "effort": "ultra"}));
        assert_eq!((saved.mode, saved.effort), (None, None));
        assert_eq!(
            saved.problems,
            [
                "The saved mode \"sideways\" is not ask, edits, plan or bypass; using the default.",
                "The saved effort \"ultra\" is not low, medium, high, xhigh, max or auto; using the default.",
            ]
        );
    }

    #[test]
    fn prompts_are_kept_per_folder_newest_last_without_repeats() {
        let data = tempfile::tempdir().expect("data");
        let engine = agent_engine::fake::FakeEngine::new(Vec::new(), Vec::new());
        let client = super::super::tests::client(&engine, data.path());
        client.save_settings(json!({"history": {"/a": ["one", "two"]}})).expect("saved");
        let saver = Saver::start(&client);
        saver.remember_prompt(Path::new("/a"), "one");
        saver.flush();
        assert_eq!(history(&client.settings().expect("settings"), Path::new("/a")), ["two", "one"]);
        saver.remember_prompt(Path::new("/a"), "two");
        saver.flush();
        let settings = client.settings().expect("settings");
        assert_eq!(history(&settings, Path::new("/a")), ["one", "two"]);
        assert!(history(&settings, Path::new("/b")).is_empty());
    }

    #[test]
    fn two_chats_open_at_once_keep_what_each_other_remembers() {
        let data = tempfile::tempdir().expect("data");
        let engine = agent_engine::fake::FakeEngine::new(Vec::new(), Vec::new());
        let chats =
            [super::super::tests::client(&engine, data.path()), super::super::tests::client(&engine, data.path())];
        let savers = [Saver::start(&chats[0]), Saver::start(&chats[1])];
        let (one, two) = (SessionId("s-1".into()), SessionId("s-2".into()));
        for at in 0..20 {
            savers[0].remember("model", "keyed:x");
            savers[0].remember_session(Path::new("/a"), &one);
            savers[0].remember_title(Path::new("/a"), &one, "Login page");
            savers[0].remember_prompt(Path::new("/shared"), &format!("one-{at}"));
            savers[1].remember("effort", "high");
            savers[1].remember_session(Path::new("/b"), &two);
            savers[1].remember_prompt(Path::new("/shared"), &format!("two-{at}"));
        }
        for saver in &savers {
            saver.flush();
        }

        let settings = chats[0].settings().expect("settings");
        assert_eq!((&settings["model"], &settings["effort"]), (&json!("keyed:x"), &json!("high")));
        assert_eq!(last_session(&settings, Path::new("/a")), Some(one.clone()));
        assert_eq!(last_session(&settings, Path::new("/b")), Some(two));
        assert_eq!(title(&settings, Path::new("/a"), &one).as_deref(), Some("Login page"));
        assert_eq!(history(&settings, Path::new("/shared")).len(), 40, "every prompt of both chats");
        assert_eq!(settings["conversations"], json!(null), "the session list lives in the catalog alone");
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
