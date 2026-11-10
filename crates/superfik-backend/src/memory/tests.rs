use superfik_core::{MemoryKind, ModelOption};

use super::store::memory_store_in;
use super::*;

fn request(cwd: &Path) -> PromptRequest {
    PromptRequest {
        memory: Some(MemoryBrief::new(Some("Night sky".into()))),
        ..PromptRequest::plain(cwd.to_path_buf(), "Tune the renderer", ModelOption::new("opus", "Opus"))
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
fn memory_lives_in_the_data_folder_or_only_in_the_process() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut book = MemoryBook::default();
    book.apply(&[remember("Uses wgpu")], &by_user());
    let owner = Principal::LocalOwner;
    let project = Path::new("/repo");

    let on_disk = memory_store_in(Some(dir.path().to_path_buf()));
    on_disk.save(&owner, project, &book).expect("save");
    assert!(dir.path().join("memory/local/-repo.json").exists());

    let in_process = memory_store_in(None);
    in_process.save(&owner, project, &book).expect("save");
    assert_eq!(in_process.load(&owner, project), book, "kept for the process, never in a temp folder");
}

#[test]
fn an_unreadable_book_is_kept_aside_and_never_overwritten() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileMemoryStore::new(dir.path().to_path_buf());
    let project = Path::new("/repo");
    let mut book = MemoryBook::default();
    book.apply(&[remember("Uses wgpu")], &by_user());
    store.save(&Principal::LocalOwner, project, &book).expect("save");
    let folder = dir.path().join("local");
    let file = folder.join("-repo.json");
    std::fs::write(&file, br#"{"next": 1, "memories": [{"id": 0, "#).expect("truncate");

    assert_eq!(store.load(&Principal::LocalOwner, project), MemoryBook::default());
    store.save(&Principal::LocalOwner, project, &MemoryBook::default()).expect("save");

    let kept: Vec<String> = std::fs::read_dir(&folder)
        .expect("folder")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.starts_with("-repo.json.corrupt-"))
        .collect();
    assert_eq!(kept.len(), 1, "the unreadable file is kept aside: {kept:?}");
    let aside = std::fs::read(folder.join(&kept[0])).expect("read");
    assert_eq!(aside, br#"{"next": 1, "memories": [{"id": 0, "#);
}

#[test]
fn edits_answer_with_the_whole_memory() {
    let memories = Memories::plain(Arc::new(InMemoryStore::default()));
    let owner = Principal::LocalOwner;
    memories.edit(&owner, Path::new("/repo"), &[remember("Uses wgpu")], by_user());
    match memories.edit(&owner, Path::new("/repo"), &[remember("Inter is the only font")], by_user()) {
        ServerEvent::Memory { path, memories } => {
            assert_eq!(path, PathBuf::from("/repo"));
            assert_eq!(memories.len(), 2);
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

    let briefed = memories.brief(&owner, RunId::new(), &request(dir.path()), false);
    let brief = briefed.memory.clone().expect("brief");
    assert_eq!(brief.recalled.len(), 1);
    assert_eq!(brief.origin.as_deref(), Some("Night sky"));
    assert!(!brief.output.as_os_str().is_empty());

    std::fs::write(&brief.output, r#"{"remember":[{"kind":"episodic","text":"Tuned the renderer"}],"forget":[0]}"#)
        .expect("write");
    match memories.finish(&owner, &briefed, true) {
        Some(ServerEvent::Memory { memories, .. }) => {
            assert_eq!(memories.len(), 1);
            assert_eq!((memories[0].kind, memories[0].author), (MemoryKind::Episodic, Author::Agent));
            assert_eq!(memories[0].origin.as_deref(), Some("Night sky"));
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
    let briefed = memories.brief(&owner, RunId::new(), &request(dir.path()), false);
    let output = briefed.memory.clone().expect("brief").output;
    std::fs::write(&output, r#"{"remember":[{"kind":"semantic","text":"Half done"}]}"#).expect("write");
    assert_eq!(memories.finish(&owner, &briefed, false), None);
    assert!(!output.exists());
    let mut no_memory = request(dir.path());
    no_memory.memory = None;
    assert_eq!(memories.brief(&owner, RunId::new(), &no_memory, false), no_memory);
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
    let with = memories.brief(&owner, RunId::new(), &request(dir.path()), true).memory.expect("brief");
    let without = memories.brief(&owner, RunId::new(), &request(dir.path()), false).memory.expect("brief");
    assert_eq!((with.recalled.len(), without.recalled.len()), (RecallBudget::WITH_TOOL.facts, 30));
    assert!(with.tool);
}
