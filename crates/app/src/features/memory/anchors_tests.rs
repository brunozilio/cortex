use std::path::Path;
use std::sync::Arc;

use domain::{Author, MemoryBrief, MemoryEdit, MemoryKind, ModelOption, PromptRequest, Provenance, RunId};

use super::Memories;
use super::store::InMemoryStore;

fn remember(text: &str, touches: &[&str]) -> MemoryEdit {
    MemoryEdit::Remember {
        kind: MemoryKind::Semantic,
        text: text.into(),
        tags: Vec::new(),
        touches: touches.iter().map(|touch| (*touch).to_owned()).collect(),
    }
}

#[test]
fn a_run_is_handed_the_memories_about_files_that_its_prompt_did_not_recall() {
    let project = tempfile::tempdir().expect("project");
    let memories = Memories::new(Arc::new(InMemoryStore::default()));
    let edits = [
        remember("Keep defaultRetryMax at 4", &["client.go"]),
        remember("The renderer draws signed distance fields", &[]),
    ];
    let by = Provenance { author: Author::User, now: 1, origin: None };
    memories.edit(project.path(), &edits, by).expect("edit");
    let request = PromptRequest::plain(project.path().into(), "Tune the renderer", ModelOption::new("m", "M"))
        .remembering(MemoryBrief::new(None));

    let briefed = memories.brief(RunId::new(), &request, false);

    let brief = briefed.host.memory.expect("brief");
    assert!(brief.anchors && brief.structured);
    let anchored: Vec<&str> = brief.anchored.iter().map(|memory| memory.text.as_str()).collect();
    let recalled: Vec<&str> = brief.recalled.iter().map(|memory| memory.text.as_str()).collect();
    assert_eq!(anchored.len() + recalled.len(), 2, "{anchored:?} {recalled:?}");
    assert!(!anchored.iter().any(|text| recalled.contains(text)), "a recalled memory is not handed twice");
    assert!(anchored.iter().chain(&recalled).any(|text| *text == "Keep defaultRetryMax at 4"));
    let told = brief.edits_format().expect("format");
    assert!(told.contains("\"touches\"") && told.contains("\"ruled_out\""), "{told}");
}

#[test]
fn a_project_that_turns_the_context_features_off_gets_none_of_them() {
    let project = tempfile::tempdir().expect("project");
    let settings = project.path().join(brand::DIR);
    std::fs::create_dir_all(&settings).expect("settings folder");
    let off = serde_json::json!({ "features": {
        domain::features::MEMORY_ANCHORS: false,
        domain::features::STRUCTURED_LEARNINGS: false,
        domain::features::LEARNING_CHECK: false,
        domain::features::MEMORY_SEED: false,
    } });
    std::fs::write(settings.join("settings.json"), off.to_string()).expect("settings");
    let memories = Memories::new(Arc::new(InMemoryStore::default()));
    let by = Provenance { author: Author::User, now: 1, origin: None };
    memories.edit(project.path(), &[remember("Keep retries at 4", &["client.go"])], by).expect("edit");
    let request = PromptRequest::plain(project.path().into(), "Tune the renderer", ModelOption::new("m", "M"))
        .remembering(MemoryBrief::new(None));

    let brief = memories.brief(RunId::new(), &request, false).host.memory.expect("brief");

    assert!(!brief.anchors && !brief.structured && brief.anchored.is_empty());
    let told = brief.edits_format().expect("format");
    assert!(!told.contains("\"touches\"") && !told.contains("\"ruled_out\""), "{told}");
    let refused = memories.check(project.path()).expect_err("off");
    assert!(refused.contains(domain::features::LEARNING_CHECK), "{refused}");
    let services = crate::features::context::testing::services(None);
    let seeded =
        crate::features::context::testing::with(1, &services, |context| memories.offered(context, project.path()));
    assert!(seeded.expect_err("off").contains(domain::features::MEMORY_SEED));
}

#[test]
fn the_check_needs_git_and_then_names_the_memories_about_the_changes() {
    let project = tempfile::tempdir().expect("project");
    let memories = Memories::new(Arc::new(InMemoryStore::default()));
    assert!(memories.check(project.path()).is_err());

    let git = |args: &[&str]| {
        let done = std::process::Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(project.path())
            .output()
            .expect("git");
        assert!(done.status.success());
    };
    git(&["init", "-q", "-b", "main"]);
    std::fs::write(project.path().join("client.go"), "retries = 4\n").expect("write");
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "one"]);
    let by = Provenance { author: Author::User, now: 1, origin: None };
    memories.edit(project.path(), &[remember("Keep defaultRetryMax at 4", &["client.go"])], by).expect("edit");

    let clean = memories.check(project.path()).expect("check");
    assert!(clean.prompt.is_none(), "nothing changed: {clean:?}");

    std::fs::write(project.path().join("client.go"), "retries = 10\n").expect("write");
    let changed = memories.check(Path::new(project.path())).expect("check");
    let prompt = changed.prompt.as_deref().expect("prompt");
    assert!(prompt.contains("Keep defaultRetryMax at 4") && prompt.contains("[about client.go]"), "{prompt}");
    assert_eq!((changed.changed, changed.memories), (1, 1));
}
