use std::path::Path;
use std::sync::Arc;

use agent_engine::model::scripted::ScriptedModel;
use agent_engine::tools::Tool;
use agent_engine::tools::script_tool::ScriptAccess;
use agent_engine::tools::skill::{FoundSkill, SkillTool, discover, expand_command, expand_initial};
use agent_engine::transcript::{Block, Message};
use domain::{AskKind, Decision, EngineEvent, ModelOption, PermissionMode, PromptRequest};
use serde_json::json;

use crate::{Rig, tool_use};

const SKILL: &str = r#"
export const description = "Marks the folder.";
export default (args, ctx) => { ctx.writeFile("mark.txt", "marked"); return "MARKED"; };
"#;

fn skill(root: &Path, text: &str) {
    let folder = root.join(brand::DIR).join("skills").join("mark");
    std::fs::create_dir_all(&folder).expect("skill");
    std::fs::write(folder.join("skill.js"), text).expect("skill");
}

fn found(project: &Path, home: &Path) -> Vec<FoundSkill> {
    let request = PromptRequest::plain(project.to_path_buf(), "hi", ModelOption::new("m", "M"));
    discover(&request, Some(home))
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
fn the_owner_s_script_skill_runs_without_asking() {
    let rig = Rig::new(PermissionMode::Ask);
    let home = tempfile::tempdir().expect("home");
    skill(home.path(), SKILL);
    load_mark(&rig, found(rig.dir.path(), home.path()));

    assert!(asks(&rig).is_empty());
    assert_eq!(std::fs::read_to_string(rig.dir.path().join("mark.txt")).expect("mark"), "marked");
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

    let initial = expand_initial("/mark", &found, project.path(), ScriptAccess::default());
    assert!(initial.is_err() && !mark.exists(), "an agent's initial prompt is not the user typing: {initial:?}");

    let off = ScriptAccess { files: false, ..ScriptAccess::default() };
    let typed = expand_command("/mark", &found, project.path(), off).expect("a skill");
    assert!(typed.is_err() && !mark.exists(), "the settings' scripts.files holds for a typed command: {typed:?}");

    let typed = expand_command("/mark", &found, project.path(), ScriptAccess::default()).expect("a skill");
    assert_eq!(typed.as_deref(), Ok("MARKED"));
    assert!(mark.exists());
}
