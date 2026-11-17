#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::fs::File;
use std::path::Path;
use std::time::{Duration, SystemTime};

use super::*;

fn written(path: &Path, seconds_ago: u64) {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).expect("folder");
    }
    std::fs::write(path, "x").expect("file");
    let at = SystemTime::now() - Duration::from_secs(seconds_ago);
    File::options().write(true).open(path).expect("open").set_modified(at).expect("time");
}

#[test]
fn a_release_without_the_built_page_is_refused() {
    let web = tempfile::tempdir().expect("web");
    written(&web.path().join("src/main.tsx"), 60);

    let problem = release_problem(web.path()).expect("checked").expect("a problem");

    assert!(problem.contains("dist/index.html is missing"), "{problem}");
}

#[test]
fn a_release_whose_page_is_older_than_a_source_or_a_doc_is_refused() {
    for changed in ["src/lib/live.ts", "docs/en/hooks.md", "package.json"] {
        let web = tempfile::tempdir().expect("web");
        written(&web.path().join("src/main.tsx"), 300);
        written(&web.path().join("dist/index.html"), 200);
        written(&web.path().join(changed), 100);

        let problem = release_problem(web.path()).expect("checked").expect("a problem");

        assert!(problem.contains("stale page") && problem.contains(changed), "{problem}");
    }
}

#[test]
fn a_page_built_after_every_source_passes_and_other_folders_do_not_count() {
    let web = tempfile::tempdir().expect("web");
    written(&web.path().join("src/main.tsx"), 300);
    written(&web.path().join("docs/en/hooks.md"), 250);
    written(&web.path().join("dist/index.html"), 200);
    written(&web.path().join("node_modules/pkg/index.js"), 10);
    written(&web.path().join("test/report.test.ts"), 10);

    assert_eq!(release_problem(web.path()).expect("checked"), None);
}

#[test]
fn a_missing_bun_fails_and_says_where_to_get_it() {
    let web = tempfile::tempdir().expect("web");
    written(&web.path().join("package.json"), 1);

    let failure = built_with("no-such-bun-program", web.path()).expect_err("no bun");

    let told = failure.to_string();
    assert!(
        told.contains("no-such-bun-program install --frozen-lockfile") && told.contains("https://bun.sh"),
        "{told}"
    );
    assert!(!web.path().join("dist").exists());
}

#[test]
fn a_folder_that_is_not_the_page_is_refused_before_anything_runs() {
    let empty = tempfile::tempdir().expect("web");
    let told = built_with("no-such-bun-program", empty.path()).expect_err("no package").to_string();
    assert!(told.contains("has no package.json"), "{told}");
}

#[test]
fn the_page_s_build_script_watches_the_same_sources_as_the_release_check() {
    let script = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/page/build.rs"))
        .expect("the page's build script");
    for source in SOURCES {
        assert!(script.contains(&format!("\"{source}\"")), "crates/page/build.rs does not watch {source}");
    }
}
