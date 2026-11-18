use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_engine::Engine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;
use domain::{EngineEvent, PromptRequest, SessionId};
use serde_json::{Value, json};

use crate::{Source, finish, model, start};

struct Project {
    home: tempfile::TempDir,
    repo: tempfile::TempDir,
    models: Vec<Arc<ScriptedModel>>,
}

impl Project {
    fn new(runs: usize) -> Self {
        static INSTALLED: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
        let installed = INSTALLED.get_or_init(|| tempfile::tempdir().expect("installed content"));
        let _ = content::use_folder(installed.path().to_path_buf());
        let models = (0..runs)
            .map(|_| Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Done.".into())])])))
            .collect();
        Self { home: tempfile::tempdir().expect("home"), repo: tempfile::tempdir().expect("repo"), models }
    }

    fn engine(&self) -> Engine {
        Engine::new(Arc::new(Source(Mutex::new(self.models.clone()))), Arc::new(InMemorySessions::default()))
    }

    fn journal(&self) -> PathBuf {
        self.home.path().join("journal")
    }

    fn journaling(&self, events: &[&str]) {
        let journal = self.journal().display().to_string();
        let hooks: serde_json::Map<String, Value> = events
            .iter()
            .map(|event| {
                let line = format!("echo {event} >> '{journal}'; cat > /dev/null");
                ((*event).to_owned(), json!([{"hooks": [{"type": "command", "command": line}]}]))
            })
            .collect();
        self.owner_file("settings.json", &json!({ "hooks": hooks }).to_string());
    }

    fn owner_file(&self, name: &str, text: &str) {
        let folder = self.home.path().join(brand::DIR);
        std::fs::create_dir_all(&folder).expect("owner's folder");
        std::fs::write(folder.join(name), text).expect("owner's file");
    }

    fn project_file(&self, name: &str, text: &str) {
        let path = self.repo.path().join(name);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
        std::fs::write(path, text).expect("project file");
    }

    fn heard(&self) -> Vec<String> {
        std::fs::read_to_string(self.journal()).unwrap_or_default().lines().map(str::to_owned).collect()
    }

    fn request(&self, prompt: &str, resume: Option<&SessionId>) -> PromptRequest {
        let mut request = PromptRequest::plain(self.repo.path().into(), prompt, model());
        request.host.extensions = Some(domain::Extensions {
            home: Some(self.home.path().into()),
            trust: domain::trust::Trust::Trusted,
            ..Default::default()
        });
        request.session.resume = resume.cloned();
        request
    }
}

fn run(engine: &Engine, request: &PromptRequest) -> SessionId {
    let events = finish(&start(engine, request));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    match events.first() {
        Some(EngineEvent::SessionStarted(id)) => id.clone(),
        _ => panic!("{events:?}"),
    }
}

fn eventually(what: impl Fn() -> bool) -> bool {
    let until = Instant::now() + Duration::from_secs(10);
    while Instant::now() < until {
        if what() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn two_runs_of_an_unchanged_project_load_the_brief_once() {
    let project = Project::new(3);
    project.journaling(&["Brief"]);
    let engine = project.engine();

    let session = run(&engine, &project.request("one", None));
    run(&engine, &project.request("two", Some(&session)));
    assert_eq!(project.heard(), ["Brief"], "the second run reused the session's brief");

    run(&engine, &project.request("three", None));
    assert_eq!(project.heard(), ["Brief", "Brief"], "another session loads its own");
}

#[test]
fn an_instruction_file_edited_between_runs_reaches_the_next_run() {
    let project = Project::new(3);
    project.project_file(brand::INSTRUCTIONS, "Indent with tabs.");
    let engine = project.engine();

    let session = run(&engine, &project.request("one", None));
    project.project_file(brand::INSTRUCTIONS, "Indent with four spaces.");
    run(&engine, &project.request("two", Some(&session)));
    run(&engine, &project.request("three", Some(&session)));

    let systems: Vec<String> = project.models.iter().map(|model| model.asked()[0].system.clone()).collect();
    assert!(systems[0].contains("Indent with tabs."), "{}", systems[0]);
    assert!(systems[1].contains("Indent with four spaces.") && !systems[1].contains("tabs"), "{}", systems[1]);
    assert!(systems[2].contains("Indent with four spaces."));
}

#[test]
fn session_start_fires_once_run_start_and_run_end_every_run_and_session_end_when_the_engine_goes() {
    let project = Project::new(2);
    project.journaling(&["SessionStart", "RunStart", "RunEnd", "SessionEnd"]);
    let engine = project.engine();

    let session = run(&engine, &project.request("one", None));
    run(&engine, &project.request("two", Some(&session)));
    assert_eq!(project.heard(), ["SessionStart", "RunStart", "RunEnd", "RunStart", "RunEnd"]);

    drop(engine);
    assert_eq!(project.heard(), ["SessionStart", "RunStart", "RunEnd", "RunStart", "RunEnd", "SessionEnd"]);
}

#[test]
fn a_session_pushed_out_by_newer_ones_ends() {
    let project = Project::new(9);
    project.journaling(&["SessionEnd"]);
    let engine = project.engine();

    let sessions: Vec<SessionId> = (0..9).map(|at| run(&engine, &project.request(&format!("p{at}"), None))).collect();

    assert!(eventually(|| project.heard().len() == 1), "the oldest of nine sessions ended: {:?}", project.heard());
    assert_eq!(sessions.len(), 9);
}

const SERVER: &str = r#"echo $$ >> "$1"
while IFS= read -r line; do
id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
case "$line" in
*'"method":"initialize"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{}}\n' "$id";;
*'"method":"tools/list"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"tools":[{"name":"ping","inputSchema":{"type":"object"}}]}}\n' "$id";;
esac
done"#;

fn serving(project: &Project) -> PathBuf {
    let started = project.home.path().join("started");
    project.project_file("server.sh", SERVER);
    let args = [project.repo.path().join("server.sh").display().to_string(), started.display().to_string()];
    let config = json!({"mcpServers": {"jira": {"command": "/bin/sh", "args": args}}});
    project.project_file(&format!("{}/mcp.json", brand::DIR), &config.to_string());
    project.project_file(&format!("{}/settings.local.json", brand::DIR), r#"{"enabledMcpServers": ["jira"]}"#);
    started
}

fn pids(started: &Path) -> Vec<String> {
    std::fs::read_to_string(started).unwrap_or_default().lines().map(str::to_owned).collect()
}

fn running(pid: &str) -> bool {
    let state = std::process::Command::new("ps").args(["-o", "stat=", "-p", pid]).output();
    state.is_ok_and(|done| {
        let stat = String::from_utf8_lossy(&done.stdout).trim().to_owned();
        done.status.success() && !stat.is_empty() && !stat.starts_with('Z')
    })
}

fn offered(model: &ScriptedModel) -> bool {
    agent_engine::tools::repl::within_reach(&model.asked()[0].tools).iter().any(|tool| tool == "mcp__jira__ping")
}

#[test]
#[cfg(unix)]
fn a_session_keeps_its_mcp_server_across_runs_and_closes_it_when_approval_is_revoked() {
    let project = Project::new(3);
    let started = serving(&project);
    let engine = project.engine();

    let session = run(&engine, &project.request("one", None));
    run(&engine, &project.request("two", Some(&session)));
    assert!(offered(&project.models[0]) && offered(&project.models[1]));
    let pid = pids(&started);
    assert_eq!(pid.len(), 1, "started once for both runs");
    assert!(running(&pid[0]));

    project.project_file(&format!("{}/settings.local.json", brand::DIR), r#"{"enabledMcpServers": []}"#);
    run(&engine, &project.request("three", Some(&session)));

    assert!(!offered(&project.models[2]), "revoked between runs");
    assert!(!running(&pid[0]), "the revoked server was closed");
}

#[test]
#[cfg(unix)]
fn a_session_s_mcp_server_ends_with_the_engine_and_starts_again_if_it_died() {
    let project = Project::new(2);
    let started = serving(&project);
    let engine = project.engine();

    let session = run(&engine, &project.request("one", None));
    let first = pids(&started)[0].clone();
    std::process::Command::new("kill").args(["-9", &first]).output().expect("kill");
    assert!(eventually(|| !running(&first)));
    run(&engine, &project.request("two", Some(&session)));
    assert!(offered(&project.models[1]), "a dead server is started again");
    let second = pids(&started)[1].clone();
    assert!(running(&second));

    drop(engine);
    assert!(!running(&second), "the session's servers end with it");
}
