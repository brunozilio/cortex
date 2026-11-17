use serde_json::json;

use super::*;

fn workspace() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::create_dir(dir.path().join("src")).expect("dir");
    std::fs::write(dir.path().join("src/a.rs"), "fn main() {\n    run();\n}\n").expect("write");
    std::fs::write(dir.path().join("src/b.md"), "Run it\nrun again\n").expect("write");
    let workspace = Workspace::new(dir.path());
    (dir, workspace)
}

fn built_in(input: &Value, workspace: &Workspace) -> Outcome {
    search(input, workspace, &Engine::BuiltIn)
}

#[test]
fn lists_files_counts_and_lines() {
    let (_dir, workspace) = workspace();
    let files: Vec<String> =
        built_in(&json!({"pattern": "run"}), &workspace).content.lines().map(|line| line.replace('\\', "/")).collect();
    assert_eq!(files, vec!["src/a.rs", "src/b.md"]);

    let count = built_in(&json!({"pattern": "run", "-i": true, "glob": "*.md", "output_mode": "count"}), &workspace);
    assert_eq!(count.content.replace('\\', "/"), "src/b.md:2");

    let content = built_in(&json!({"pattern": "run\\(", "output_mode": "content", "-n": true, "-B": 1}), &workspace);
    assert_eq!(content.content.replace('\\', "/"), "src/a.rs\n1-fn main() {\n2:    run();");
}

fn content_of(dir: &tempfile::TempDir, input: &Value) -> String {
    built_in(input, &Workspace::new(dir.path())).content
}

#[test]
fn matching_lines_are_grouped_under_their_file_in_path_order() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("b.txt"), "hit three\n").expect("write");
    std::fs::write(dir.path().join("a.txt"), "hit one\nmiss\nhit two\n").expect("write");
    let found = content_of(&dir, &json!({"pattern": "hit", "output_mode": "content", "-n": true}));
    assert_eq!(found, "a.txt\n1:hit one\n3:hit two\n\nb.txt\n1:hit three");
}

#[test]
fn a_long_line_is_cut_and_says_how_much_was_left_out() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("min.js"), format!("hit{}\n", "x".repeat(1_000))).expect("write");
    let found = content_of(&dir, &json!({"pattern": "hit", "output_mode": "content"}));
    assert_eq!(found, format!("min.js\nhit{}… [+703 characters]", "x".repeat(297)));
}

#[test]
fn one_file_shows_its_first_matches_and_counts_the_rest() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("log.txt"), "hit\n".repeat(30)).expect("write");
    let found = content_of(&dir, &json!({"pattern": "hit", "output_mode": "content"}));
    assert_eq!(found, format!("log.txt\n{}… [5 more matches in this file]", "hit\n".repeat(25)));
}

#[test]
fn results_stop_at_a_default_limit_that_head_limit_replaces() {
    let dir = tempfile::tempdir().expect("dir");
    for at in 0..205 {
        std::fs::write(dir.path().join(format!("f{at}.txt")), "hit\n").expect("write");
    }
    let listed = content_of(&dir, &json!({"pattern": "hit"}));
    assert_eq!(listed.lines().count(), 201);
    assert!(listed.ends_with("\n… [5 more results; narrow the search or raise head_limit]"), "{listed}");
    assert_eq!(content_of(&dir, &json!({"pattern": "hit", "head_limit": 300})).lines().count(), 205);
    let two = content_of(&dir, &json!({"pattern": "hit", "head_limit": 2}));
    assert!(two.ends_with("\n… [203 more results; narrow the search or raise head_limit]"), "{two}");
}

#[test]
fn reports_no_match_and_bad_patterns() {
    let (_dir, workspace) = workspace();
    assert_eq!(built_in(&json!({"pattern": "absent"}), &workspace).content, "No matches found.");
    assert!(built_in(&json!({"pattern": "("}), &workspace).is_error);
}

#[test]
fn a_multiline_match_spans_lines_and_shows_every_line_it_covers() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("a.rs"), "fn start() {\n    middle();\n}\nfn other() {}\n").expect("write");
    let spanning = json!({"pattern": "start.*\\n.*middle", "output_mode": "content", "-n": true});
    assert_eq!(content_of(&dir, &spanning), "No matches found.", "without multiline a match stays on its line");
    let mut multiline = spanning.clone();
    multiline["multiline"] = json!(true);
    assert_eq!(content_of(&dir, &multiline), "a.rs\n1:fn start() {\n2:    middle();");
    let dotted = json!({"pattern": "start.*\\}", "multiline": true, "output_mode": "count"});
    assert_eq!(content_of(&dir, &dotted), "a.rs:1", "a dot crosses lines and the match runs to the last brace");
    let per_line = json!({"pattern": "\\(\\)", "output_mode": "count"});
    assert_eq!(content_of(&dir, &per_line), "a.rs:3");
}

fn git(dir: &std::path::Path, args: &[&str]) {
    let done = std::process::Command::new("git").args(args).current_dir(dir).output().expect("git");
    assert!(done.status.success(), "git {args:?}");
}

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path();
    git(root, &["init", "-q"]);
    let files: [(&str, &[u8]); 15] = [
        (".gitignore", b"ignored.txt\nbuild/\n"),
        (".rgignore", b"skipped.txt\n"),
        ("ignored.txt", b"hit ignored\n"),
        ("skipped.txt", b"hit skipped\n"),
        ("build/out.txt", b"hit built\n"),
        (".hidden/secret.txt", b"hit hidden\nHIT loud\n"),
        (".env", b"TOKEN=hit\n"),
        ("src/a.rs", b"fn start() {\n    hit();\n}\n\nfn other() { hit(); hit(); }\n"),
        ("src/b.md", b"Hit it\nmiss\nmiss\nmiss\nhit again\n"),
        ("src/deep/c.rs", b"// HIT\nlet start = 1;\nlet end = 2;\n"),
        ("src/nul.dat", b"hit\0before a nul\n"),
        ("src/latin1.txt", b"hit \xe9t\xe9\n"),
        ("src/a-b.txt", b"hit dash\n"),
        ("a/z.txt", b"hit z\n"),
        ("big.log", b"one hit\ntwo\nthree hit\nfour\nfive\nsix hit\n"),
    ];
    for (name, bytes) in files {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
        std::fs::write(path, bytes).expect("write");
    }
    std::fs::create_dir_all(root.join(".git/info")).expect("info");
    std::fs::write(root.join(".git/info/exclude"), "excluded.txt\n").expect("exclude");
    std::fs::write(root.join("excluded.txt"), "hit excluded\n").expect("excluded");
    dir
}

fn questions() -> Vec<Value> {
    let mut asked = vec![
        json!({"pattern": "hit"}),
        json!({"pattern": "hit", "-i": true}),
        json!({"pattern": "hit", "glob": "*.rs"}),
        json!({"pattern": "hit", "glob": "src/**"}),
        json!({"pattern": "hit", "path": "src"}),
        json!({"pattern": "hit", "path": "src/b.md"}),
        json!({"pattern": "hit", "output_mode": "count"}),
        json!({"pattern": "hit", "output_mode": "count", "-i": true}),
        json!({"pattern": "hit", "output_mode": "content", "-n": true}),
        json!({"pattern": "hit", "output_mode": "content", "-C": 1}),
        json!({"pattern": "hit", "output_mode": "content", "-n": true, "-A": 2, "-B": 1}),
        json!({"pattern": "hit", "head_limit": 3}),
        json!({"pattern": "^fn \\w+", "output_mode": "content"}),
        json!({"pattern": "start.*\\}", "multiline": true}),
        json!({"pattern": "start.*\\}", "multiline": true, "output_mode": "count"}),
        json!({"pattern": "start\\s+let", "multiline": true, "output_mode": "content", "-n": true}),
        json!({"pattern": "start.*end", "multiline": true, "-i": true, "output_mode": "content"}),
        json!({"pattern": "absent"}),
        json!({"pattern": "befo\\x{0}"}),
        json!({"pattern": "a\\nb"}),
    ];
    asked.push(json!({"pattern": "nul", "output_mode": "content"}));
    asked
}

#[test]
fn the_built_in_search_skips_ignored_files_and_git_reads_hidden_ones_and_orders_by_path() {
    let dir = fixture();
    let found = content_of(&dir, &json!({"pattern": "hit", "-i": true}));
    let found: Vec<&str> = found.lines().collect();
    assert_eq!(
        found,
        [
            ".env",
            ".hidden/secret.txt",
            "a/z.txt",
            "big.log",
            "src/a-b.txt",
            "src/a.rs",
            "src/b.md",
            "src/deep/c.rs",
            "src/nul.dat"
        ]
    );
}

#[test]
fn ripgrep_and_the_built_in_search_give_the_same_answers() {
    let Some(rg) = ripgrep::program() else {
        eprintln!("rg is not on PATH: only the built-in search was checked");
        return;
    };
    let dir = fixture();
    let workspace = Workspace::new(dir.path());
    let asked = ripgrep::Asked { pattern: "hit", ignore_case: false, multiline: false };
    let listed = ripgrep::candidates(&rg, dir.path(), &asked, &workspace.stop).expect("ripgrep answered");
    let names: Vec<String> = listed.iter().map(|path| shown(path, &workspace).replace('\\', "/")).collect();
    assert!(names.contains(&"src/a.rs".to_owned()) && names.contains(&".env".to_owned()), "{names:?}");
    for left_out in ["ignored.txt", "skipped.txt", "excluded.txt", "build/out.txt"] {
        assert!(!names.contains(&left_out.to_owned()), "{left_out} in {names:?}");
    }
    for asked in questions() {
        let quick = search(&asked, &workspace, &Engine::Ripgrep(rg.clone()));
        let slow = built_in(&asked, &workspace);
        assert_eq!(quick, slow, "{asked}");
    }
}

#[test]
fn ripgrep_is_found_on_the_path_and_asked_for_the_files_that_could_match() {
    let dir = tempfile::tempdir().expect("dir");
    let name = if cfg!(windows) { "rg.exe" } else { "rg" };
    std::fs::write(dir.path().join(name), "").expect("rg");
    let path = std::env::join_paths([dir.path().join("missing"), dir.path().to_path_buf()]).expect("path");
    assert_eq!(ripgrep::on_path(Some(&path)), Some(dir.path().join(name)));
    assert_eq!(ripgrep::on_path(Some(dir.path().join("missing").as_os_str())), None);
    assert_eq!(ripgrep::on_path(None), None);

    let asked = ripgrep::Asked { pattern: "-x", ignore_case: true, multiline: true };
    let arguments: Vec<String> = ripgrep::arguments(std::path::Path::new("/repo"), &asked)
        .into_iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    assert!(arguments.starts_with(&["--no-config".to_owned(), "--files-with-matches".to_owned()]));
    for wanted in ["--hidden", "--text", "--ignore-case", "--multiline", "--multiline-dotall", "--glob=!.git"] {
        assert!(arguments.iter().any(|argument| argument == wanted), "{wanted}");
    }
    assert!(arguments.ends_with(&["--regexp".into(), "-x".into(), "--".into(), "/repo".into()]));
}

#[cfg(unix)]
#[test]
fn a_ripgrep_that_fails_leaves_the_search_to_the_built_in_engine() {
    use std::os::unix::fs::PermissionsExt;
    let dir = fixture();
    let fake = tempfile::tempdir().expect("fake");
    let program = fake.path().join("rg");
    std::fs::write(&program, "#!/bin/sh\nexit 2\n").expect("script");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("mode");
    let workspace = Workspace::new(dir.path());
    let asked = json!({"pattern": "hit", "path": "src"});
    assert_eq!(search(&asked, &workspace, &Engine::Ripgrep(program)), built_in(&asked, &workspace));
}
