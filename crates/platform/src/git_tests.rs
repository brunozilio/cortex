use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::*;

const IDENTITY: [&str; 6] = ["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"];

fn setup(dir: &Path, args: &[&str]) {
    let done = Command::new("git").args(IDENTITY).args(args).current_dir(dir).output().expect("git");
    assert!(done.status.success(), "{args:?}: {}", String::from_utf8_lossy(&done.stderr));
}

fn marker_script(dir: &Path, name: &str) -> (PathBuf, PathBuf) {
    let marker = dir.join(format!("{name}.ran"));
    let script = dir.join(format!("{name}.sh"));
    std::fs::write(&script, format!("#!/bin/sh\ntouch '{}'\ncat\n", marker.display())).expect("script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("mode");
    (script, marker)
}

struct Hostile {
    repo: tempfile::TempDir,
    _scripts: tempfile::TempDir,
    markers: Vec<PathBuf>,
}

fn hostile_repository() -> Hostile {
    let repo = tempfile::tempdir().expect("repo");
    let dir = repo.path();
    setup(dir, &["init", "-q", "-b", "main"]);
    std::fs::write(dir.join("a.txt"), "one\n").expect("file");
    setup(dir, &["add", "a.txt"]);
    setup(dir, &["commit", "-q", "-m", "one"]);
    let scripts = tempfile::tempdir().expect("scripts");
    let outside = scripts.path();
    let (monitor, monitored) = marker_script(outside, "fsmonitor");
    let (filter, filtered) = marker_script(outside, "filter");
    let (textconv, converted) = marker_script(outside, "textconv");
    let (external, diffed) = marker_script(outside, "external");
    let hooks = dir.join(".git/hooks");
    let mut markers = vec![monitored, filtered, converted, diffed];
    for hook in ["pre-commit", "post-commit", "post-checkout", "commit-msg", "reference-transaction"] {
        let (script, marker) = marker_script(outside, hook);
        std::fs::copy(&script, hooks.join(hook)).expect("hook");
        markers.push(marker);
    }
    let config = format!(
        "[core]\n\tfsmonitor = {}\n[filter \"evil\"]\n\tclean = {}\n\tsmudge = {}\n\trequired = true\n[diff \
         \"evil\"]\n\ttextconv = {}\n[diff]\n\texternal = {}\n",
        monitor.display(),
        filter.display(),
        filter.display(),
        textconv.display(),
        external.display()
    );
    let mut text = std::fs::read_to_string(dir.join(".git/config")).expect("config");
    text.push_str(&config);
    std::fs::write(dir.join(".git/config"), text).expect("config");
    std::fs::write(dir.join(".gitattributes"), "* filter=evil diff=evil\n").expect("attributes");
    std::fs::write(dir.join("a.txt"), "two\n").expect("edit");
    Hostile { repo, _scripts: scripts, markers }
}

impl Hostile {
    fn ran(&self) -> Vec<&PathBuf> {
        self.markers.iter().filter(|marker| marker.exists()).collect()
    }
}

fn run(dir: &Path, args: &[&str]) -> std::process::Output {
    let mut git = command(dir);
    git.args(IDENTITY).args(args).output().expect("git")
}

#[test]
fn a_repository_s_own_config_runs_nothing_through_the_app_s_git() {
    let hostile = hostile_repository();
    let dir = hostile.repo.path();

    for args in [
        &["status", "--porcelain=v1", "--untracked-files=all"][..],
        &["diff", "--no-ext-diff", "--no-textconv", "HEAD"],
        &["diff", "--name-only", "HEAD"],
        &["log", "--oneline", "-5"],
        &["rev-parse", "--show-toplevel"],
        &["show", "HEAD:a.txt"],
        &["ls-files", "--others", "--exclude-standard"],
    ] {
        let done = run(dir, args);
        assert!(done.status.success(), "{args:?}: {}", String::from_utf8_lossy(&done.stderr));
    }
    assert!(String::from_utf8_lossy(&run(dir, &["status", "--short"]).stdout).contains("a.txt"));
    let worktree = dir.join("elsewhere");
    let added = run(dir, &["worktree", "add", "-q", "--detach", &worktree.to_string_lossy(), "HEAD"]);
    assert!(added.status.success(), "{}", String::from_utf8_lossy(&added.stderr));
    assert!(run(dir, &["commit", "-q", "--allow-empty", "-m", "two"]).status.success());

    assert_eq!(hostile.ran(), Vec::<&PathBuf>::new(), "nothing the repository named ran");

    for args in [&["status"][..], &["diff", "HEAD"], &["commit", "-q", "--allow-empty", "-m", "three"]] {
        let _ = Command::new("git").args(IDENTITY).args(args).current_dir(dir).output().expect("plain git");
    }
    let ran: Vec<String> =
        hostile.ran().iter().filter_map(|marker| marker.file_stem()?.to_str().map(str::to_owned)).collect();
    for expected in ["fsmonitor", "filter", "external", "pre-commit", "post-commit"] {
        assert!(ran.iter().any(|name| name == expected), "plain git runs {expected}: {ran:?}");
    }
}

#[test]
fn a_submodule_s_own_config_runs_nothing_either() {
    let root = tempfile::tempdir().expect("root");
    let (inner, outer) = (root.path().join("inner"), root.path().join("outer"));
    std::fs::create_dir_all(&inner).expect("inner");
    std::fs::create_dir_all(&outer).expect("outer");
    setup(&inner, &["init", "-q", "-b", "main"]);
    std::fs::write(inner.join("f"), "a\n").expect("file");
    setup(&inner, &["add", "f"]);
    setup(&inner, &["commit", "-q", "-m", "one"]);
    setup(&outer, &["init", "-q", "-b", "main"]);
    setup(&outer, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", "../inner", "inner"]);
    setup(&outer, &["commit", "-q", "-m", "sub"]);
    let (filter, filtered) = marker_script(root.path(), "filter");
    let config = outer.join(".git/modules/inner/config");
    let mut text = std::fs::read_to_string(&config).expect("config");
    text.push_str(&format!("[filter \"evil\"]\n\tclean = {}\n", filter.display()));
    std::fs::write(&config, text).expect("config");
    std::fs::write(outer.join("inner/.gitattributes"), "* filter=evil\n").expect("attributes");
    std::fs::write(outer.join("inner/f"), "b\n").expect("edit");

    for args in [&["status", "--porcelain=v1"][..], &["diff", "--no-ext-diff", "--no-textconv", "HEAD"]] {
        assert!(run(&outer, args).status.success(), "{args:?}");
    }

    assert!(!filtered.exists(), "the submodule's filter ran");
    let _ = Command::new("git").arg("status").current_dir(&outer).output().expect("plain git");
    assert!(filtered.exists(), "plain git does run it");
}

#[test]
fn a_filter_the_user_set_up_outside_the_repository_still_applies() {
    let listed = "global\tfilter.lfs.clean\nlocal\tfilter.evil.clean\nworktree\tfilter.odd.name.process\n\
                  system\tfilter.sys.smudge\nlocal\tfilter.evil.smudge\ncommand\tfilter.x.clean\n";
    assert_eq!(drivers_in(listed), BTreeSet::from(["evil".to_owned(), "odd.name".to_owned()]));
    assert!(drivers_in("").is_empty());
}

#[test]
fn git_cannot_prompt_page_or_take_optional_locks() {
    let dir = tempfile::tempdir().expect("dir");
    let git = command(dir.path());
    let envs: Vec<_> = git.get_envs().map(|(key, value)| (key.to_owned(), value.map(ToOwned::to_owned))).collect();
    assert!(envs.contains(&("GIT_TERMINAL_PROMPT".into(), Some("0".into()))), "{envs:?}");
    let args: Vec<String> = git.get_args().map(|arg| arg.to_string_lossy().into_owned()).collect();
    for wanted in ["--no-pager", "--no-optional-locks", "core.fsmonitor=false", "core.hooksPath=/dev/null"] {
        assert!(args.iter().any(|arg| arg == wanted), "{wanted} in {args:?}");
    }
    assert_eq!(git.get_current_dir(), Some(dir.path()));
}
