use std::path::Path;

use super::*;

fn repo(dir: &Path) {
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .expect("git");
        assert!(status.success(), "git {args:?}");
    };
    git(&["init", "-b", "main"]);
    std::fs::write(dir.join("README.md"), "base").expect("readme");
    git(&["add", "README.md"]);
    git(&["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-m", "base"]);
}

fn home_with_a_rule() -> tempfile::TempDir {
    let home = tempfile::tempdir().expect("home");
    let rules = home.path().join(concat!(brand::dir!(), "/rules"));
    std::fs::create_dir_all(&rules).expect("rules");
    std::fs::write(rules.join("mine.md"), "Always answer in Portuguese.").expect("rule");
    home
}

fn knows_the_home_rule(setup: &Setup) -> bool {
    let brief = setup.context.as_ref().expect("the worktree has a brief");
    brief.rules.iter().any(|rule| rule.text == "Always answer in Portuguese.")
}

#[test]
fn a_worktree_keeps_the_rules_of_the_user_s_home_when_made_and_when_resumed() {
    let project = tempfile::tempdir().expect("project");
    repo(project.path());
    let home = home_with_a_rule();
    let mut workspace = Workspace::new(project.path());
    workspace.home = Some(home.path().to_path_buf());
    let config = AgentConfig { worktree: true, ..Default::default() };

    let made = prepare(&config, "main", &workspace).expect("worktree");
    assert!(made.worktree_root.is_some(), "the run is isolated");
    assert!(knows_the_home_rule(&made), "a new worktree reads the user's home");

    let state = made.worktree_state().expect("state");
    let resumed = resume(&config, "main", &state, &workspace).expect("resumed");
    assert_eq!(resumed.cwd.canonicalize().expect("cwd"), state.cwd.canonicalize().expect("cwd"));
    assert!(knows_the_home_rule(&resumed), "a resumed worktree reads the user's home");
}
