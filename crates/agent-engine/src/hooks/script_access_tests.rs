use std::process::Command;
use std::sync::{Arc, Mutex};

use platform::sandbox::Confinement;
use serde_json::json;

use super::super::Hooks;
use crate::tools::{Launch, Launcher, Workspace};

const REACHING: &str = r#"export const event = "PreToolUse";
export const capabilities = ["readFile", "exec", "fetch"];
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
    let decision = hooks.run(crate::hooks::HookEvent::PreToolUse, "Bash", input, workspace).expect("run");
    serde_json::from_str(&decision.blocked.expect("reason")).expect("json")
}

fn echo_allowed(workspace: &mut Workspace) {
    workspace.policy.permissions = domain::permission_rules::PermissionRules::default()
        .with(domain::permission_rules::RuleKind::Allow, "Bash(echo ran)");
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
    echo_allowed(&mut workspace);
    let tried = reached(&hooks, &workspace);
    assert_eq!(tried["files"], "alpha");
    assert_eq!(tried["processes"], "ran");
    assert!(tried["network"].as_str().is_some_and(|said| said.contains("nobody can be asked")), "{tried}");
}

fn hook_in(home: &std::path::Path, source: &str) -> Hooks {
    let folder = home.join(brand::DIR).join("hooks");
    std::fs::create_dir_all(&folder).expect("hooks");
    std::fs::write(folder.join("act.js"), source).expect("hook");
    Hooks::from_scripts(&folder)
}

const ACTING: &str = r#"export const event = "PreToolUse";
export const capabilities = ["exec"];
export default function (input, ctx) {
  let said;
  try { said = String(ctx.exec("touch made.txt").code); } catch (error) { said = `refused: ${error.message ?? error}`; }
  let read;
  try { read = ctx.readFile("notes.txt"); } catch (error) { read = `refused: ${error.message ?? error}`; }
  return { decision: "block", reason: JSON.stringify({ said, read }) };
}"#;

#[test]
fn a_hook_s_undeclared_call_fails_and_says_what_to_declare() {
    let home = tempfile::tempdir().expect("home");
    let project = project_with_notes();
    let hooks = hook_in(home.path(), ACTING);
    let mut workspace = Workspace::new(project.path());
    workspace.home = Some(home.path().to_path_buf());
    let tried = reached(&hooks, &workspace);
    let read = tried["read"].as_str().unwrap_or_default();
    assert!(read.contains("did not declare ctx.readFile") && read.contains("export const capabilities"), "{read}");
}

#[test]
fn a_hook_s_declared_call_is_judged_by_the_policy_and_with_nobody_to_ask_a_rule_must_allow_it() {
    let home = tempfile::tempdir().expect("home");
    let project = project_with_notes();
    let hooks = hook_in(home.path(), ACTING);
    let mut workspace = Workspace::new(project.path());
    workspace.home = Some(home.path().to_path_buf());
    let tried = reached(&hooks, &workspace);
    let said = tried["said"].as_str().unwrap_or_default();
    assert!(said.contains("nobody can be asked") && said.contains("Bash(touch:*)"), "{said}");
    assert!(!project.path().join("made.txt").exists());

    workspace.policy.permissions = domain::permission_rules::PermissionRules::default()
        .with(domain::permission_rules::RuleKind::Allow, "Bash(touch made.txt)");
    assert_eq!(reached(&hooks, &workspace)["said"], "0");
    assert!(project.path().join("made.txt").exists());

    workspace.policy.permissions = domain::permission_rules::PermissionRules::default()
        .with(domain::permission_rules::RuleKind::Deny, "Bash(touch:*)");
    let denied = reached(&hooks, &workspace)["said"].as_str().unwrap_or_default().to_owned();
    assert!(denied.contains("Bash(touch:*)"), "{denied}");
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
    echo_allowed(&mut workspace);
    let tried = reached(&hooks, &workspace);
    assert_eq!(tried["processes"], "ran");
    let launched = seen.0.lock().expect("seen").clone();
    assert!(launched.iter().any(|line| line.contains("echo ran")), "{launched:?}");
}
