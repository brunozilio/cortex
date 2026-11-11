use super::*;

fn environment() -> Environment {
    Environment {
        cwd: "/repo".into(),
        platform: "macOS",
        date: "2026-11-07".into(),
        model: "Qwen3.8 27B 8-bit".into(),
        ..Environment::default()
    }
}

#[test]
fn speaks_as_superfik_and_never_as_another_product() {
    let mut everything = environment();
    everything.tools = vec!["mcp__superfik_memory__memory_search".to_owned()];
    let prompt = system(&everything);
    assert!(prompt.starts_with("You are Superfik, an interactive agent"));
    for other in ["Claude", "Anthropic", "claude.ai", "1Password", "CLAUDE.md", "Sofik"] {
        assert!(!prompt.contains(other), "the prompt names {other}");
    }
}

#[test]
fn offers_the_sections_of_the_tools_the_run_has() {
    assert!(!system(&environment()).contains("# Memory"));
    let mut granted = environment();
    granted.tools = vec!["mcp__superfik_memory__memory_search".into()];
    assert!(system(&granted).contains("# Memory"));
}

#[test]
fn names_the_folders_added_to_the_project() {
    assert!(!system(&environment()).contains("Additional working directories"));
    let mut added = environment();
    added.added_dirs = vec!["/shared".into(), "/home/a/notes".into()];
    assert!(
        system(&added)
            .contains("- Working directory: /repo\n- Additional working directories: /shared, /home/a/notes\n")
    );
}

#[test]
fn says_where_it_runs_and_what_git_looked_like() {
    let mut environment = environment();
    environment.git =
        Some(GitSnapshot { branch: "feat/x".into(), status: " M src/a.rs\n".into(), recent: "abc123 fix: a\n".into() });
    let prompt = system(&environment);
    assert!(
        prompt.contains("- Working directory: /repo\n- Git repository: yes\n- Platform: macOS\n- Today: 2026-11-07")
    );
    assert!(prompt.contains("- Model: Qwen3.8 27B 8-bit"));
    assert!(prompt.contains("- Current branch: feat/x"));
    assert!(prompt.contains("Status:\n M src/a.rs\n"));
    assert!(prompt.contains("Recent commits:\nabc123 fix: a"));
}

#[test]
fn carries_each_instruction_file_with_who_wrote_it() {
    let mut environment = environment();
    environment.instructions = vec![
        Instructions { path: "/home/.superfik/SUPERFIK.md".into(), scope: Scope::User, text: "Be brief.\n".into() },
        Instructions { path: "/repo/SUPERFIK.md".into(), scope: Scope::Project, text: "Use tabs.".into() },
    ];
    let prompt = system(&environment);
    let user = prompt.find("Contents of /home/.superfik/SUPERFIK.md (the user's private").expect("user's");
    let project = prompt.find("Contents of /repo/SUPERFIK.md (the project's instructions").expect("project's");
    assert!(user < project);
    assert!(prompt.contains("kept in the repository):\n\nUse tabs."));
    assert!(prompt.contains("take precedence over any default behaviour"));
}

#[test]
fn finds_superfik_md_first_then_agents_md() {
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    std::fs::create_dir(home.path().join(".superfik")).expect("dir");
    std::fs::write(home.path().join(".superfik/SUPERFIK.md"), "mine").expect("write");
    std::fs::write(repo.path().join("SUPERFIK.md"), "project").expect("write");
    std::fs::write(repo.path().join("CLAUDE.md"), "not ours").expect("write");
    std::fs::write(repo.path().join("AGENTS.md"), "older").expect("write");
    std::fs::write(repo.path().join("SUPERFIK.local.md"), "local").expect("write");
    let found: Vec<(Scope, String)> =
        instructions(repo.path(), Some(home.path())).into_iter().map(|file| (file.scope, file.text)).collect();
    assert_eq!(
        found,
        vec![
            (Scope::User, "mine".into()),
            (Scope::Project, "project".into()),
            (Scope::Project, "older".into()),
            (Scope::Local, "local".into()),
        ]
    );
}

#[test]
fn reads_a_real_repository() {
    let repo = tempfile::tempdir().expect("repo");
    assert_eq!(snapshot(repo.path()), None, "not a repository");
    let git = |args: &[&str]| {
        let ok = superfik_process::command("git").arg("-C").arg(repo.path()).args(args).output().expect("git");
        assert!(ok.status.success(), "{args:?}: {}", String::from_utf8_lossy(&ok.stderr));
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["-c", "user.name=T", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "first"]);
    std::fs::write(repo.path().join("a.txt"), "a").expect("write");
    let found = snapshot(repo.path()).expect("snapshot");
    assert_eq!(found.branch, "main");
    assert!(found.status.contains("?? a.txt"), "{found:?}");
    assert!(found.recent.contains("first"));
}

#[test]
fn lists_the_skills_and_asks_for_the_ones_the_user_picked() {
    let mut environment = environment();
    environment.skills = "- release: Cuts a release.".into();
    assert!(system(&environment).contains("# Skills\nThese skills are available through the Skill tool."));
    assert!(
        system(&environment).contains("approach. Use only the names listed; do not guess.\n- release: Cuts a release.")
    );
    assert!(!system(&super::tests::environment()).contains("# Skills"));

    let model = superfik_core::ModelOption::new("m", "M");
    let request = PromptRequest::plain("/r".into(), "Ship it", model);
    assert_eq!(user_message(&request), "Ship it");
}

#[test]
fn instructions_load_ancestors_imports_once_and_keep_code_literals() {
    let repo = tempfile::tempdir().expect("repo");
    std::fs::create_dir(repo.path().join(".git")).expect("trust root");
    std::fs::create_dir_all(repo.path().join("docs")).expect("docs");
    std::fs::create_dir_all(repo.path().join("src/module")).expect("module");
    std::fs::write(repo.path().join("SUPERFIK.md"),"Root instructions\n@docs/guide.md\n<!-- private maintainer comment -->\n```md\n@not-an-import.md\n<!-- code literal -->\n```\n").expect("root");
    std::fs::write(repo.path().join("docs/guide.md"), "Imported guidance\n@../SUPERFIK.md\n").expect("import");
    std::fs::write(repo.path().join("src/SUPERFIK.local.md"), "Package-local guidance").expect("local");
    let files = instructions(&repo.path().join("src/module"), None);
    let text = files.iter().map(|f| f.text.as_str()).collect::<Vec<_>>().join("\n");
    assert_eq!(text.matches("Root instructions").count(), 1);
    assert_eq!(text.matches("Imported guidance").count(), 1);
    assert!(!text.contains("private maintainer comment"));
    assert!(text.contains("<!-- code literal -->"));
    assert!(text.find("Root instructions") < text.find("Package-local guidance"));
    assert!(!text.contains("not-an-import.md:"));
}
#[test]
fn project_import_cannot_read_a_sibling_private_file() {
    let root = tempfile::tempdir().expect("root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(repo.join(".git")).expect("repo");
    std::fs::write(root.path().join("private.md"), "PRIVATE-SENTINEL").expect("private fixture");
    std::fs::write(repo.join("SUPERFIK.md"), "@../private.md").expect("root instructions");
    let text = instructions(&repo, None).into_iter().map(|f| f.text).collect::<String>();
    assert!(!text.contains("PRIVATE-SENTINEL"));
    assert!(text.contains("outside the trusted directory"));
}

#[test]
fn the_turn_reminder_guides_planning_and_repeats_the_style() {
    use superfik_core::PermissionMode;
    let planning = reminder(PermissionMode::Plan, 0, Some("Be concise."));
    assert!(planning.contains("Planning mode is on.") && planning.contains("**Verification**"));
    assert!(planning.ends_with("\nBe concise."), "the style's reminder comes last");
    let sub_agent = reminder(PermissionMode::Plan, 1, Some("Be concise."));
    assert!(!sub_agent.contains("Planning mode is on.") && !sub_agent.contains("Be concise."));
    assert!(!reminder(PermissionMode::Ask, 0, None).contains("Planning"));
}
