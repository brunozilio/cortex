use serde_json::json;

use super::bash::Bash;
use crate::hostile_repository::Hostile;
use crate::tools::{Tool, Workspace};

const READS: [&str; 52] = [
    "git status",
    "git status --short",
    "git status -sb",
    "git status --porcelain",
    "git status --porcelain=v1 --untracked-files=all",
    "git status --ignored",
    "git log",
    "git log -p",
    "git log -1 --stat",
    "git log --oneline -5",
    "git log --graph --all --decorate",
    "git log --format='%h %an %s'",
    "git log --pretty=oneline",
    "git log -G two",
    "git log -S one",
    "git log --follow a.txt",
    "git diff",
    "git diff HEAD",
    "git diff --cached",
    "git diff --stat",
    "git diff --word-diff HEAD",
    "git diff HEAD~1 HEAD",
    "git show",
    "git show HEAD",
    "git show HEAD:a.txt",
    "git show --stat HEAD",
    "git show v1",
    "git blame a.txt",
    "git blame -L 1,1 a.txt",
    "git grep two",
    "git grep -n --cached one",
    "git ls-files",
    "git ls-files --others --exclude-standard",
    "git ls-files -s",
    "git rev-parse HEAD",
    "git rev-parse --show-toplevel",
    "git rev-parse --abbrev-ref HEAD",
    "git describe --always --dirty",
    "git describe --tags",
    "git shortlog -s HEAD",
    "git shortlog -sne HEAD",
    "git branch",
    "git branch -a -v",
    "git branch --list",
    "git branch --show-current",
    "git tag",
    "git tag -l",
    "git tag -n",
    "git remote",
    "git remote -v",
    "git status && git diff --stat && git log --oneline -3",
    "git log -1 | cat",
];

const ASKED_ABOUT_BUT_STILL_NEUTRAL: [&str; 8] = [
    "git log -1 --format=%GS",
    "git log -1 --format=%GK%GG%GF%GP%GT",
    "git log -1 --format='%G?'",
    "git show --pretty=format:%GK -s",
    "git log -1 --show-signature",
    "git show --show-signature HEAD",
    "git verify-commit HEAD",
    "git cat-file -p HEAD",
];

#[test]
fn every_git_form_the_policy_reads_without_asking_is_in_the_table() {
    let named: Vec<&str> = READS.iter().filter_map(|command| command.split_whitespace().nth(1)).collect();
    for reader in domain::command_policy::git_readers() {
        assert!(named.contains(&reader), "git {reader} is read without asking but not run against the fixture");
    }
    for command in READS {
        assert!(domain::command_policy::runs_as_a_read(command, None), "{command} would be asked about");
    }
    for command in &ASKED_ABOUT_BUT_STILL_NEUTRAL[..6] {
        assert!(!domain::command_policy::runs_as_a_read(command, None), "{command} checks a signature");
    }
}

#[test]
fn no_git_read_the_agent_runs_without_asking_runs_anything_a_maximally_hostile_repository_names() {
    let hostile = Hostile::new();
    let workspace = Workspace::new(hostile.path());

    let mut ran = Vec::new();
    for command in READS.iter().chain(&ASKED_ABOUT_BUT_STILL_NEUTRAL) {
        let _ = Bash.run(&json!({ "command": command }), &workspace);
        if let Some(what) = hostile.ran() {
            ran.push(format!("{command}: {what}"));
            hostile.forget();
        }
    }

    assert!(ran.is_empty(), "the repository's programs ran:\n{}", ran.join("\n"));
    assert!(hostile.plain_git_runs(&["status"]).is_some(), "plain git status does run them");
    assert!(hostile.plain_git_runs(&["log", "-1", "--format=%GS"]).is_some(), "plain git runs gpg.program");
}

#[test]
fn a_git_command_the_user_approved_keeps_the_project_s_hooks_but_not_its_signing_program() {
    let hostile = Hostile::new();
    let workspace = Workspace::new(hostile.path());

    let committed = Bash.run(
        &json!({ "command": "git -c user.name=t -c user.email=t@t commit -q --allow-empty --no-gpg-sign -m two" }),
        &workspace,
    );

    assert!(!committed.is_error, "{committed:?}");
    let ran = hostile.ran().unwrap_or_default();
    assert!(ran.contains("hooks/pre-commit") && ran.contains("hooks/post-commit"), "{ran}");
    hostile.forget();
    let signed = Bash.run(&json!({ "command": "git log -1 HEAD~1 --show-signature" }), &workspace);
    assert!(!signed.is_error, "{signed:?}");
    assert_eq!(hostile.ran(), None, "the repository's own gpg.program ran");
}
