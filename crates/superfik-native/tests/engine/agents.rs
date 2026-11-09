//! A run's configured main agent: its prompt, initial prompt, memory,
//! tools, and the backend's model guard over it and its children.

use std::sync::{Arc, Mutex};

use serde_json::json;
use superfik_core::{EngineEvent, PromptRequest};
use superfik_native::NativeEngine;
use superfik_native::model::scripted::ScriptedModel;
use superfik_native::sessions::InMemorySessions;
use superfik_native::transcript::Block;

use crate::{Source, finish, model, start};

#[test]
fn configured_main_agent_applies_initial_prompt_once_and_blocks_unadvertised_tools() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join(".superfik");
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
    let engine = NativeEngine::new(
        Arc::new(Source(Mutex::new(vec![first.clone(), second.clone()]))),
        Arc::new(InMemorySessions::default()),
    );
    let request = PromptRequest::plain(dir.path().into(), "Check this project", model());
    let events = finish(&start(&engine, &request));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    assert!(!dir.path().join("forbidden").exists());
    let asked = first.asked();
    assert!(asked[0].system.contains("You review carefully."));
    assert!(asked[0].messages.last().expect("first user").text().starts_with("Start with a review."));
    assert_eq!(asked[0].effort, superfik_core::Effort::Low);
    let session = events
        .iter()
        .find_map(|event| if let EngineEvent::SessionStarted(id) = event { Some(id.clone()) } else { None })
        .expect("session");
    let mut follow = PromptRequest::plain(dir.path().into(), "Continue", model());
    follow.resume = Some(session);
    assert!(matches!(finish(&start(&engine, &follow)).last(), Some(EngineEvent::Completed { .. })));
    assert_eq!(second.asked()[0].messages.last().expect("follow-up").text(), "Continue");
}

#[test]
fn disabling_auto_memory_prevents_configured_agent_memory_setup() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join(".superfik");
    std::fs::create_dir_all(root.join("agents")).expect("agents");
    std::fs::write(root.join("settings.json"), r#"{"agent":"reviewer","autoMemoryEnabled":false}"#).expect("settings");
    std::fs::write(
        root.join("agents/reviewer.md"),
        "---\nname: reviewer\ntools: [Read]\nmemory: project\n---\nReview.",
    )
    .expect("agent");
    let model_api = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Reviewed".into())])]));
    let engine =
        NativeEngine::new(Arc::new(Source(Mutex::new(vec![model_api.clone()]))), Arc::new(InMemorySessions::default()));
    assert!(matches!(
        finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "Review", model()))).last(),
        Some(EngineEvent::Completed { .. })
    ));
    assert!(!root.join("agent-memory").exists());
    let asked = model_api.asked();
    assert!(!asked[0].system.contains("Persistent agent memory"));
    assert_eq!(asked[0].tools.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["Read"]);
}

#[test]
fn initial_prompt_expands_an_explicit_skill_before_the_first_model_call_through_the_worker() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join(".superfik");
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
        NativeEngine::new(Arc::new(Source(Mutex::new(vec![model_api.clone()]))), Arc::new(InMemorySessions::default()))
            .with_worker(std::path::Path::new(env!("CARGO_BIN_EXE_superfik-native-worker")).into());
    let events = finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "Review this", model())));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    let first = model_api.asked()[0].messages.last().expect("first user").text();
    assert!(first.contains("INITIAL_SKILL_BODY_SENTINEL project-A"));
    assert!(!first.contains("$ARGUMENTS"));
    assert!(!model_api.asked()[0].system.contains("- start:"));
}
#[test]
fn initial_prompt_expands_a_legacy_custom_command() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join(".claude");
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
        NativeEngine::new(Arc::new(Source(Mutex::new(vec![model_api.clone()]))), Arc::new(InMemorySessions::default()));
    let events = finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "Review this", model())));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    assert!(
        model_api.asked()[0].messages.last().expect("first user").text().contains("INITIAL_COMMAND_SENTINEL project-B")
    );
}

#[test]
fn the_backend_model_guard_blocks_a_configured_model_override_before_inference() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join(".superfik");
    std::fs::create_dir_all(root.join("agents")).expect("agents");
    std::fs::write(root.join("settings.json"), r#"{"agent":"reviewer"}"#).expect("settings");
    std::fs::write(root.join("agents/reviewer.md"), "---\nname: reviewer\nmodel: haiku\n---\nReview.").expect("agent");
    let model_api = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Must not run".into())])]));
    let engine =
        NativeEngine::new(Arc::new(Source(Mutex::new(vec![model_api.clone()]))), Arc::new(InMemorySessions::default()));
    let mut request = PromptRequest::plain(dir.path().into(), "Review", model());
    request.credentials.model_guard = Some(superfik_core::credentials::ModelGuard::new(|model| {
        if model.id.contains("haiku") { Err("fixture policy forbids the selected model".into()) } else { Ok(()) }
    }));
    let events = finish(&start(&engine, &request));
    assert!(matches!(events.last(), Some(EngineEvent::Failed { .. })), "{events:?}");
    assert!(model_api.asked().is_empty(), "policy must reject before a provider call");
    assert!(!serde_json::to_string(&request).expect("wire").contains("model_guard"));
}
#[test]
fn the_model_guard_also_checks_the_actual_model_of_a_child() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join(".superfik/agents");
    std::fs::create_dir_all(&root).expect("agents");
    std::fs::write(root.join("reviewer.md"), "---\nname: reviewer\nmodel: haiku\n---\nReview.").expect("agent");
    let model_api = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "delegate".into(),
            name: "Agent".into(),
            input: json!({"description":"Review project","prompt":"Review","subagent_type":"reviewer"}),
        }]),
        ScriptedModel::reply(vec![Block::Text("Child was denied".into())]),
    ]));
    let engine =
        NativeEngine::new(Arc::new(Source(Mutex::new(vec![model_api.clone()]))), Arc::new(InMemorySessions::default()));
    let mut request = PromptRequest::plain(dir.path().into(), "Review", model());
    request.credentials.model_guard = Some(superfik_core::credentials::ModelGuard::new(|model| {
        if model.id.contains("haiku") { Err("child model denied".into()) } else { Ok(()) }
    }));
    let events = finish(&start(&engine, &request));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    assert_eq!(model_api.asked().len(), 2);
    assert!(format!("{:?}", model_api.asked()[1].messages).contains("child model denied"));
}
