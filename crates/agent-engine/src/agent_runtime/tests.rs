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
fn agent_memory_kept_before_the_owner_was_removed_stays_in_its_folder() {
    const KEPT_IN: &str = "b9de53edd9e0fc9b237d59bb5b213e9a6c4eebb83708b060a065f80a83f7443f/\
                           reviewer-2d70999ae1805e4bcef9b4ab3a4b827f578c61740f30076fcdc35c7ae7f586b3";
    let project = tempfile::tempdir().expect("project");
    let workspace = Workspace::new(project.path());
    let config = AgentConfig { memory: Some("project".into()), ..Default::default() };

    let setup = prepare(&config, "reviewer", &workspace).expect("setup");

    let dir = setup.memory_dir.expect("memory dir");
    let base = project.path().canonicalize().expect("project").join(concat!(brand::dir!(), "/agent-memory"));
    let kept: Vec<String> = dir
        .strip_prefix(&base)
        .expect("under the project's agent memory")
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    assert_eq!(kept.join("/"), KEPT_IN);
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
