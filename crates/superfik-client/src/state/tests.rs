use std::path::PathBuf;

use superfik_core::{
    Activity, AgentStatus, EngineEvent, ResearchItem, ResearchKind, RunId, Skill, SkillIcon, WorkspaceSnapshot,
};
use superfik_protocol::{ClientCommand, ServerEvent};

use super::*;

fn skill(name: &str) -> Skill {
    Skill { name: name.into(), title: format!("{name} skill"), description: String::new(), icon: SkillIcon::Cube }
}

fn session() -> Session {
    let mut session = Session::new(Some(PathBuf::from("/repo")));
    session.available_skills = vec![skill("dither-minis"), skill("motion")];
    session
}

fn type_text(session: &mut Session, text: &str) {
    session.apply(Action::Edit(EditKey::Insert(text.into())), 0.0);
}

fn submitted_run(session: &mut Session) -> RunId {
    type_text(session, "Create a small night sky scene.");
    let effects = session.apply(Action::Submit, 1.0);
    match commands(&effects).as_slice() {
        [ClientCommand::SubmitPrompt { run, .. }] => *run,
        other => panic!("unexpected effects {other:?}"),
    }
}

pub(super) fn commands(effects: &[Effect]) -> Vec<&ClientCommand> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Command(command) => Some(command.as_ref()),
            _ => None,
        })
        .collect()
}

fn run_event(run: RunId, event: EngineEvent) -> Action {
    Action::Server(ServerEvent::Run { run, event })
}

fn silent(effects: Vec<Effect>) -> Vec<Effect> {
    effects.into_iter().filter(|e| !matches!(e, Effect::Sound(_))).collect()
}

#[test]
fn slash_tokens_select_skills_and_open_picker() {
    let mut s = session();
    type_text(&mut s, "/mo");
    assert_eq!(s.popover, Some(PopoverKind::Skills));
    let skills = s.picks().iter().filter(|p| matches!(p, Pick::Skill(_))).count();
    assert_eq!(skills, 1, "commands such as /model are listed after it");
    s.apply(Action::CompleteSkill("motion".into()), 0.0);
    assert_eq!(s.conversation().composer.text(), "/motion ");
    assert_eq!(s.popover, None);
}

#[test]
fn every_skill_is_selected_until_taken_out() {
    let mut s = session();
    assert_eq!(s.selected_skills(), vec![skill("dither-minis"), skill("motion")]);
    assert!(s.addable_skills().is_empty());
    s.apply(Action::RemoveSkill("motion".into()), 0.0);
    assert_eq!(s.selected_skills(), vec![skill("dither-minis")]);
    assert_eq!(s.addable_skills(), vec![&skill("motion")]);
    s.apply(Action::OpenPopover(PopoverKind::AddSkill), 0.0);
    s.apply(Action::AddSkill("motion".into()), 0.0);
    assert_eq!(s.selected_skills(), vec![skill("dither-minis"), skill("motion")]);
    assert_eq!(s.popover, None);
}

#[test]
fn naming_a_removed_skill_brings_it_back() {
    let mut s = session();
    s.apply(Action::RemoveSkill("motion".into()), 0.0);
    type_text(&mut s, "/motion Make it move");
    assert!(s.selected_skills().contains(&skill("motion")));
    s.apply(Action::Submit, 0.0);
    assert!(s.selected_skills().contains(&skill("motion")), "stays in after the prompt");
}

#[test]
fn arrows_move_the_picker_highlight_and_typing_resets_it() {
    let mut s = session();
    type_text(&mut s, "/");
    assert_eq!(s.highlighted_pick().map(|p| p.label()).as_deref(), Some("/dither-minis"));
    s.apply(Action::MovePick(1), 0.0);
    s.apply(Action::MovePick(1), 0.0);
    assert_eq!(s.highlighted_pick().map(|p| p.label()).as_deref(), Some("/context"), "skills, then commands");
    s.apply(Action::MovePick(100), 0.0);
    let last = s.picks().len() - 1;
    assert_eq!(s.pick_cursor, last, "stops at the last row");
    s.apply(Action::MovePick(-100), 0.0);
    assert_eq!(s.pick_cursor, 0, "stops at the first row");
    s.apply(Action::MovePick(1), 0.0);
    type_text(&mut s, "m");
    assert_eq!(s.pick_cursor, 0);
}

#[test]
fn removing_a_skill_strips_its_token() {
    let mut s = session();
    type_text(&mut s, "/dither-minis /motion Create a sky");
    s.apply(Action::RemoveSkill("motion".into()), 0.0);
    assert_eq!(s.conversation().composer.text(), "/dither-minis Create a sky");
    assert_eq!(s.selected_skills(), vec![skill("dither-minis")]);
}

#[test]
fn submit_builds_request_and_turn() {
    let mut s = session();
    type_text(&mut s, "/motion Make it move");
    let effects = silent(s.apply(Action::Submit, 2.0));
    let [ClientCommand::SubmitPrompt { request, .. }] = commands(&effects).as_slice() else {
        panic!("expected one submit")
    };
    assert_eq!(request.skills, vec![skill("motion")], "named skills are invoked");
    assert!(request.disabled_skills.is_empty(), "the rest stay available");
    assert!(s.conversation().composer.text().is_empty());
    assert_eq!(s.conversation().turns.len(), 1);
    assert!(s.is_running());
}

#[test]
fn submit_without_workspace_asks_for_one() {
    let mut s = Session::default();
    type_text(&mut s, "hey");
    let effects = s.apply(Action::Submit, 0.0);
    assert!(effects.iter().any(|e| matches!(e, Effect::PickWorkspace)));
    assert!(commands(&effects).is_empty());
    assert!(s.notice.is_some());
    assert!(s.conversation().turns.is_empty());
}

#[test]
fn empty_submits_and_a_second_submit_in_a_running_conversation_are_ignored() {
    let mut s = session();
    assert!(commands(&s.apply(Action::Submit, 0.0)).is_empty());
    submitted_run(&mut s);
    type_text(&mut s, "again");
    assert!(commands(&s.apply(Action::Submit, 0.0)).is_empty());
    assert_eq!(s.conversation().turns.len(), 1);
}

#[test]
fn run_events_stream_into_the_turn_and_finish_agents() {
    let mut s = session();
    let run = submitted_run(&mut s);
    s.apply(run_event(run, EngineEvent::TextDelta("Hey!".into())), 1.5);
    let spawned = |index| EngineEvent::AgentSpawned {
        index,
        kind: "Explore".into(),
        summary: "Map it".into(),
        prompt: String::new(),
    };
    s.apply(run_event(run, spawned(0)), 1.55);
    s.apply(run_event(run, spawned(1)), 1.56);
    s.apply(
        run_event(
            run,
            EngineEvent::AgentUpdate { index: 0, status: AgentStatus::Running { progress: 0.5 }, summary: None },
        ),
        1.6,
    );
    let item = ResearchItem { title: "Moonlight".into(), url: None, kind: ResearchKind::Article };
    s.apply(run_event(run, EngineEvent::ResearchFound(item.clone())), 1.7);
    s.apply(run_event(run, EngineEvent::ResearchFound(item)), 1.8);
    s.apply(run_event(run, EngineEvent::Completed { duration_ms: 4000, cost_usd: None }), 5.0);

    let response = &s.conversation().turns[0].response;
    assert_eq!(response.text, "Hey!");
    assert_eq!(response.activity, Activity::Done);
    assert_eq!(s.conversation().research_items.len(), 1);
    assert_eq!(s.conversation().agents[0].status, AgentStatus::Done { seconds: 4 });
    assert_eq!(s.conversation().agents[1].status, AgentStatus::Done { seconds: 4 });
    assert!(!s.is_running());
}

#[test]
fn events_for_unknown_runs_are_ignored() {
    let mut s = session();
    submitted_run(&mut s);
    s.apply(run_event(RunId::new(), EngineEvent::TextDelta("stray".into())), 1.0);
    assert!(s.conversation().turns[0].response.text.is_empty());
}

#[test]
fn cancel_targets_active_run() {
    let mut s = session();
    assert!(commands(&s.apply(Action::Cancel, 0.0)).is_empty());
    let run = submitted_run(&mut s);
    let effects = s.apply(Action::Cancel, 2.0);
    assert!(matches!(commands(&effects).as_slice(), [ClientCommand::CancelRun { run: r }] if *r == run));
}

#[test]
fn model_change_records_previous_label_for_morph() {
    let mut s = session();
    s.apply(Action::OpenPopover(PopoverKind::Model), 0.0);
    s.apply(Action::SelectModel(1), 3.0);
    assert_eq!(s.model().label, "Fable 5.1");
    assert_eq!(s.model_changed, Some(("Opus 5.5".into(), 3.0)));
    assert_eq!(s.popover, None);
}

#[test]
fn another_provider_starts_the_engine_session_afresh() {
    use superfik_core::{EngineKind, ModelProvider, SessionId};
    let mut s = session();
    let local = |id: &str| ModelOption::new(EngineKind::Native, id, id).served_by(ModelProvider::Local);
    s.models.extend([local("qwen"), local("glm")]);
    let (qwen, glm) = (s.models.len() - 2, s.models.len() - 1);
    s.conversation_mut().engine_session = Some(SessionId("anthropic".into()));
    s.apply(Action::SelectModel(qwen), 1.0);
    assert_eq!(s.conversation().engine_session, None);
    s.conversation_mut().engine_session = Some(SessionId("local".into()));
    s.apply(Action::SelectModel(glm), 2.0);
    assert_eq!(s.conversation().engine_session, Some(SessionId("local".into())));
}

#[test]
fn spawned_agents_get_cards_that_the_next_run_replaces() {
    let mut s = session();
    let run = submitted_run(&mut s);
    let spawned = |index, kind: &str| EngineEvent::AgentSpawned {
        index,
        kind: kind.into(),
        summary: "Look".into(),
        prompt: String::new(),
    };
    s.apply(run_event(run, spawned(0, "Explore")), 1.0);
    let update = EngineEvent::AgentUpdate { index: 2, status: AgentStatus::Running { progress: 0.3 }, summary: None };
    s.apply(run_event(run, update), 1.1);
    let agents = &s.conversation().agents;
    assert_eq!(agents.iter().map(|a| a.index).collect::<Vec<_>>(), vec![0, 1, 2]);
    assert_eq!((agents[0].label(), agents[0].summary.as_str()), ("Explorer", "Look"));
    assert_eq!(agents[2].status, AgentStatus::Running { progress: 0.3 });
    s.apply(run_event(run, spawned(1, "general-purpose")), 1.2);
    assert_eq!(s.conversation().agents.len(), 3);
    assert_eq!(s.conversation().agents[1].label(), "general-purpose");
    s.apply(run_event(run, EngineEvent::Completed { duration_ms: 10, cost_usd: None }), 2.0);

    s.apply(Action::RemoveAgent(0), 3.0);
    assert_eq!(s.conversation().agents.iter().map(|a| a.index).collect::<Vec<_>>(), vec![0, 1]);
    submitted_run(&mut s);
    assert!(s.conversation().agents.is_empty(), "each run shows its own sub-agents");
}

#[test]
fn effort_is_set_directly() {
    let mut s = session();
    s.apply(Action::SetEffort(superfik_core::Effort::Max), 0.0);
    assert!(s.effort().is_ultra());
}

#[test]
fn any_file_becomes_a_reference_once() {
    let mut s = session();
    s.apply(Action::AddReferences(vec!["/a/sky.PNG".into(), "/a/notes.md".into(), "/a/sky.PNG".into()]), 0.0);
    assert_eq!(s.conversation().references, vec![PathBuf::from("/a/sky.PNG"), PathBuf::from("/a/notes.md")]);
}

#[test]
fn only_images_travel_with_the_message() {
    let mut s = session();
    s.apply(Action::AddReferences(vec!["/a/sky.png".into(), "/a/notes.md".into()]), 0.0);
    s.apply(Action::Edit(EditKey::Insert("go".into())), 0.0);
    s.apply(Action::Submit, 0.0);
    assert_eq!(s.conversation().turns[0].user.images, vec![PathBuf::from("/a/sky.png")]);
}

#[test]
fn references_expand_one_at_a_time_and_can_be_removed() {
    let mut s = session();
    s.apply(Action::AddReferences(vec!["/a.png".into(), "/b.png".into(), "/c.png".into()]), 0.0);
    s.apply(Action::ToggleReference(1), 0.0);
    assert_eq!(s.conversation().expanded_reference, Some(1));
    s.apply(Action::ToggleReference(1), 0.0);
    assert_eq!(s.conversation().expanded_reference, None);
    s.apply(Action::ToggleReference(2), 0.0);
    s.apply(Action::RemoveReference(0), 0.0);
    assert_eq!(s.conversation().references, vec![PathBuf::from("/b.png"), PathBuf::from("/c.png")]);
    assert_eq!(s.conversation().expanded_reference, Some(1), "follows the shifted item");
    s.apply(Action::RemoveReference(1), 0.0);
    assert_eq!(s.conversation().expanded_reference, None);
    s.apply(Action::ToggleReference(9), 0.0);
    assert_eq!(s.conversation().expanded_reference, None);
}

#[test]
fn opening_workspace_resets_context_and_inspects() {
    let mut s = session();
    s.conversation_mut().references.push("/x.png".into());
    let effects = s.apply(Action::OpenWorkspace("/other".into()), 0.0);
    assert!(s.conversation().references.is_empty());
    let names: Vec<&str> = commands(&effects).iter().map(|c| c.name()).collect();
    for expected in ["inspect_workspace", "list_skills", "list_agents"] {
        assert!(names.contains(&expected), "{names:?}");
    }
    assert!(effects.iter().any(|e| matches!(e, Effect::Persist)));
    s.apply(Action::Server(ServerEvent::Workspace(WorkspaceSnapshot::default())), 0.0);
    assert_eq!(s.root, Some(PathBuf::from("/other")));
}

#[test]
fn removing_the_workspace_promotes_an_attached_repository_or_empties_the_session() {
    let mut s = session();
    s.clone_root = PathBuf::from("/w");
    let engine = superfik_core::Repository { root: "/engine".into(), slug: "acme/engine".into(), ..Default::default() };
    let docs = superfik_core::Repository { root: "/docs".into(), slug: "acme/docs".into(), ..Default::default() };
    s.apply(Action::Server(ServerEvent::Repository(engine.clone())), 0.0);
    s.apply(Action::Server(ServerEvent::Repository(docs.clone())), 0.0);
    let effects = s.apply(Action::RemoveWorkspace, 0.0);
    assert!(
        matches!(commands(&effects)[..], [ClientCommand::InspectWorkspace { path }, ..] if path == &PathBuf::from("/engine"))
    );
    assert_eq!(s.root, Some(PathBuf::from("/engine")));
    assert_eq!(s.conversation().repositories, vec![docs]);
    assert_eq!(s.clone_root, PathBuf::from("/w"), "account-level state survives");
    s.apply(Action::RemoveWorkspace, 0.0);
    let effects = s.apply(Action::RemoveWorkspace, 0.0);
    assert_eq!(s.root, None);
    assert!(commands(&effects).is_empty() && effects.iter().any(|e| matches!(e, Effect::Persist)));
    assert!(s.apply(Action::RemoveWorkspace, 0.0).is_empty(), "nothing left to remove");
}

#[test]
fn removed_skills_are_disabled_for_the_agent() {
    let mut s = session();
    s.apply(Action::RemoveSkill("dither-minis".into()), 0.0);
    type_text(&mut s, "Make it move");
    let effects = silent(s.apply(Action::Submit, 0.0));
    let [ClientCommand::SubmitPrompt { request, .. }] = commands(&effects).as_slice() else {
        panic!("expected one submit")
    };
    assert!(request.skills.is_empty());
    assert_eq!(request.disabled_skills, vec!["dither-minis".to_owned()]);
    assert_eq!(s.selected_skills(), vec![skill("motion")], "stays out after the prompt");
}

#[test]
fn opening_workspaces_keeps_a_recent_list() {
    let mut s = session();
    s.apply(Action::RestoreRecents(vec!["/a".into(), "/b".into()]), 0.0);
    s.apply(Action::OpenWorkspace("/b".into()), 0.0);
    assert_eq!(s.recent_workspaces, vec![PathBuf::from("/b"), PathBuf::from("/a")]);
    for i in 0..20 {
        s.apply(Action::OpenWorkspace(format!("/r{i}").into()), 0.0);
    }
    assert_eq!(s.recent_workspaces.len(), 8);
    assert_eq!(s.recent_workspaces[0], PathBuf::from("/r19"));
}

#[test]
fn gathering_ends_when_text_or_agents_arrive() {
    let mut s = session();
    assert!(!s.is_gathering());
    let run = submitted_run(&mut s);
    assert!(s.is_gathering());
    s.apply(run_event(run, EngineEvent::TextDelta("Here".into())), 2.0);
    assert!(!s.is_gathering());

    let mut s = session();
    let run = submitted_run(&mut s);
    let spawned =
        EngineEvent::AgentSpawned { index: 0, kind: "Explore".into(), summary: String::new(), prompt: String::new() };
    s.apply(run_event(run, spawned), 1.0);
    assert!(!s.is_gathering());
}

#[test]
fn agent_sessions_fill_from_their_events_and_open_in_a_sheet() {
    let mut s = session();
    let run = submitted_run(&mut s);
    let spawned = EngineEvent::AgentSpawned {
        index: 0,
        kind: "Explore".into(),
        summary: "Map".into(),
        prompt: "Map the parser".into(),
    };
    s.apply(run_event(run, spawned), 1.0);
    let relay = |event| run_event(run, EngineEvent::AgentEvent { index: 0, event: Box::new(event) });
    s.apply(relay(EngineEvent::ToolStarted(superfik_core::ToolCall::new("s1", "Read", "src/lib.rs"))), 1.1);
    s.apply(relay(EngineEvent::TextDelta("It parses lines.".into())), 1.2);
    assert_eq!(s.conversation().agent_live_since(0), Some(1.0));
    let done = EngineEvent::AgentUpdate { index: 0, status: AgentStatus::Done { seconds: 2 }, summary: None };
    s.apply(run_event(run, done), 3.0);

    let transcript = s.conversation().agents[0].transcript.clone().expect("a session");
    assert_eq!(transcript.user.text, "Map the parser");
    assert_eq!(transcript.response.tools.len(), 1);
    assert_eq!(transcript.response.text, "It parses lines.");
    assert_eq!(transcript.response.activity, Activity::Done);
    assert!(!s.conversation().turns[0].response.text.contains("parses"), "never in the main reply");
    assert_eq!(s.conversation().agent_live_since(0), None);

    s.apply(Action::ToggleAgent(0), 3.0);
    assert_eq!(s.conversation().opened_agent().map(|a| a.index), Some(0));
    s.apply(Action::ScrollAgent { by: 50.0, max: 20.0 }, 3.0);
    assert_eq!(s.conversation().agent_scroll, 20.0);
    s.apply(Action::ToggleAgent(0), 3.0);
    assert!(s.conversation().opened_agent().is_none(), "clicking the open card closes it");
    s.apply(Action::ToggleAgent(0), 3.0);
    assert_eq!(s.conversation().agent_scroll, 0.0, "reopening follows the latest step");
    s.apply(Action::ToggleAgent(5), 3.0);
    assert_eq!(s.conversation().open_agent, Some(0), "no such card");
    s.apply(run_event(run, EngineEvent::Completed { duration_ms: 10, cost_usd: None }), 5.0);
    s.apply(Action::RemoveAgent(0), 6.0);
    assert!(s.conversation().open_agent.is_none(), "removing the card closes its sheet");
}
