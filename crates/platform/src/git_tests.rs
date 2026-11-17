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
    signed_commit_on_top(dir);
    let (signer, signed) = marker_script(outside, "gpg");
    markers.push(signed);
    for hook in
        ["pre-commit", "post-commit", "post-checkout", "commit-msg", "reference-transaction", "post-index-change"]
    {
        let (script, marker) = marker_script(outside, hook);
        std::fs::copy(&script, hooks.join(hook)).expect("hook");
        markers.push(marker);
    }
    let config = format!(
        "[core]\n\tfsmonitor = {}\n[filter \"evil\"]\n\tclean = {}\n\tsmudge = {}\n\trequired = true\n[diff \
         \"evil\"]\n\ttextconv = {}\n[diff]\n\texternal = {}\n[gpg]\n\tprogram = {5}\n[gpg \"ssh\"]\n\tprogram = \
         {5}\n[gpg \"x509\"]\n\tprogram = {5}\n[log]\n\tshowSignature = true\n",
        monitor.display(),
        filter.display(),
        filter.display(),
        textconv.display(),
        external.display(),
        signer.display()
    );
    let mut text = std::fs::read_to_string(dir.join(".git/config")).expect("config");
    text.push_str(&config);
    std::fs::write(dir.join(".git/config"), text).expect("config");
    std::fs::write(dir.join(".gitattributes"), "* filter=evil diff=evil\n").expect("attributes");
    std::thread::sleep(std::time::Duration::from_millis(1100));
    std::fs::write(dir.join("a.txt"), "two\n").expect("edit");
    Hostile { repo, _scripts: scripts, markers }
}

fn signed_commit_on_top(dir: &Path) {
    let text = |args: &[&str]| {
        let done = Command::new("git").args(args).current_dir(dir).output().expect("git");
        String::from_utf8_lossy(&done.stdout).trim().to_owned()
    };
    let (tree, parent) = (text(&["rev-parse", "HEAD^{tree}"]), text(&["rev-parse", "HEAD"]));
    let body = format!(
        "tree {tree}\nparent {parent}\nauthor t <t@t> 1700000000 +0000\ncommitter t <t@t> 1700000000 +0000\ngpgsig \
         -----BEGIN PGP SIGNATURE-----\n \n iQEz\n -----END PGP SIGNATURE-----\n\nsigned\n"
    );
    let object = dir.join(".git/signed-commit");
    std::fs::write(&object, body).expect("commit");
    let commit = text(&["hash-object", "-t", "commit", "-w", &object.to_string_lossy()]);
    std::fs::remove_file(&object).expect("cleaned");
    setup(dir, &["-c", "core.hooksPath=/dev/null", "update-ref", "HEAD", &commit]);
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
        &["log", "-1", "--format=%GS %G?"],
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
                  system\tfilter.sys.smudge\nlocal\tfilter.evil.smudge\ncommand\tfilter.x.clean\n\
                  local\tdiff.pdf.textconv\nglobal\tdiff.mine.textconv\nworktree\tdiff.tool.command\n\
                  local\tdiff.external\nglobal\tdiff.external\nglobal\tgpg.program\nlocal\tgpg.ssh.program\n\
                  worktree\tgpg.x509.program\nlocal\tgpg.ssh.defaultkeycommand\n";
    let driver = |section, name: &str| Driver { section, name: name.to_owned() };
    assert_eq!(
        drivers_in(listed),
        BTreeSet::from([
            driver("filter", "evil"),
            driver("filter", "odd.name"),
            driver("diff", "diff.pdf.textconv"),
            driver("diff", "diff.tool.command"),
            driver("diff", "diff.external"),
            driver("gpg", "gpg.ssh.program"),
            driver("gpg", "gpg.x509.program"),
            driver("gpg", "gpg.ssh.defaultKeyCommand"),
        ])
    );
    let harmless: Vec<String> = drivers_in(listed).iter().flat_map(emptied).collect();
    assert!(harmless.contains(&"diff.pdf.textconv=cat".to_owned()), "{harmless:?}");
    assert!(harmless.contains(&"gpg.ssh.program=false".to_owned()), "{harmless:?}");
    assert!(!harmless.iter().any(|setting| setting.starts_with("gpg.program")), "the user's own signer stays");
    assert!(harmless.contains(&format!("diff.external={PLAIN_DIFF}")), "{harmless:?}");
    assert!(drivers_in("").is_empty());
}

#[test]
fn the_agent_s_own_git_reads_run_nothing_the_repository_names() {
    let hostile = hostile_repository();
    let dir = hostile.repo.path();
    let hardening = environment(dir, None, Run::ReadOnly);

    for args in [
        &["status"][..],
        &["status", "--short"],
        &["diff"],
        &["diff", "HEAD"],
        &["diff", "--stat"],
        &["log", "-p"],
        &["log", "-1", "--stat"],
        &["log", "-1", "--format=%GS %GK %G?"],
        &["show"],
        &["show", "--pretty=format:%GK", "-s"],
        &["show", "HEAD:a.txt"],
        &["blame", "a.txt"],
        &["grep", "two"],
    ] {
        let done = Command::new("git").args(args).envs(hardening.clone()).current_dir(dir).output().expect("git");
        assert!(done.status.success() || args[0] == "grep", "{args:?}: {}", String::from_utf8_lossy(&done.stderr));
    }

    assert_eq!(hostile.ran(), Vec::<&PathBuf>::new(), "nothing the repository named ran");
    let _ = Command::new("git").args(["diff", "HEAD"]).current_dir(dir).output().expect("plain git");
    assert!(!hostile.ran().is_empty(), "plain git does run them");
}

#[test]
fn an_approved_git_command_keeps_the_hooks_but_never_runs_the_repository_s_own_signing_program() {
    let hostile = hostile_repository();
    let dir = hostile.repo.path();
    let approved = environment(dir, None, Run::Approved);

    let mut commit = Command::new("git");
    commit.args(IDENTITY).args(["commit", "-q", "--allow-empty", "-m", "two"]).envs(approved.clone());
    assert!(commit.current_dir(dir).status().expect("git").success());
    let verified = Command::new("git").args(["log", "-2", "--show-signature"]).envs(approved).current_dir(dir).output();
    assert!(verified.expect("git").status.success());

    let ran: Vec<String> =
        hostile.ran().iter().filter_map(|marker| marker.file_stem()?.to_str().map(str::to_owned)).collect();
    assert!(ran.iter().any(|name| name == "pre-commit"), "the project's hooks still run: {ran:?}");
    assert!(!ran.iter().any(|name| name == "gpg"), "the repository's gpg.program ran: {ran:?}");
}

#[test]
fn a_read_turns_hooks_signing_programs_and_optional_locks_off() {
    let dir = tempfile::tempdir().expect("dir");
    let read = environment(dir.path(), None, Run::ReadOnly);
    let value_of = |key: &str| {
        let at = read.iter().find(|(name, value)| name.starts_with("GIT_CONFIG_KEY_") && value == key)?;
        let index = at.0.trim_start_matches("GIT_CONFIG_KEY_");
        read.iter().find(|(name, _)| *name == format!("GIT_CONFIG_VALUE_{index}")).map(|(_, value)| value.clone())
    };
    assert_eq!(value_of("core.hooksPath").as_deref(), Some(NOWHERE));
    for program in ["gpg.program", "gpg.ssh.program", "gpg.x509.program"] {
        assert_eq!(value_of(program).as_deref(), Some("false"), "{program}");
    }
    assert!(read.contains(&("GIT_OPTIONAL_LOCKS".to_owned(), "0".to_owned())), "{read:?}");
    let approved = environment(dir.path(), None, Run::Approved);
    assert!(!approved.iter().any(|(_, value)| value == "core.hooksPath" || value == "gpg.program"), "{approved:?}");
    assert!(!approved.iter().any(|(name, _)| name == "GIT_OPTIONAL_LOCKS"), "{approved:?}");
}

#[test]
fn the_agent_s_git_settings_come_after_the_ones_it_inherited() {
    let dir = tempfile::tempdir().expect("dir");
    let fresh = environment(dir.path(), None, Run::Approved);
    assert!(fresh.contains(&("GIT_CONFIG_KEY_0".to_owned(), "core.fsmonitor".to_owned())), "{fresh:?}");
    assert!(fresh.contains(&("GIT_CONFIG_KEY_4".to_owned(), "log.showSignature".to_owned())), "{fresh:?}");
    assert_eq!(fresh.last(), Some(&("GIT_CONFIG_COUNT".to_owned(), "5".to_owned())));
    let after = environment(dir.path(), Some("2"), Run::Approved);
    assert!(after.contains(&("GIT_CONFIG_KEY_2".to_owned(), "core.fsmonitor".to_owned())), "{after:?}");
    assert_eq!(after.last(), Some(&("GIT_CONFIG_COUNT".to_owned(), "7".to_owned())));
    let read = environment(dir.path(), Some("2"), Run::ReadOnly);
    assert_eq!(read.last(), Some(&("GIT_CONFIG_COUNT".to_owned(), "11".to_owned())));
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
