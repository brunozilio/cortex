use std::sync::{Arc, Mutex};

use agent_engine::Engine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;
use domain::{EngineEvent, PromptRequest};
use serde_json::json;

use crate::{Source, finish, model, start};

#[test]
fn configured_main_agent_applies_initial_prompt_once_and_blocks_unadvertised_tools() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join(brand::DIR);
    std::fs::create_dir_all(root.join("agents")).expect("agents");
    std::fs::write(root.join("settings.json"), r#"{"agent":"reviewer"}"#).expect("settings");
    std::fs::write(root.join("agents/reviewer.md"),"---\nname: reviewer\ntools: [Read]\ninitialPrompt: Start with a review.\neffort: low\npermissionMode: dontAsk\n---\nYou review carefully.").expect("agent");
    let first = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "bad".into(),
            name: "Bash".into(),
            input: json!({"command":"echo bad > forbidden"}),
        }]),
        ScriptedModel::reply(vec![Block::Text("Reviewed".into())]),
    ]));
    let second = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Again".into())])]));
    let engine = Engine::new(
        Arc::new(Source(Mutex::new(vec![first.clone(), second.clone()]))),
        Arc::new(InMemorySessions::default()),
    );
    let request = crate::trusted(dir.path(), "Check this project");
    let events = finish(&start(&engine, &request));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    assert!(!dir.path().join("forbidden").exists());
    let asked = first.asked();
    assert!(asked[0].system.contains("You review carefully."));
    assert!(asked[0].messages.last().expect("first user").text().starts_with("Start with a review."));
    assert_eq!(asked[0].effort, domain::Effort::Low);
    let session = events
        .iter()
        .find_map(|event| if let EngineEvent::SessionStarted(id) = event { Some(id.clone()) } else { None })
        .expect("session");
    let mut follow = crate::trusted(dir.path(), "Continue");
    follow.session.resume = Some(session);
    assert!(matches!(finish(&start(&engine, &follow)).last(), Some(EngineEvent::Completed { .. })));
    assert_eq!(second.asked()[0].messages.last().expect("follow-up").text(), "Continue");
}

#[test]
fn an_untrusted_folder_s_agent_keeps_its_instructions_and_loses_its_first_prompt_mode_and_hooks() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join(brand::DIR);
    std::fs::create_dir_all(root.join("agents")).expect("agents");
    std::fs::write(root.join("settings.json"), r#"{"agent":"reviewer"}"#).expect("settings");
    let agent = "---\nname: reviewer\ninitialPrompt: Start with a review.\npermissionMode: bypassPermissions\nhooks:\n  \
                 PreToolUse:\n    - hooks:\n        - type: command\n          command: touch hooked\n---\nYou review carefully.";
    std::fs::write(root.join("agents/reviewer.md"), agent).expect("agent");
    std::fs::write(dir.path().join("notes.txt"), "alpha").expect("notes");
    let scripted = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "r1".into(),
            name: "Read".into(),
            input: json!({"file_path":"notes.txt"}),
        }]),
        ScriptedModel::reply(vec![Block::Text("Reviewed".into())]),
    ]));
    let engine =
        Engine::new(Arc::new(Source(Mutex::new(vec![scripted.clone()]))), Arc::new(InMemorySessions::default()));

    let events = finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "Check this project", model())));

    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    let asked = scripted.asked();
    assert!(asked[0].system.contains("You review carefully."), "its instructions are prompt text and still apply");
    assert_eq!(
        asked[0].messages.last().expect("first user").text(),
        "Check this project",
        "no first prompt of its own"
    );
    assert!(!dir.path().join("hooked").exists(), "its hook did not run");
}

#[test]
fn a_leftover_auto_memory_switch_leaves_agent_memory_on() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join(brand::DIR);
    std::fs::create_dir_all(root.join("agents")).expect("agents");
    std::fs::write(root.join("settings.json"), r#"{"agent":"reviewer","autoMemoryEnabled":false}"#).expect("settings");
    std::fs::write(
        root.join("agents/reviewer.md"),
        "---\nname: reviewer\ntools: [Read]\nmemory: project\n---\nReview.",
    )
    .expect("agent");
    let model_api = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Reviewed".into())])]));
    let engine =
        Engine::new(Arc::new(Source(Mutex::new(vec![model_api.clone()]))), Arc::new(InMemorySessions::default()));
    assert!(matches!(
        finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "Review", model()))).last(),
        Some(EngineEvent::Completed { .. })
    ));
    assert!(root.join("agent-memory").exists());
    let asked = model_api.asked();
    assert!(asked[0].system.contains("Persistent agent memory"));
}

#[test]
fn initial_prompt_expands_an_explicit_skill_before_the_first_model_call_through_the_worker() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join(brand::DIR);
    std::fs::create_dir_all(root.join("agents")).expect("agents");
    std::fs::create_dir_all(root.join("skills/start")).expect("skill");
    std::fs::write(root.join("settings.json"), r#"{"agent":"reviewer"}"#).expect("settings");
    std::fs::write(
        root.join("agents/reviewer.md"),
        "---\nname: reviewer\ninitialPrompt: /start project-A\n---\nReview.",
    )
    .expect("agent");
    std::fs::write(
        root.join("skills/start/SKILL.md"),
        "---\nname: start\ndisable-model-invocation: true\n---\nINITIAL_SKILL_BODY_SENTINEL $ARGUMENTS",
    )
    .expect("skill");
    let model_api = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Reviewed".into())])]));
    let engine =
        Engine::new(Arc::new(Source(Mutex::new(vec![model_api.clone()]))), Arc::new(InMemorySessions::default()));
    let events = finish(&start(&engine, &crate::trusted(dir.path(), "Review this")));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    let first = model_api.asked()[0].messages.last().expect("first user").text();
    assert!(first.contains("INITIAL_SKILL_BODY_SENTINEL project-A"));
    assert!(!first.contains("$ARGUMENTS"));
    assert!(!model_api.asked()[0].system.contains("- start:"));
}
#[test]
fn initial_prompt_expands_a_custom_command() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join(brand::DIR);
    std::fs::create_dir_all(root.join("agents")).expect("agents");
    std::fs::create_dir_all(root.join("commands")).expect("commands");
    std::fs::write(root.join("settings.json"), r#"{"agent":"reviewer"}"#).expect("settings");
    std::fs::write(
        root.join("agents/reviewer.md"),
        "---\nname: reviewer\ninitialPrompt: /audit project-B\n---\nReview.",
    )
    .expect("agent");
    std::fs::write(
        root.join("commands/audit.md"),
        "---\ndescription: Audit the project\n---\nINITIAL_COMMAND_SENTINEL $ARGUMENTS",
    )
    .expect("command");
    let model_api = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Reviewed".into())])]));
    let engine =
        Engine::new(Arc::new(Source(Mutex::new(vec![model_api.clone()]))), Arc::new(InMemorySessions::default()));
    let events = finish(&start(&engine, &crate::trusted(dir.path(), "Review this")));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    assert!(
        model_api.asked()[0].messages.last().expect("first user").text().contains("INITIAL_COMMAND_SENTINEL project-B")
    );
}

#[test]
fn a_typed_slash_command_becomes_its_text_and_a_path_stays_as_typed() {
    let dir = tempfile::tempdir().expect("dir");
    let commands = dir.path().join(concat!(brand::dir!(), "/commands"));
    std::fs::create_dir_all(&commands).expect("commands");
    std::fs::write(commands.join("audit.md"), "---\ndescription: Audit\n---\nTYPED_COMMAND_SENTINEL $ARGUMENTS")
        .expect("command");
    for (typed, expected) in [("/audit project-C", "TYPED_COMMAND_SENTINEL project-C"), ("/etc/hosts", "/etc/hosts")] {
        let model_api = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Done".into())])]));
        let engine =
            Engine::new(Arc::new(Source(Mutex::new(vec![model_api.clone()]))), Arc::new(InMemorySessions::default()));
        let events = finish(&start(&engine, &PromptRequest::plain(dir.path().into(), typed, model())));
        assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
        let sent = model_api.asked()[0].messages.last().expect("user").text();
        assert!(sent.contains(expected), "{typed}: {sent}");
    }
}

#[test]
fn a_javascript_command_and_skill_write_their_text_from_their_arguments() {
    let dir = tempfile::tempdir().expect("dir");
    let commands = dir.path().join(concat!(brand::dir!(), "/commands"));
    let skill = dir.path().join(concat!(brand::dir!(), "/skills/count"));
    std::fs::create_dir_all(&commands).expect("commands");
    std::fs::create_dir_all(&skill).expect("skill");
    std::fs::write(
        commands.join("greet.js"),
        r#"export const description = "Greets someone.";
export default (args, ctx) => args ? `JS_COMMAND greet ${args.toUpperCase()}` : "JS_COMMAND nobody";"#,
    )
    .expect("command");
    std::fs::write(
        skill.join("skill.js"),
        r#"export const description = "Counts words.";
export default (args) => `JS_SKILL ${args.split(" ").filter(Boolean).length} words`;"#,
    )
    .expect("skill");
    for (typed, expected) in [("/greet ana", "JS_COMMAND greet ANA"), ("/count one two three", "JS_SKILL 3 words")] {
        let model_api = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Done".into())])]));
        let engine =
            Engine::new(Arc::new(Source(Mutex::new(vec![model_api.clone()]))), Arc::new(InMemorySessions::default()));
        let events = finish(&start(&engine, &crate::trusted(dir.path(), typed)));
        assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
        let sent = model_api.asked()[0].messages.last().expect("user").text();
        assert!(sent.contains(expected), "{typed}: {sent}");
        let system = &model_api.asked()[0].system;
        assert!(system.contains("count: Counts words."), "the skill is listed for the model");
    }
}
