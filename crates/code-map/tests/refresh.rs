use std::collections::BTreeMap;
use std::sync::Mutex;

use code_map::{CodeMap, FileStat, Source, refresh, refresh_with};

#[derive(Default)]
struct FakeSource {
    files: Mutex<BTreeMap<String, (i64, String)>>,
    reads: Mutex<Vec<String>>,
}

impl FakeSource {
    fn put(&self, path: &str, modified: i64, text: &str) {
        if let Ok(mut files) = self.files.lock() {
            files.insert(path.into(), (modified, text.into()));
        }
    }

    fn remove(&self, path: &str) {
        if let Ok(mut files) = self.files.lock() {
            files.remove(path);
        }
    }

    fn take_reads(&self) -> Vec<String> {
        let mut reads = self.reads.lock().map(|mut reads| std::mem::take(&mut *reads)).unwrap_or_default();
        reads.sort();
        reads
    }
}

impl Source for FakeSource {
    fn files(&self) -> Vec<FileStat> {
        let files = self.files.lock().map(|files| files.clone()).unwrap_or_default();
        files.into_iter().map(|(path, (modified, text))| FileStat { path, len: text.len() as u64, modified }).collect()
    }

    fn read(&self, path: &str) -> Option<String> {
        if let Ok(mut reads) = self.reads.lock() {
            reads.push(path.into());
        }
        self.files.lock().ok()?.get(path).map(|(_, text)| text.clone())
    }
}

fn names(map: &CodeMap) -> Vec<String> {
    map.files.iter().flat_map(|file| file.symbols.iter().map(|s| format!("{}:{}", file.path, s.name))).collect()
}

#[test]
fn refresh_reads_only_new_or_changed_files_and_drops_deleted_ones() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = dir.path().join("cache/code-map.json");
    let source = FakeSource::default();
    source.put("a.rs", 1, "fn alpha() {}");
    source.put("b.py", 1, "def beta(): pass");
    source.put("c.go", 1, "package c\nfunc Gamma() {}");

    let first = refresh_with(&source, &cache);
    assert_eq!(source.take_reads(), vec!["a.rs", "b.py", "c.go"]);
    assert_eq!(names(&first), vec!["a.rs:alpha", "b.py:beta", "c.go:Gamma"]);
    assert!(cache.is_file());

    let unchanged = refresh_with(&source, &cache);
    assert!(source.take_reads().is_empty());
    assert_eq!(unchanged, first);

    source.put("a.rs", 2, "fn alpha_two() {}");
    source.remove("b.py");
    source.put("d.ts", 1, "export function delta() {}");
    let changed = refresh_with(&source, &cache);
    assert_eq!(source.take_reads(), vec!["a.rs", "d.ts"]);
    assert_eq!(names(&changed), vec!["a.rs:alpha_two", "c.go:Gamma", "d.ts:delta"]);

    let again = refresh_with(&source, &cache);
    assert!(source.take_reads().is_empty());
    assert_eq!(again, changed);
}

#[test]
fn a_corrupt_or_missing_cache_rebuilds_without_panicking() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = dir.path().join("code-map.json");
    let source = FakeSource::default();
    source.put("a.rs", 1, "fn alpha() {}");
    std::fs::write(&cache, "{ not json").expect("write cache");

    let map = refresh_with(&source, &cache);
    assert_eq!(names(&map), vec!["a.rs:alpha"]);
    assert_eq!(source.take_reads(), vec!["a.rs"]);

    refresh_with(&source, &cache);
    assert!(source.take_reads().is_empty());

    std::fs::write(&cache, r#"{"version":999,"entries":[]}"#).expect("write cache");
    refresh_with(&source, &cache);
    assert_eq!(source.take_reads(), vec!["a.rs"]);
}

#[test]
fn an_unwritable_cache_still_returns_the_map() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("blocker"), "").expect("write blocker");
    let source = FakeSource::default();
    source.put("a.rs", 1, "fn alpha() {}");
    let map = refresh_with(&source, &dir.path().join("blocker/cache.json"));
    assert_eq!(names(&map), vec!["a.rs:alpha"]);
}

#[test]
fn refresh_walks_a_real_directory_respecting_gitignore_and_skipped_dirs() {
    let repo = tempfile::tempdir().expect("tempdir");
    let root = repo.path();
    let write = |path: &str, text: &str| {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("create dir");
        std::fs::write(full, text).expect("write file");
    };
    write(".gitignore", "generated/\n");
    write("src/lib.rs", "pub fn kept() { helper(); }\nfn helper() {}\n");
    write("web/app.tsx", "export function App() { return null; }\n");
    write("generated/out.rs", "fn ignored() {}\n");
    write("target/debug/build.rs", "fn built() {}\n");
    write("node_modules/pkg/index.js", "function vendored() {}\n");
    write("notes.md", "# notes\n");
    write("big.rs", &"fn big() {}\n".repeat(50_000));

    let scratch = tempfile::tempdir().expect("tempdir");
    let cache = scratch.path().join("map.json");
    let map = refresh(root, &cache);
    assert_eq!(names(&map), vec!["src/lib.rs:kept", "src/lib.rs:helper", "web/app.tsx:App"]);
    assert_eq!(map.files.iter().flat_map(|file| &file.calls).filter(|call| call.callee == "helper").count(), 1);

    write("src/lib.rs", "pub fn kept_changed() {}\n");
    let map = refresh(root, &cache);
    assert_eq!(names(&map), vec!["src/lib.rs:kept_changed", "web/app.tsx:App"]);
}

#[test]
#[ignore = "times a cold and a warm refresh of this workspace"]
fn times_refreshing_the_workspace() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cache = tempfile::tempdir().expect("tempdir");
    let cache = cache.path().join("workspace.json");
    let started = std::time::Instant::now();
    let cold = refresh(&root, &cache);
    let cold_time = started.elapsed();
    let started = std::time::Instant::now();
    let warm = refresh(&root, &cache);
    let warm_time = started.elapsed();
    let symbols: usize = warm.files.iter().map(|file| file.symbols.len()).sum();
    let calls: usize = warm.files.iter().map(|file| file.calls.len()).sum();
    let bytes = std::fs::metadata(&cache).map(|meta| meta.len()).unwrap_or(0);
    println!(
        "files {} symbols {symbols} calls {calls} cache {bytes} bytes; cold {cold_time:?} warm {warm_time:?}",
        cold.files.len()
    );
    let started = std::time::Instant::now();
    let block = code_map::render(&warm.relevant("why does merge_near_duplicates drop memories?", 12));
    println!("relevant+render {:?}\n{block}", started.elapsed());
    assert_eq!(cold, warm);
}
