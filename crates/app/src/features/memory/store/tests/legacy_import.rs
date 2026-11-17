use domain::legacy::OWNER_DIR;

use super::super::legacy::LegacyMemoryStore;
use super::*;
use crate::DataRoot;

const WITH_EMBEDDINGS: &str = r#"{
    "next": 2,
    "memories": [
        {"id": 0, "kind": "semantic", "text": "Uses wgpu", "tags": ["render"], "created": 10, "author": "agent"},
        {"id": 1, "kind": "episodic", "text": "Fixed the cache", "created": 20, "author": "user"}
    ],
    "vectors": {"0": [0.25, 0.75], "1": [1.0, 0.0]},
    "model": "mini"
}"#;

const IMPORTED: [&str; 3] = ["-work-empty.json.imported", "-work-old.json.imported", "-work-project.json.imported"];

fn learned() -> MemoryBook {
    let mut book = MemoryBook::default();
    for change in what_a_project_learns() {
        change(&mut book);
    }
    book
}

fn fixture(data: &Path) -> MemoryBook {
    let legacy = LegacyMemoryStore::new(data.join("memory"));
    legacy.save(Path::new("/work/project"), &learned());
    legacy.save(Path::new("/work/empty"), &MemoryBook::default());
    std::fs::write(data.join("memory/local/-work-old.json"), WITH_EMBEDDINGS).expect("old book");
    learned()
}

fn files(data: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(data.join("memory").join(OWNER_DIR))
        .expect("folder")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| !name.ends_with(".lock"))
        .collect();
    names.sort();
    names
}

fn store(data: &Path) -> Arc<dyn MemoryStore> {
    DataRoot::under(data).stores().memories.store
}

#[test]
fn the_memory_of_an_earlier_version_is_imported_once_and_its_files_renamed() {
    let data = tempfile::tempdir().expect("data");
    let legacy = fixture(data.path());
    let before = std::fs::read(data.path().join("memory/local/-work-project.json")).expect("legacy bytes");

    let kept = store(data.path());

    assert_eq!(kept.read(Path::new("/work/project")), Revised { revision: 1, book: legacy.clone() });
    assert_eq!(kept.read(Path::new("/work/empty")), Revised::default());
    let old = kept.load(Path::new("/work/old"));
    assert_eq!(
        old.memories.iter().map(|memory| memory.text.as_str()).collect::<Vec<_>>(),
        ["Uses wgpu", "Fixed the cache"]
    );
    assert_eq!((old.next, old.memories[0].author, old.memories[1].kind), (2, Author::Agent, MemoryKind::Episodic));
    assert_eq!(files(data.path()), IMPORTED);
    let renamed = data.path().join("memory/local/-work-project.json.imported");
    assert_eq!(std::fs::read(renamed).expect("kept"), before, "renamed, never rewritten");

    kept.update(Path::new("/work/project"), &mut |book| drop(book.apply(&[fact("Learned since")], &by(99))))
        .expect("update");
    drop(kept);
    let again = store(data.path());
    assert_eq!(again.revision(Path::new("/work/project")), 2, "opening again imports nothing");
    assert_eq!(files(data.path()), IMPORTED);
}

#[test]
fn memory_files_put_back_beside_a_database_that_has_memory_are_left_alone() {
    let data = tempfile::tempdir().expect("data");
    let kept = store(data.path());
    kept.update(Path::new("/work/newer"), &mut |book| drop(book.apply(&[fact("Newer")], &by(5)))).expect("update");
    fixture(data.path());

    let reopened = store(data.path());

    assert_eq!(reopened.load(Path::new("/work/project")), MemoryBook::default());
    assert_eq!(reopened.load(Path::new("/work/newer")).memories.len(), 1);
    assert_eq!(files(data.path()), ["-work-empty.json", "-work-old.json", "-work-project.json"]);
}

#[test]
fn two_programs_starting_at_once_import_the_memory_once() {
    let data = tempfile::tempdir().expect("data");
    let legacy = fixture(data.path());
    let start = Barrier::new(2);

    std::thread::scope(|scope| {
        for _ in 0..2 {
            scope.spawn(|| {
                start.wait();
                let kept = store(data.path());
                assert_eq!(kept.read(Path::new("/work/project")), Revised { revision: 1, book: legacy.clone() });
            });
        }
    });

    assert_eq!(files(data.path()), IMPORTED);
    assert_eq!(store(data.path()).load(Path::new("/work/old")).memories.len(), 2);
}

#[test]
fn a_memory_file_that_does_not_parse_is_kept_aside_while_the_others_are_imported() {
    let data = tempfile::tempdir().expect("data");
    let legacy = fixture(data.path());
    let broken = br#"{"next": 1, "memories": [{"id": 0, "#;
    std::fs::write(data.path().join("memory/local/-work-broken.json"), broken).expect("truncate");

    let kept = store(data.path());

    assert_eq!(kept.load(Path::new("/work/project")), legacy);
    assert_eq!(kept.load(Path::new("/work/broken")), MemoryBook::default());
    let names = files(data.path());
    let aside: Vec<&String> = names.iter().filter(|name| name.starts_with("-work-broken.json")).collect();
    assert_eq!(aside.len(), 1, "{names:?}");
    assert!(aside[0].starts_with("-work-broken.json.corrupt-"), "{names:?}");
    assert_eq!(std::fs::read(data.path().join("memory/local").join(aside[0])).expect("aside"), broken);
    assert_eq!(names.iter().filter(|name| name.ends_with(".imported")).count(), 3);
    kept.save(Path::new("/work/broken"), &legacy).expect("the project starts again from nothing");
}

#[test]
fn a_memory_file_repeating_an_id_keeps_the_first_memory_under_it() {
    let data = tempfile::tempdir().expect("data");
    std::fs::create_dir_all(data.path().join("memory/local")).expect("folder");
    let repeated = WITH_EMBEDDINGS.replace(r#""id": 1"#, r#""id": 0"#);
    std::fs::write(data.path().join("memory/local/-work-old.json"), repeated).expect("write");

    let kept = store(data.path()).load(Path::new("/work/old"));

    assert_eq!(kept.memories.iter().map(|memory| memory.text.as_str()).collect::<Vec<_>>(), ["Uses wgpu"]);
    assert_eq!(files(data.path()), ["-work-old.json.imported"]);
}
