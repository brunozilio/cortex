use agent_engine::model::scripted::ScriptedModel;
use agent_engine::team::Team;
use agent_engine::transcript::{Block, Message};
use domain::PermissionMode;
use domain::team::{LEAD, Member, Protocol};
use serde_json::json;

use crate::{Rig, tool_use};

fn only_shipped_content() {
    static EMPTY: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    let folder = EMPTY.get_or_init(|| tempfile::tempdir().expect("content folder"));
    let _ = content::use_folder(folder.path().to_path_buf());
}

fn lead_with_a_teammate() -> (Rig, tempfile::TempDir, Team) {
    only_shipped_content();
    let home = tempfile::tempdir().expect("home");
    let mut rig = Rig::new(PermissionMode::AcceptEdits);
    rig.workspace.home = Some(home.path().to_path_buf());
    rig.workspace.session_id = "lead-session-1".into();
    let team = Team::of_session(home.path(), "lead-session-1");
    team.open("lead-session-1").expect("open");
    team.join("tester", Member { agent_type: "general-purpose".into(), ..Member::default() }).expect("join");
    (rig, home, team)
}

fn results(history: &[Message]) -> Vec<String> {
    history
        .iter()
        .flat_map(|message| message.blocks.iter())
        .filter_map(|block| match block {
            Block::ToolResult { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_lead_messages_a_teammate_by_name_and_asks_it_to_shut_down() {
    let (rig, _home, team) = lead_with_a_teammate();
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use(
            "m1",
            "SendMessage",
            json!({ "to": "tester", "summary": "next task", "message": "Now fix the lint." }),
        )]),
        ScriptedModel::reply(vec![tool_use(
            "m2",
            "SendMessage",
            json!({ "to": "tester", "message": { "type": "shutdown_request", "reason": "done" } }),
        )]),
        ScriptedModel::reply(vec![tool_use("m3", "SendMessage", json!({ "to": "ghost", "message": "hello" }))]),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]);
    let mut history = vec![Message::user("coordinate")];

    rig.run_with(&model, &mut history, vec![Box::new(agent_engine::tools::send_message::SendMessageTool)])
        .expect("run");

    let results = results(&history);
    assert_eq!(results[0], "Sent to tester.");
    assert_eq!(results[1], "Sent to tester.");
    assert!(!results[2].starts_with("Sent"), "an unknown name is not a teammate: {}", results[2]);
    let mail = team.unread("tester");
    assert_eq!(mail.len(), 2);
    assert_eq!((mail[0].from.as_str(), mail[0].text.as_str()), (LEAD, "Now fix the lint."));
    assert_eq!(mail[0].summary.as_deref(), Some("next task"));
    assert!(matches!(
        Protocol::of(&mail[1].text),
        Some(Protocol::ShutdownRequest { from, reason: Some(reason), .. }) if from == LEAD && reason == "done"
    ));
}

#[test]
fn a_named_agent_is_a_teammate_and_a_bad_name_starts_nothing() {
    let (rig, _home, team) = lead_with_a_teammate();
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use(
            "t",
            "Agent",
            json!({ "description": "helper", "prompt": "help", "name": "no way" }),
        )]),
        ScriptedModel::reply(vec![tool_use("u", "Agent", json!({ "description": "helper", "name": "helper" }))]),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]);
    let mut history = vec![Message::user("go")];
    let agent_tool = agent_engine::tools::subagent::AgentTool { agents: Vec::new() };

    rig.run_with(&model, &mut history, vec![Box::new(agent_tool)]).expect("run");

    let results = results(&history);
    assert!(results[0].contains("is not a teammate name"), "{}", results[0]);
    assert!(results[1].contains("prompt"), "a teammate needs its task: {}", results[1]);
    assert_eq!(team.read().expect("team").names(), ["tester"], "nobody joined");
}

#[cfg(unix)]
#[test]
fn a_teammate_idle_hook_can_keep_a_teammate_working_and_never_runs_for_the_lead() {
    let (mut rig, home, team) = lead_with_a_teammate();
    let marker = home.path().join("asked-once");
    let command =
        format!("[ -e '{0}' ] && exit 0; touch '{0}'; printf 'check the lint too' >&2; exit 2", marker.display());
    let groups = json!({ "TeammateIdle": [{"hooks": [{"type": "command", "command": command}]}] });
    rig.workspace.hooks = agent_engine::hooks::Hooks::parse(&groups).expect("hooks");
    let lead = ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Led.".into())])]);
    rig.run(&lead, &mut vec![Message::user("lead")]).expect("lead run");
    assert!(!marker.exists(), "the lead is no teammate");

    rig.workspace.teammate = Some(agent_engine::team::Me { team, name: "tester".into() });
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
        ScriptedModel::reply(vec![Block::Text("Lint is clean too.".into())]),
    ]);
    rig.run(&model, &mut vec![Message::user("fix the tests")]).expect("teammate run");

    let asked = model.asked();
    assert_eq!(asked.len(), 2, "the hook sent it back once");
    assert!(format!("{:?}", asked[1].messages).contains("check the lint too"));
}
