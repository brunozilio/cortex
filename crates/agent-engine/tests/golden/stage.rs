use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crossbeam_channel::unbounded;
use std::time::Duration;

use agent_engine::model::scripted::ScriptedModel;
use agent_engine::model::{ModelApi, ModelError, Reply};
use agent_engine::sessions::{FileSessions, Sessions};
use agent_engine::{AgentEngine, ApiSource, Engine};
use domain::{EngineEvent, Extensions, ModelOption, PromptRequest, RunId, SessionId};

use crate::render;
use crate::run::Run;

pub const PATIENCE: Duration = Duration::from_secs(120);

#[derive(Default)]
struct Models(Mutex<VecDeque<Arc<dyn ModelApi>>>);

impl ApiSource for Models {
    fn api(&self, _request: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError> {
        let next = self.0.lock().expect("models").pop_front();
        next.ok_or_else(|| ModelError::Unauthorized("no account connected".into()))
    }
}

pub struct Stage {
    root: tempfile::TempDir,
    pub project: PathBuf,
    pub home: PathBuf,
    pub store: PathBuf,
    pub sessions: Arc<FileSessions>,
    models: Arc<Models>,
    engine: Engine,
    known: Mutex<Vec<SessionId>>,
    logged: Mutex<HashMap<SessionId, usize>>,
    transcript: Mutex<Vec<String>>,
}

#[derive(Clone, Copy)]
pub enum Order {
    Emitted,
    PerCall,
    PerLane,
}

impl Stage {
    pub fn new() -> Self {
        let root = tempfile::tempdir().expect("stage");
        let base = root.path().canonicalize().expect("canonical stage");
        let (project, home, store) = (base.join("project"), base.join("home"), base.join("sessions"));
        for dir in [&project, &home, &store] {
            std::fs::create_dir_all(dir).expect("stage folder");
        }
        let sessions = Arc::new(FileSessions::new(&store));
        let models = Arc::new(Models::default());
        let engine = Engine::new(Arc::clone(&models) as Arc<dyn ApiSource>, Arc::clone(&sessions) as _);
        Self {
            root,
            project,
            home,
            store,
            sessions,
            models,
            engine,
            known: Mutex::default(),
            logged: Mutex::default(),
            transcript: Mutex::default(),
        }
    }

    pub fn script(&self, replies: impl IntoIterator<Item = Result<Reply, ModelError>>) -> Arc<ScriptedModel> {
        let model = Arc::new(ScriptedModel::new(replies));
        self.model(Arc::clone(&model) as Arc<dyn ModelApi>);
        model
    }

    pub fn model(&self, model: Arc<dyn ModelApi>) {
        self.models.0.lock().expect("models").push_back(model);
    }

    pub fn request(&self, prompt: &str) -> PromptRequest {
        let mut request =
            PromptRequest::plain(self.project.clone(), prompt, ModelOption::new("claude-opus-5-5", "Opus 5.5"));
        request.host.extensions = Some(Extensions { home: Some(self.home.clone()), ..Extensions::default() });
        request
    }

    pub fn trusted(&self, prompt: &str) -> PromptRequest {
        let mut request = self.request(prompt);
        request.host.extensions = Some(Extensions {
            home: Some(self.home.clone()),
            trust: domain::trust::Trust::Trusted,
            ..Extensions::default()
        });
        request
    }

    pub fn resuming(&self, session: &SessionId, prompt: &str) -> PromptRequest {
        let mut request = self.request(prompt);
        request.session.resume = Some(session.clone());
        request
    }

    pub fn settings(&self, settings: &serde_json::Value) {
        let dir = self.home.join(brand::DIR);
        std::fs::create_dir_all(&dir).expect("settings folder");
        std::fs::write(dir.join("settings.json"), settings.to_string()).expect("settings");
    }

    pub fn file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.project.join(name);
        std::fs::write(&path, content).expect("project file");
        path
    }

    pub fn start(&self, request: &PromptRequest) -> Run {
        let (send, events) = unbounded();
        let send = Mutex::new(send);
        let sink = Arc::new(move |_run: RunId, event: EngineEvent| {
            let _ = send.lock().expect("send").send(event);
        });
        let handle = self.engine.start(RunId::new(), request, sink).expect("start");
        Run::open(request, events, handle)
    }

    pub fn play(&self, label: &str, request: &PromptRequest) -> SessionId {
        self.end(label, self.start(request))
    }

    pub fn prepare(&self, request: &PromptRequest) -> SessionId {
        let (request, events) = self.start(request).finish();
        assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "the preparing run failed: {events:#?}");
        let session = events
            .iter()
            .find_map(|event| match event {
                EngineEvent::SessionStarted(id) => Some(id.clone()),
                _ => None,
            })
            .expect("the preparing run starts a session");
        self.know(&session);
        let entries = self.sessions.entries(&request.session.cwd, &session).expect("readable log");
        self.logged.lock().expect("logged").insert(session.clone(), entries.len());
        session
    }

    pub fn end(&self, label: &str, run: Run) -> SessionId {
        self.end_ordered(label, run, Order::Emitted)
    }

    pub fn end_ordered(&self, label: &str, run: Run, order: Order) -> SessionId {
        let (request, events) = run.finish();
        let started = events.iter().rev().find_map(|event| match event {
            EngineEvent::SessionStarted(id) => Some(id.clone()),
            _ => None,
        });
        if let Some(id) = &started {
            self.know(id);
        }
        let lines = match order {
            Order::Emitted => render::events(&events),
            Order::PerCall => render::calls_grouped(&events),
            Order::PerLane => render::lanes(&events),
        };
        self.section(&format!("{label}: events"), lines);
        let session = started.or_else(|| request.session.resume.clone()).unwrap_or_else(|| SessionId("none".into()));
        if session.0 != "none" {
            self.saved_ordered(label, &request, &session, order);
        }
        session
    }

    pub fn saved(&self, label: &str, request: &PromptRequest, session: &SessionId) {
        self.saved_ordered(label, request, session, Order::Emitted);
    }

    fn saved_ordered(&self, label: &str, request: &PromptRequest, session: &SessionId, order: Order) {
        self.know(session);
        let history = self.sessions.load_for_checked(&request.session.cwd, session).expect("readable session");
        let conversation = match &history {
            Some(history) if history.is_empty() => vec!["(empty)".to_owned()],
            Some(history) => render::conversation(history),
            None => vec!["(nothing kept)".to_owned()],
        };
        self.section(&format!("{label}: conversation"), conversation);
        let entries = self.sessions.entries(&request.session.cwd, session).expect("readable log");
        let mut logged = self.logged.lock().expect("logged");
        let before = logged.insert(session.clone(), entries.len()).unwrap_or(0);
        let fresh = &entries[before.min(entries.len())..];
        let appended = match order {
            Order::Emitted => render::log(fresh),
            Order::PerCall => render::log_calls_grouped(fresh),
            Order::PerLane => render::log_lanes(fresh),
        };
        drop(logged);
        self.section(
            &format!("{label}: log appended"),
            if appended.is_empty() { vec!["(nothing)".to_owned()] } else { appended },
        );
    }

    pub fn checkpoints(&self, label: &str, request: &PromptRequest, session: &SessionId) {
        let kept = self.sessions.load_checkpoints(&request.session.cwd, session).expect("checkpoints");
        let lines = kept
            .iter()
            .enumerate()
            .map(|(index, checkpoint)| {
                let files: Vec<String> = checkpoint.files.keys().map(|path| path.display().to_string()).collect();
                format!(
                    "{index}: prompt={:?} history_len={} files=[{}]",
                    checkpoint.prompt,
                    checkpoint.history_len,
                    files.join(", ")
                )
            })
            .collect::<Vec<_>>();
        self.section(
            &format!("{label}: checkpoints"),
            if lines.is_empty() { vec!["(none)".to_owned()] } else { lines },
        );
    }

    pub fn files(&self, label: &str) {
        let mut kept: Vec<PathBuf> = std::fs::read_dir(&self.store)
            .expect("store")
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "jsonl"))
            .collect();
        kept.sort();
        for path in kept {
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            let stream = if name.ends_with(".trace.jsonl") { "trace file" } else { "conversation file" };
            let text = std::fs::read_to_string(&path).expect("a session file");
            self.section(&format!("{label}: {stream}"), text.lines().map(render::line).collect());
        }
    }

    pub fn note(&self, title: &str, lines: impl IntoIterator<Item = String>) {
        self.section(title, lines.into_iter().collect());
    }

    pub fn disk(&self, title: &str, path: &Path) {
        let content = std::fs::read_to_string(path).unwrap_or_else(|_| "(absent)".to_owned());
        let mut transcript = self.transcript.lock().expect("transcript");
        transcript.push(format!("== {title} =="));
        transcript.extend(content.lines().map(|line| self.scrubbed(line)));
    }

    fn section(&self, title: &str, lines: Vec<String>) {
        let mut transcript = self.transcript.lock().expect("transcript");
        transcript.push(format!("== {title} =="));
        transcript.extend(lines.iter().map(|line| render::clipped(&self.scrubbed(line))));
    }

    fn know(&self, session: &SessionId) {
        let mut known = self.known.lock().expect("known");
        if !known.contains(session) {
            known.push(session.clone());
        }
    }

    fn scrubbed(&self, line: &str) -> String {
        let mut line = line.to_owned();
        let temp = std::env::temp_dir();
        let places = [
            (self.project.clone(), "<project>"),
            (self.root.path().join("project"), "<project>"),
            (self.home.clone(), "<home>"),
            (self.root.path().join("home"), "<home>"),
            (self.store.clone(), "<sessions>"),
            (self.root.path().join("sessions"), "<sessions>"),
            (temp.canonicalize().unwrap_or_else(|_| temp.clone()), "<tmp>"),
            (temp.clone(), "<tmp>"),
        ];
        for (path, name) in places {
            let path = path.display().to_string();
            line = line.replace(path.trim_end_matches('/'), name);
        }
        for (index, session) in self.known.lock().expect("known").iter().enumerate() {
            line = line.replace(&session.0, &format!("<session-{}>", index + 1));
        }
        let uuid = regex::Regex::new("[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").expect("pattern");
        line = uuid.replace_all(&line, "<uuid>").into_owned();
        let epoch = regex::Regex::new("[0-9]{13}").expect("pattern");
        line = epoch.replace_all(&line, "<epoch-ms>").into_owned();
        for word in [brand::NAME, brand::COMMAND] {
            line = line.replace(word, "<brand>");
        }
        line
    }

    pub fn assert(&self, expected: &str) {
        let actual = self.transcript.lock().expect("transcript").clone();
        let expected = render::dedented(expected);
        if actual == expected {
            return;
        }
        let differs =
            actual.iter().zip(&expected).position(|(a, e)| a != e).unwrap_or(actual.len().min(expected.len()));
        let pasted: String = actual.iter().map(|line| format!("        {line}\n")).collect();
        if let Some(dir) = std::env::var_os("GOLDEN_ACTUAL_DIR") {
            let name = std::thread::current().name().unwrap_or("unnamed").replace("::", ".");
            std::fs::write(Path::new(&dir).join(format!("{name}.txt")), &pasted).expect("actual transcript");
        }
        panic!(
            "the transcript changed at line {}:\n  expected: {}\n  actual:   {}\n\nfull actual transcript:\n{pasted}",
            differs + 1,
            expected.get(differs).map_or("(end)", String::as_str),
            actual.get(differs).map_or("(end)", String::as_str),
        );
    }
}
