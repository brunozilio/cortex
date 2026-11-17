use std::path::Path;
use std::sync::Arc;

use agent_engine::model::scripted::ScriptedModel;
use agent_engine::tools::script_tool::ScriptAccess;
use agent_engine::tools::skill::{FoundSkill, SkillTool, discover, expand_command, expand_initial};
use agent_engine::tools::{Tool, Workspace};
use agent_engine::transcript::{Block, Message};
use domain::{AskKind, Decision, EngineEvent, ModelOption, PermissionMode, PromptRequest};
use serde_json::json;

use crate::{Rig, tool_use};

const SKILL: &str = r#"
export const description = "Marks the folder.";
export const capabilities = ["writeFile"];
export default (args, ctx) => { ctx.writeFile("mark.txt", "marked"); return "MARKED"; };
"#;

const UNDECLARED: &str = r#"
export const description = "Marks the folder.";
export default (args, ctx) => { ctx.writeFile("mark.txt", "marked"); return "MARKED"; };
"#;

fn attended(rig: &mut Rig) {
    let events = Arc::clone(&rig.events);
    let emit: agent_engine::tools::host_judge::Emit = Arc::new(move |event| events.lock().expect("events").push(event));
    let judge = agent_engine::tools::host_judge::HostJudge::attended(Arc::clone(&rig.control), emit);
    rig.workspace.services.judge = Some(Arc::new(judge));
}

fn writes_allowed() -> domain::permission_rules::PermissionRules {
    domain::permission_rules::PermissionRules::default().with(domain::permission_rules::RuleKind::Allow, "Write")
}

fn skill(root: &Path, text: &str) {
    let folder = root.join(brand::DIR).join("skills").join("mark");
    std::fs::create_dir_all(&folder).expect("skill");
    std::fs::write(folder.join("skill.js"), text).expect("skill");
}

fn found(project: &Path, home: &Path) -> Vec<FoundSkill> {
    let mut request = PromptRequest::plain(project.to_path_buf(), "hi", ModelOption::new("m", "M"));
    request.host.extensions = Some(domain::Extensions { trust: domain::trust::Trust::Trusted, ..Default::default() });
    discover(request.folder(), Some(home))
}

fn asks(rig: &Rig) -> Vec<String> {
    rig.events()
        .into_iter()
        .filter_map(|event| match event {
            EngineEvent::Asked(ask) => match ask.kind {
                AskKind::Permission { subject, .. } => Some(subject),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn deny_the_first_ask(rig: &Rig) -> std::thread::JoinHandle<()> {
    let (control, events) = (Arc::clone(&rig.control), Arc::clone(&rig.events));
    std::thread::spawn(move || {
        for _ in 0..400 {
            let ask = events.lock().expect("events").iter().find_map(|event| match event {
                EngineEvent::Asked(ask) => Some(ask.id.clone()),
                _ => None,
            });
            if let Some(ask) = ask {
                control.answer(&ask, Decision::Deny);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    })
}

fn load_mark(rig: &Rig, skills: Vec<FoundSkill>) {
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("s", "Skill", json!({ "skill": "mark" }))]),
        ScriptedModel::reply(vec![Block::Text("Ok.".into())]),
    ]);
    let tool: Box<dyn Tool> = Box::new(SkillTool { skills });
    rig.run_with(&model, &mut vec![Message::user("mark")], vec![tool]).expect("run");
}

#[test]
fn a_project_s_script_skill_asks_before_it_runs_even_when_bypassing() {
    let rig = Rig::new(PermissionMode::Bypass);
    let home = tempfile::tempdir().expect("home");
    skill(rig.dir.path(), SKILL);
    let denier = deny_the_first_ask(&rig);
    load_mark(&rig, found(rig.dir.path(), home.path()));
    denier.join().expect("denier");

    let asks = asks(&rig);
    assert_eq!(asks.len(), 1, "{asks:?}");
    assert!(asks[0].contains("skill.js"), "the ask names the project's script: {asks:?}");
    assert!(!rig.dir.path().join("mark.txt").exists(), "a refused script never runs");
}

#[test]
fn the_owner_s_script_skill_runs_without_asking_and_its_write_is_judged_by_the_mode() {
    let mut rig = Rig::new(PermissionMode::AcceptEdits);
    attended(&mut rig);
    let home = tempfile::tempdir().expect("home");
    skill(home.path(), SKILL);
    load_mark(&rig, found(rig.dir.path(), home.path()));

    assert!(asks(&rig).is_empty());
    assert_eq!(std::fs::read_to_string(rig.dir.path().join("mark.txt")).expect("mark"), "marked");
}

#[test]
fn a_script_skill_s_write_asks_the_user_where_the_mode_asks_and_a_refusal_writes_nothing() {
    let mut rig = Rig::new(PermissionMode::Ask);
    attended(&mut rig);
    let home = tempfile::tempdir().expect("home");
    skill(home.path(), SKILL);
    let denier = deny_the_first_ask(&rig);
    load_mark(&rig, found(rig.dir.path(), home.path()));
    denier.join().expect("denier");

    let asks = asks(&rig);
    assert_eq!(asks.len(), 1, "{asks:?}");
    assert!(asks[0].contains("mark.txt") && asks[0].contains("ctx.writeFile in the skill script"), "{asks:?}");
    assert!(!rig.dir.path().join("mark.txt").exists());
}

#[test]
fn a_script_skill_that_did_not_declare_a_write_cannot_write() {
    let project = tempfile::tempdir().expect("project");
    let home = tempfile::tempdir().expect("home");
    skill(home.path(), UNDECLARED);
    let found = found(project.path(), home.path());
    let mut workspace = Workspace::new(project.path());
    workspace.policy.permissions = writes_allowed();
    let typed = expand_command("/mark", &found, project.path(), &workspace).expect("a skill");
    assert!(
        typed.as_ref().is_err_and(
            |why| why.contains("did not declare ctx.writeFile") && why.contains("export const capabilities")
        ),
        "{typed:?}"
    );
    assert!(!project.path().join("mark.txt").exists());
}

#[test]
fn a_project_s_script_never_takes_the_place_of_the_owner_s_skill_of_that_name() {
    let project = tempfile::tempdir().expect("project");
    let home = tempfile::tempdir().expect("home");
    skill(project.path(), SKILL);
    skill(home.path(), r#"export const description = "The owner's."; export default () => "OWNER";"#);

    let found = found(project.path(), home.path());
    let marks: Vec<&FoundSkill> = found.iter().filter(|skill| skill.name == "mark").collect();
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].description, "The owner's.");
    assert!(marks[0].file.starts_with(home.path()) && marks[0].trusted);
}

#[test]
fn only_a_command_the_user_typed_gives_a_project_s_script_its_access() {
    let project = tempfile::tempdir().expect("project");
    let home = tempfile::tempdir().expect("home");
    skill(project.path(), SKILL);
    let found = found(project.path(), home.path());
    let mark = project.path().join("mark.txt");

    let mut workspace = Workspace::new(project.path());
    workspace.policy.permissions = writes_allowed();
    let everything = ScriptAccess { files: true, processes: true, network: true };
    workspace.policy.scripts.project = everything;
    let initial = expand_initial("/mark", &found, project.path(), &workspace);
    assert!(initial.is_err() && !mark.exists(), "an agent's initial prompt is not the user typing: {initial:?}");

    workspace.policy.scripts.project = ScriptAccess { files: false, ..everything };
    let typed = expand_command("/mark", &found, project.path(), &workspace).expect("a skill");
    assert!(typed.is_err() && !mark.exists(), "the settings' scripts.files holds for a typed command: {typed:?}");

    workspace.policy.scripts.project = everything;
    let typed = expand_command("/mark", &found, project.path(), &workspace).expect("a skill");
    assert_eq!(typed.as_deref(), Ok("MARKED"));
    assert!(mark.exists());
}

#[test]
fn a_project_s_script_skill_gets_no_files_until_the_owner_s_settings_grant_them() {
    let project = tempfile::tempdir().expect("project");
    let home = tempfile::tempdir().expect("home");
    skill(project.path(), SKILL);
    let found = found(project.path(), home.path());
    let workspace = Workspace::new(project.path());
    let typed = expand_command("/mark", &found, project.path(), &workspace).expect("a skill");
    assert!(typed.as_ref().is_err_and(|why| why.contains("scripts.files")), "{typed:?}");
    assert!(!project.path().join("mark.txt").exists());
}

#[test]
fn a_project_s_script_skill_the_user_allowed_still_gets_no_files_by_default() {
    let rig = Rig::new(PermissionMode::Ask);
    let home = tempfile::tempdir().expect("home");
    skill(rig.dir.path(), SKILL);
    let control = Arc::clone(&rig.control);
    let events = Arc::clone(&rig.events);
    let allower = std::thread::spawn(move || {
        for _ in 0..400 {
            let ask = events.lock().expect("events").iter().find_map(|event| match event {
                EngineEvent::Asked(ask) => Some(ask.id.clone()),
                _ => None,
            });
            if let Some(id) = ask {
                control.answer(&id, Decision::Allow);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    });
    load_mark(&rig, found(rig.dir.path(), home.path()));
    allower.join().expect("allower");
    assert_eq!(asks(&rig).len(), 1);
    assert!(!rig.dir.path().join("mark.txt").exists(), "allowed to run, not to write");
}
