use super::*;

fn plain(command: &str) -> CommandClass {
    classify(command, &GitFacts::default())
}

#[test]
fn commands_that_only_look_inside_the_project_are_read_only() {
    for command in [
        "ls -la",
        "git status",
        "git log --oneline -5",
        "git diff HEAD~1 -- src/lib.rs",
        "cat src/main.rs | grep fn | wc -l",
        "rg -n 'todo' crates && git branch -a",
        "find . -name '*.rs'",
        "pwd; which cargo",
    ] {
        assert_eq!(plain(command), CommandClass::ReadOnly, "{command}");
    }
}

#[test]
fn writing_reaching_outside_or_hiding_a_command_is_left_to_the_usual_checks() {
    for command in [
        "ls > listing.txt",
        "cat /etc/hosts",
        "cat ~/notes.md",
        "cat ../other/README.md",
        "find . -name '*.tmp' -delete",
        "find . -exec rm {} ;",
        "git branch -D old",
        "git log --output=log.txt",
        "ls $(pwd)",
        "cat `ls`",
        "sort -o out.txt in.txt",
        "cargo build",
        "ls && touch made",
        "grep 'unclosed",
        "",
    ] {
        assert_eq!(plain(command), CommandClass::Unknown, "{command}");
    }
}

#[test]
fn printing_secrets_is_refused_whatever_else_the_command_does() {
    for command in [
        "cat .env",
        "head -5 config/.env.production",
        "grep KEY .env.local",
        "base64 ~/.ssh/id_ed25519",
        "cargo build && cat secrets/server.pem",
        "printenv",
        "env",
        "printenv OPENAI_API_KEY",
        "cp .npmrc leaked.txt",
    ] {
        assert!(matches!(plain(command), CommandClass::Forbidden(_)), "{command}: {:?}", plain(command));
    }
    for command in ["ls .env", "ls -a", "env CARGO_TERM_COLOR=always cargo test", "cat .env.example", "git status"] {
        assert!(!matches!(plain(command), CommandClass::Forbidden(_)), "{command}");
    }
}

fn on(branch: &str) -> GitFacts<'_> {
    GitFacts { current_branch: Some(branch), default_branch: Some("main") }
}

fn risky(command: &str, git: &GitFacts<'_>) -> bool {
    matches!(classify(command, git), CommandClass::Risky(_))
}

#[test]
fn rewriting_or_deleting_shared_history_is_always_asked_about() {
    let feature = on("feature");
    for command in [
        "git push --force",
        "git push -f origin feature",
        "git push --force-with-lease",
        "git push origin +feature",
        "git push origin --delete old",
        "git push origin :old",
        "git push --mirror",
        "cargo test && git push -f",
    ] {
        assert!(risky(command, &feature), "{command}");
    }
    assert!(!risky("git push", &feature));
    assert!(!risky("git push -u origin feature", &feature));
}

#[test]
fn pushing_to_the_default_branch_is_asked_about() {
    assert!(risky("git push", &on("main")));
    assert!(risky("git push origin main", &on("feature")));
    assert!(risky("git push origin HEAD:main", &on("feature")));
    assert!(!risky("git push origin feature", &on("feature")));
    let unknown = GitFacts { current_branch: Some("master"), default_branch: None };
    assert!(risky("git push", &unknown), "main and master count as default when git does not say");
}

#[test]
fn publishing_and_merging_are_asked_about() {
    for command in [
        "cargo publish",
        "npm publish --access public",
        "gh pr merge 12 --squash",
        "gh release create v1.0",
        "gh api -X DELETE repos/o/r",
        "docker push org/image:latest",
    ] {
        assert!(risky(command, &GitFacts::default()), "{command}");
    }
    assert!(!risky("gh pr view 12", &GitFacts::default()));
}
