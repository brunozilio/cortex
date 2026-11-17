use std::process::Command;
use std::sync::{Arc, Mutex};

use platform::sandbox::Confinement;
use serde_json::json;

use super::super::Hooks;
use crate::tools::{Launch, Launcher, Workspace};

const REACHING: &str = r#"export const event = "PreToolUse";
export default function (input, ctx) {
  const tried = {};
  for (const [name, attempt] of Object.entries({
    files: () => ctx.readFile("notes.txt"),
    processes: () => ctx.exec("echo ran").stdout.trim(),
    network: () => ctx.fetch("ftp://nowhere.invalid/"),
  })) {
    try { tried[name] = String(attempt()); } catch (error) { tried[name] = `refused: ${error.message ?? error}`; }
  }
  return { decision: "block", reason: JSON.stringify(tried) };
}"#;

fn placed(root: &std::path::Path) -> Hooks {
    let folder = root.join(brand::DIR).join("hooks");
    std::fs::create_dir_all(&folder).expect("hooks");
    std::fs::write(folder.join("reach.js"), REACHING).expect("hook");
    Hooks::from_scripts(&folder)
}

fn reached(hooks: &Hooks, workspace: &Workspace) -> serde_json::Value {
    let input = json!({ "tool_name": "Bash", "tool_input": { "command": "ls" } });
    let decision = hooks.run("PreToolUse", "Bash", input, workspace).expect("run");
    serde_json::from_str(&decision.blocked.expect("reason")).expect("json")
}

fn project_with_notes() -> tempfile::TempDir {
    let project = tempfile::tempdir().expect("project");
    std::fs::write(project.path().join("notes.txt"), "alpha").expect("notes");
    project
}

#[test]
fn a_project_s_hook_script_gets_no_files_processes_or_network_by_default() {
    let home = tempfile::tempdir().expect("home");
    let project = project_with_notes();
    let hooks = placed(project.path());
    let mut workspace = Workspace::new(project.path());
    workspace.home = Some(home.path().to_path_buf());
    let tried = reached(&hooks, &workspace);
    for (what, setting) in
        [("files", "scripts.files"), ("processes", "scripts.processes"), ("network", "scripts.network")]
    {
        let said = tried[what].as_str().unwrap_or_default();
        assert!(said.starts_with("refused") && said.contains(setting), "{what}: {said}");
    }
}

#[test]
fn the_user_s_own_hook_script_keeps_files_processes_and_network() {
    let home = tempfile::tempdir().expect("home");
    let project = project_with_notes();
    let hooks = placed(home.path());
    let mut workspace = Workspace::new(project.path());
    workspace.home = Some(home.path().to_path_buf());
    let tried = reached(&hooks, &workspace);
    assert_eq!(tried["files"], "alpha");
    assert_eq!(tried["processes"], "ran");
    assert!(tried["network"].as_str().is_some_and(|said| said.contains("not an http(s) address")), "{tried}");
}

struct Recording(Mutex<Vec<String>>);

impl Launcher for Recording {
    fn launch(&self, command: Command, _confinement: Option<&Confinement>) -> Command {
        let line = command.get_args().map(|arg| arg.to_string_lossy().into_owned()).collect::<Vec<_>>().join(" ");
        self.0.lock().expect("seen").push(line);
        command
    }
}

#[test]
fn a_command_a_script_starts_goes_through_the_same_confinement_as_a_command_hook() {
    let home = tempfile::tempdir().expect("home");
    let project = project_with_notes();
    let hooks = placed(home.path());
    let seen = Arc::new(Recording(Mutex::new(Vec::new())));
    let mut workspace = Workspace::new(project.path());
    workspace.home = Some(home.path().to_path_buf());
    workspace.services.launch = Launch(seen.clone());
    let tried = reached(&hooks, &workspace);
    assert_eq!(tried["processes"], "ran");
    let launched = seen.0.lock().expect("seen").clone();
    assert!(launched.iter().any(|line| line.contains("echo ran")), "{launched:?}");
}
