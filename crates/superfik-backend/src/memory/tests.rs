use superfik_core::{Effort, EngineKind, MemoryId, MemoryKind, ModelOption, WorkspaceId};

use super::*;

fn request(cwd: &Path) -> PromptRequest {
    PromptRequest {
        connections: Vec::new(),
        plugins: None,
        credentials: Default::default(),
        workspace: WorkspaceId::new(),
        cwd: cwd.to_path_buf(),
        prompt: "Tune the renderer".into(),
        model: ModelOption::new(EngineKind::Native, "opus", "Opus"),
        effort: Effort::Medium,
        repositories: vec![],
        skills: vec![],
        disabled_skills: Vec::new(),
        references: vec![],
        resume: None,
        command: None,
        memory: Some(MemoryBrief::new(Some("Night sky".into()))),
        permission: Default::default(),
        output_style: None,
        safety_check: true,
    }
}

fn remember(text: &str) -> MemoryEdit {
    MemoryEdit::Remember { kind: MemoryKind::Semantic, text: text.into(), tags: vec!["render".into()] }
}

fn by_user() -> Provenance {
    Provenance { author: Author::User, now: 5, origin: None }
}

#[test]
fn project_keys_are_flat_file_names() {
    assert_eq!(project_key(Path::new("/Users/me/sky.app")), "-Users-me-sky-app");
}

#[test]
fn file_store_round_trips_per_owner() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileMemoryStore::new(dir.path().to_path_buf());
    let project = Path::new("/repo");
    let mut book = MemoryBook::default();
    book.apply(&[remember("Uses wgpu")], &by_user());
    store.save(&Principal::LocalOwner, project, &book).expect("save");
    assert_eq!(store.load(&Principal::LocalOwner, project), book);
    assert_eq!(
        store.load(&Principal::member(superfik_core::UserId::new(), superfik_core::OrgId::new()), project),
        MemoryBook::default()
    );
    assert_eq!(store.load(&Principal::LocalOwner, Path::new("/other")), MemoryBook::default());
}

#[test]
fn edits_answer_with_the_whole_memory() {
    let memories = Memories::plain(Arc::new(InMemoryStore::default()));
    let owner = Principal::LocalOwner;
    memories.edit(&owner, Path::new("/repo"), &[remember("Uses wgpu")], by_user());
    match memories.edit(&owner, Path::new("/repo"), &[remember("Inter is the only font")], by_user()) {
        ServerEvent::Memory { path, memories, learned } => {
            assert_eq!(path, PathBuf::from("/repo"));
            assert_eq!(memories.len(), 2);
            assert_eq!(learned, vec![MemoryId(1)]);
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn a_run_is_briefed_and_its_edits_applied_on_completion() {
    let dir = tempfile::tempdir().expect("tempdir");
    let memories = Memories::plain(Arc::new(InMemoryStore::default()));
    let owner = Principal::LocalOwner;
    memories.edit(&owner, dir.path(), &[remember("The renderer uses signed distance fields")], by_user());

    let briefed = memories.brief(&owner, RunId::new(), &request(dir.path()), None);
    let brief = briefed.memory.clone().expect("brief");
    assert_eq!(brief.recalled.len(), 1);
    assert_eq!(brief.origin.as_deref(), Some("Night sky"));
    assert!(!brief.output.as_os_str().is_empty());

    std::fs::write(&brief.output, r#"{"remember":[{"kind":"episodic","text":"Tuned the renderer"}],"forget":[0]}"#)
        .expect("write");
    match memories.finish(&owner, &briefed, true) {
        Some(ServerEvent::Memory { memories, learned, .. }) => {
            assert_eq!(memories.len(), 1);
            assert_eq!((memories[0].kind, memories[0].author), (MemoryKind::Episodic, Author::Agent));
            assert_eq!(memories[0].origin.as_deref(), Some("Night sky"));
            assert_eq!(learned, vec![MemoryId(1)]);
        }
        other => panic!("unexpected {other:?}"),
    }
    assert!(!brief.output.exists(), "the edits file is removed");
}

#[test]
fn a_failed_run_changes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let memories = Memories::plain(Arc::new(InMemoryStore::default()));
    let owner = Principal::LocalOwner;
    let briefed = memories.brief(&owner, RunId::new(), &request(dir.path()), None);
    let output = briefed.memory.clone().expect("brief").output;
    std::fs::write(&output, r#"{"remember":[{"kind":"semantic","text":"Half done"}]}"#).expect("write");
    assert_eq!(memories.finish(&owner, &briefed, false), None);
    assert!(!output.exists());
    let mut no_memory = request(dir.path());
    no_memory.memory = None;
    assert_eq!(memories.brief(&owner, RunId::new(), &no_memory, None), no_memory);
}

struct Buckets;
impl Embedder for Buckets {
    fn model(&self) -> &str {
        "buckets"
    }

    fn embed(&self, texts: &[String], _purpose: Purpose) -> Option<Vec<Vec<f32>>> {
        let synonyms = |w: &str| match w {
            "superfícies" | "surface" | "renderer" => 0,
            "lint" | "clippy" => 1,
            _ => 2 + w.len() % 6,
        };
        Some(
            texts
                .iter()
                .map(|t| {
                    let mut v = vec![0.0f32; 8];
                    for word in t.to_lowercase().split_whitespace() {
                        v[synonyms(word)] += 1.0;
                    }
                    v
                })
                .collect(),
        )
    }
}

#[test]
fn search_uses_meaning_once_vectors_exist() {
    let memories = Memories::new(Arc::new(InMemoryStore::default()), Arc::new(Buckets));
    let owner = Principal::LocalOwner;
    let project = Path::new("/repo");
    memories.edit(&owner, project, &[remember("The renderer draws every surface"), remember("Use clippy")], by_user());
    let hits = memories.search(&owner, project, "superfícies");
    assert_eq!(hits.first().map(|h| h.0), Some(MemoryId(0)), "found by meaning: {hits:?}");
    let plain = Memories::plain(Arc::new(InMemoryStore::default()));
    plain.edit(&owner, project, &[remember("The renderer draws every surface")], by_user());
    assert!(plain.search(&owner, project, "superfícies").is_empty(), "words alone miss it");
}

#[test]
fn a_tool_shortens_what_is_recalled() {
    let memories = Memories::plain(Arc::new(InMemoryStore::default()));
    let owner = Principal::LocalOwner;
    let dir = tempfile::tempdir().expect("tempdir");
    let edits: Vec<MemoryEdit> = (0..30).map(|n| remember(&format!("Fact number {n}"))).collect();
    memories.edit(&owner, dir.path(), &edits, by_user());
    let tool = MemoryTool { url: "http://127.0.0.1:1/mcp".into(), token: "t".into() };
    let with = memories.brief(&owner, RunId::new(), &request(dir.path()), Some(tool)).memory.expect("brief");
    let without = memories.brief(&owner, RunId::new(), &request(dir.path()), None).memory.expect("brief");
    assert_eq!((with.recalled.len(), without.recalled.len()), (RecallBudget::WITH_TOOL.facts, 30));
    assert!(with.tool.is_some());
}

#[test]
fn an_optimisation_can_be_undone_until_the_next_edit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let memories = Memories::plain(Arc::new(InMemoryStore::default()));
    let owner = Principal::LocalOwner;
    memories.edit(&owner, dir.path(), &[remember("Uses wgpu"), remember("The renderer uses wgpu")], by_user());
    let model = ModelOption::new(EngineKind::Native, "opus", "Opus");
    let request = memories.optimization(&owner, dir.path(), model);
    assert!(request.prompt.contains("[0] semantic (render): Uses wgpu"));
    let briefed = memories.brief(&owner, RunId::new(), &request, None);
    let brief = briefed.memory.clone().expect("brief");
    assert!(brief.optimizing && brief.recalled.is_empty() && brief.tool.is_none());
    std::fs::write(&brief.output, r#"{"forget":[0]}"#).expect("write");
    match memories.finish(&owner, &briefed, true) {
        Some(ServerEvent::Memory { memories, .. }) => assert_eq!(memories.len(), 1),
        other => panic!("unexpected {other:?}"),
    }
    match memories.undo(&owner, dir.path()) {
        ServerEvent::Memory { memories, .. } => assert_eq!(memories.len(), 2, "both memories are back"),
        other => panic!("unexpected {other:?}"),
    }
    assert!(matches!(memories.undo(&owner, dir.path()), ServerEvent::Error { .. }), "only once");
    let briefed = memories.brief(
        &owner,
        RunId::new(),
        &memories.optimization(&owner, dir.path(), ModelOption::new(EngineKind::Native, "o", "O")),
        None,
    );
    std::fs::write(&briefed.memory.clone().expect("brief").output, r#"{"forget":[1]}"#).expect("write");
    memories.finish(&owner, &briefed, true);
    memories.edit(&owner, dir.path(), &[remember("New fact")], by_user());
    assert!(matches!(memories.undo(&owner, dir.path()), ServerEvent::Error { .. }));
}
