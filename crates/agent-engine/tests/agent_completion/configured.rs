use agent_engine::{
    model::{ModelError, Reply, scripted::ScriptedModel},
    prompt::{self, Environment, Instructions, Scope},
    tools::Workspace,
    transcript::Usage,
};
use domain::PermissionMode;
use serde_json::json;

use crate::{call, delegated, done, run, run_named};

#[test]
fn dont_ask_denies_unapproved_commands_without_a_prompt() {
    let dir = tempfile::tempdir().expect("dir");
    let workspace = Workspace::new(dir.path());
    let model = ScriptedModel::new([
        delegated(),
        ScriptedModel::reply(vec![call("bad", "Bash", json!({"command":"echo changed > forbidden"}))]),
        done("Denied"),
        done("Reviewed"),
    ]);
    run(json!({"permissionMode":"dontAsk"}), &model, &workspace, PermissionMode::Ask);
    assert!(!dir.path().join("forbidden").exists());
    assert!(format!("{:?}", model.asked()[2].messages).contains("dontAsk denies"));
}
#[test]
fn explore_runs_on_the_small_model_unless_it_names_its_own() {
    agent_engine::providers::install_testing();
    let dir = tempfile::tempdir().expect("dir");
    let mut workspace = Workspace::new(dir.path());
    workspace.model =
        domain::ModelOption::new("claude-opus-5-5", "Opus").served_by(domain::ModelProvider::new("family-key"));
    workspace.small_model = Some("claude-haiku-5-5".into());
    let explore = |model: serde_json::Value| {
        ScriptedModel::reply(vec![call(
            "e",
            "Agent",
            json!({"description":"Find","prompt":"find x","subagent_type":"Explore","model":model}),
        )])
    };
    let model =
        ScriptedModel::new([explore(json!(null)), done("Found"), explore(json!("opus")), done("Found"), done("Done")]);
    run_named("Explore", json!({}), &model, &workspace, PermissionMode::Bypass);
    let asked = model.asked();
    assert_eq!(asked[1].model, "claude-haiku-5-5");
    assert_eq!(asked[3].model, "claude-opus-5-5", "an explicit model wins");
}

#[test]
fn child_omits_initial_instruction_files_and_parent_output_style() {
    let dir = tempfile::tempdir().expect("dir");
    let mut workspace = Workspace::new(dir.path());
    workspace.prompt_context = Some(Environment {
        cwd: dir.path().into(),
        instructions: vec![Instructions {
            path: dir.path().join("CLAUDE.md"),
            scope: Scope::Project,
            text: "PARENT_FILE_SENTINEL".into(),
        }],
        output_style: Some(prompt::settings::Style {
            name: "parent-style".into(),
            instructions: "PARENT_STYLE_SENTINEL".into(),
            keep_coding: true,
            reminder: None,
        }),
        ..Default::default()
    });
    let model = ScriptedModel::new([delegated(), done("Reviewed"), done("Done")]);
    run(json!({"omitClaudeMd":true}), &model, &workspace, PermissionMode::Bypass);
    assert!(model.asked()[0].system.contains("PARENT_FILE_SENTINEL"));
    let child = &model.asked()[1].system;
    assert!(!child.contains("PARENT_FILE_SENTINEL") && !child.contains("PARENT_STYLE_SENTINEL"));
    assert!(child.contains("Do the review."));
}

#[test]
fn configured_auto_compact_window_triggers_child_compaction_before_the_model_window_fills() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("small.txt"), "small result").expect("file");
    let workspace = Workspace::new(dir.path());
    let uncounted = |reply: Result<Reply, ModelError>| reply.map(|reply| Reply { usage: Usage::default(), ..reply });
    let model = ScriptedModel::new(
        [
            ScriptedModel::reply(vec![call(
                "delegate",
                "Agent",
                json!({"description":"Review large input","prompt":"x".repeat(420_000),"subagent_type":"reviewer"}),
            )]),
            ScriptedModel::reply(vec![call("read", "Read", json!({"file_path":"small.txt"}))]),
            done("Compact summary preserving the review task"),
            done("Child reviewed"),
            done("Parent done"),
        ]
        .map(uncounted),
    );
    run(json!({"autoCompactWindow":100_000}), &model, &workspace, PermissionMode::Bypass);
    let asked = model.asked();
    assert_eq!(asked.len(), 5);
    assert_eq!(asked[2].tools, asked[1].tools, "the summary reuses the cached prefix of the turn before");
    assert_eq!(asked[2].system, asked[1].system);
    assert!(
        asked[2].messages.last().expect("summary task").text().contains("Write a detailed summary of the conversation")
    );
    assert!(asked[3].messages.iter().any(|message| message.text().contains("Compact summary preserving")));
}
