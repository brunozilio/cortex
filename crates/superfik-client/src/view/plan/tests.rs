#[test]
fn paths_are_shown_relative_to_root() {
    let root = std::path::Path::new("/repo");
    assert_eq!(super::relative("/repo/app/main.rs", Some(root)), "app/main.rs");
    assert_eq!(super::relative("README.md", Some(root)), "README.md");
}

use std::path::PathBuf;

use superfik_core::{Skill, SkillIcon, Turn, UserMessage};

use super::*;

#[test]
fn repository_counts_reads_once_gathering_is_over() {
    let mut session = Session::new(Some(PathBuf::from("/repo")));
    assert_eq!(repository_activity(&session), None);
    let mut turn = Turn::new(UserMessage { text: "hi".into(), images: vec![], branch_caption: String::new() });
    turn.response.tools = vec![ToolCall::new("1", "Read", "/repo/a.rs"), ToolCall::new("2", "Grep", "/repo/b.rs")];
    session.conversation_mut().turns.push(turn);
    session.conversation_mut().run = Some(crate::state::ActiveRun { id: superfik_core::RunId::new(), started_at: 0.0 });
    assert_eq!(repository_activity(&session).as_deref(), Some("Reading b.rs"));
    if let Some(turn) = session.conversation_mut().turns.last_mut() {
        turn.response.text = "Here".into();
    }
    assert_eq!(repository_activity(&session).as_deref(), Some("Read 2 files"));
}

#[test]
fn the_skills_node_stops_growing_at_a_page() {
    let height = |count: usize| {
        let skill = |i| Skill {
            name: format!("s{i}"),
            title: String::new(),
            description: String::new(),
            icon: SkillIcon::Cube,
        };
        let mut session = Session { available_skills: (0..count).map(skill).collect(), ..Session::default() };
        session.nodes.skills = true;
        context_nodes(&session).into_iter().find(|(k, _)| *k == NodeKey::Skills).map(|(_, h)| h)
    };
    assert!(height(2) < height(5));
    assert!(height(6) > height(5), "the pager joins");
    assert_eq!(height(6), height(30));
}

#[test]
fn the_context_node_shows_empty_once_asked_for() {
    let mut session = Session::new(Some(PathBuf::from("/r")));
    let keys = |s: &Session| output_nodes(s).into_iter().map(|(k, _)| k).collect::<Vec<_>>();
    assert!(!keys(&session).contains(&NodeKey::Context), "hidden by default");
    session.nodes.context = true;
    assert!(keys(&session).contains(&NodeKey::Context), "shown even with nothing measured");
    let props = context_props(&session).expect("props");
    assert!(props.usage.is_none() && !props.measuring);
}

#[test]
fn empty_session_shows_only_the_repository_until_commands_show_more() {
    let mut session = Session::default();
    let keys = |s: &Session| context_nodes(s).into_iter().map(|(k, _)| k).collect::<Vec<_>>();
    assert_eq!(keys(&session), vec![NodeKey::Repository]);
    session.nodes.skills = true;
    assert_eq!(keys(&session), vec![NodeKey::Repository, NodeKey::Skills]);
}

#[test]
fn context_nodes_follow_selection() {
    let mut session = Session::new(Some(PathBuf::from("/r")));
    session.available_skills.push(Skill {
        name: "motion".into(),
        title: "Motion skill".into(),
        description: String::new(),
        icon: SkillIcon::Bolt,
    });
    session.apply(crate::state::Action::Edit(crate::state::EditKey::Insert("/mo".into())), 0.0);
    let area = superfik_render::Rect::new(0.0, 0.0, 1838.0, 1080.0);
    let picking: Vec<NodeKey> = plan(&session, area).slots.iter().map(|s| s.key).collect();
    assert_eq!(
        picking,
        vec![NodeKey::Skills, NodeKey::Connections, NodeKey::Commands],
        "an open picker is laid out alone, its three nodes even when a list is empty"
    );
    session.apply(crate::state::Action::Edit(crate::state::EditKey::Insert("zzz".into())), 0.0);
    let none: Vec<NodeKey> =
        plan(&session, superfik_render::Rect::new(0.0, 0.0, 1838.0, 1080.0)).slots.iter().map(|s| s.key).collect();
    assert_eq!(none, vec![NodeKey::Skills, NodeKey::Connections, NodeKey::Commands], "no match keeps them empty");
    session.apply(crate::state::Action::Edit(crate::state::EditKey::Backspace), 0.0);
    session.apply(crate::state::Action::Edit(crate::state::EditKey::Backspace), 0.0);
    session.apply(crate::state::Action::Edit(crate::state::EditKey::Backspace), 0.0);
    session.apply(crate::state::Action::Edit(crate::state::EditKey::Insert("tion ".into())), 0.0);
    session.nodes.skills = true;
    session.nodes.memory = true;
    session.conversation_mut().references.push(PathBuf::from("/a.png"));
    let c = session.conversation_mut();
    c.research_items.push(superfik_core::ResearchItem {
        title: "Moonlight".into(),
        url: None,
        kind: superfik_core::ResearchKind::Article,
    });
    c.agents = (0..2).map(|i| superfik_core::SubAgent::spawned(i, "Explore", "", "", &[])).collect();
    let layout = plan(&session, superfik_render::Rect::new(0.0, 0.0, 1838.0, 1080.0));
    let keys: Vec<NodeKey> = layout.slots.iter().map(|s| s.key).collect();
    assert_eq!(
        keys,
        vec![
            NodeKey::Repository,
            NodeKey::Skills,
            NodeKey::References,
            NodeKey::Memory,
            NodeKey::Agent(0),
            NodeKey::Agent(1),
            NodeKey::Research
        ]
    );
}
