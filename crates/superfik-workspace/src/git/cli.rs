//! [`GitInspector`] built from a handful of read-only `git` commands.

use std::path::{Path, PathBuf};

use superfik_core::Repository;
use superfik_core::project::ProjectRef;

use super::GitInspector;
use super::runner::{GitRunner, SystemGit};
use super::slug::remote_slug;
use crate::error::WorkspaceError;

#[derive(Debug, Clone, Default)]
pub struct GitCli<R = SystemGit> {
    runner: R,
}

impl GitCli<SystemGit> {
    pub fn system() -> Self {
        Self::default()
    }
}

impl<R: GitRunner> GitCli<R> {
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    pub(crate) fn runner(&self) -> &R {
        &self.runner
    }

    fn optional(&self, root: &Path, args: &[&str]) -> Option<String> {
        self.runner.run(root, args).ok().map(|out| out.trim().to_owned()).filter(|out| !out.is_empty())
    }
}

impl<R: GitRunner> GitInspector for GitCli<R> {
    fn inspect(&self, root: &Path) -> Result<Repository, WorkspaceError> {
        let toplevel = self
            .runner
            .run(root, &["rev-parse", "--show-toplevel"])
            .map_err(|_| WorkspaceError::NotARepository(root.to_path_buf()))?;
        let root = native_path(std::env::consts::OS, toplevel.trim());
        let remote = self.optional(&root, &["config", "--get", "remote.origin.url"]);
        let head = self.optional(&root, &["log", "-1", "--format=%h%x00%s"]).unwrap_or_default();
        let (head_short_sha, head_subject) = parse_head(&head);
        let branch = self.optional(&root, &["symbolic-ref", "--short", "-q", "HEAD"]);
        let branches = parse_lines(
            &self.optional(&root, &["for-each-ref", "--format=%(refname:short)", "refs/heads"]).unwrap_or_default(),
        );
        let remote_branches = parse_remote_branches(
            &self.optional(&root, &["for-each-ref", "--format=%(refname)", REMOTE_REFS]).unwrap_or_default(),
            &branches,
        );
        Ok(Repository {
            slug: repository_slug(remote.as_deref(), &root),
            project: Some(ProjectRef::of(remote.as_deref(), root.clone())),
            branch: branch.unwrap_or_else(|| detached_label(&head_short_sha)),
            root,
            head_short_sha,
            head_subject,
            branches,
            remote_branches,
            pull_request: None,
        })
    }
}

const REMOTE_REFS: &str = "refs/remotes/origin";

pub(crate) fn parse_remote_branches(output: &str, local: &[String]) -> Vec<String> {
    let prefix = format!("{REMOTE_REFS}/");
    parse_lines(output)
        .into_iter()
        .filter_map(|line| line.strip_prefix(&prefix).map(str::to_owned))
        .filter(|name| name != "HEAD" && !local.contains(name))
        .collect()
}

pub(crate) fn repository_slug(remote: Option<&str>, root: &Path) -> String {
    remote.and_then(remote_slug).unwrap_or_else(|| {
        root.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| root.display().to_string())
    })
}

pub(crate) fn parse_head(output: &str) -> (String, String) {
    let output = output.trim();
    match output.split_once('\0') {
        Some((sha, subject)) => (sha.trim().to_owned(), subject.trim().to_owned()),
        None => (output.to_owned(), String::new()),
    }
}

pub(crate) fn parse_lines(output: &str) -> Vec<String> {
    output.lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_owned).collect()
}

fn detached_label(sha: &str) -> String {
    if sha.is_empty() { "HEAD".to_owned() } else { format!("detached@{sha}") }
}

pub(crate) fn native_path(os: &str, printed: &str) -> PathBuf {
    if os == "windows" { PathBuf::from(printed.replace('/', "\\")) } else { PathBuf::from(printed) }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn gits_windows_paths_take_backslashes() {
        assert_eq!(native_path("windows", "C:/Users/ana/repo"), PathBuf::from(r"C:\Users\ana\repo"));
        assert_eq!(native_path("macos", "/Users/ana/repo"), PathBuf::from("/Users/ana/repo"));
    }

    struct ScriptedGit(HashMap<&'static str, &'static str>);

    impl GitRunner for ScriptedGit {
        fn run(&self, _cwd: &Path, args: &[&str]) -> Result<String, WorkspaceError> {
            self.0
                .get(args.join(" ").as_str())
                .map(|out| (*out).to_owned())
                .ok_or_else(|| WorkspaceError::Git { command: args.join(" "), message: "fatal".into() })
        }
    }

    fn git(pairs: &[(&'static str, &'static str)]) -> GitCli<ScriptedGit> {
        GitCli::new(ScriptedGit(pairs.iter().copied().collect()))
    }

    #[test]
    fn full_repository() {
        let repo = git(&[
            ("rev-parse --show-toplevel", "/work/dither-minis\n"),
            ("config --get remote.origin.url", "git@github.com:zzzzshawn/dither-minis.git\n"),
            ("log -1 --format=%h%x00%s", "a1f3c2e\0Tighten the outline\n"),
            ("symbolic-ref --short -q HEAD", "main\n"),
            ("for-each-ref --format=%(refname:short) refs/heads", "dev\nmain\n"),
            (
                "for-each-ref --format=%(refname) refs/remotes/origin",
                "refs/remotes/origin/HEAD\nrefs/remotes/origin/main\nrefs/remotes/origin/spike\n",
            ),
        ])
        .inspect(Path::new("/work/dither-minis/src"))
        .expect("repo");
        assert_eq!(repo.root, PathBuf::from("/work/dither-minis"));
        assert_eq!(repo.slug, "zzzzshawn/dither-minis");
        assert_eq!(repo.project, Some(ProjectRef::Remote("github.com/zzzzshawn/dither-minis".into())));
        assert_eq!(repo.head_caption(), "a1f3c2e · Tighten the outline");
        assert_eq!(repo.branch, "main");
        assert_eq!(repo.branches, vec!["dev", "main"]);
        assert_eq!(repo.remote_branches, vec!["spike"]);
        assert_eq!(
            parse_remote_branches(
                "refs/remotes/origin/a/b\nrefs/remotes/upstream/c\nrefs/remotes/origin/dev\n",
                &repo.branches
            ),
            vec!["a/b"]
        );
    }

    #[test]
    fn fresh_repository_without_remote_or_commits() {
        let repo = git(&[("rev-parse --show-toplevel", "/work/sky\n"), ("symbolic-ref --short -q HEAD", "main\n")])
            .inspect(Path::new("/work/sky"))
            .expect("repo");
        assert_eq!(repo.slug, "sky");
        assert_eq!(repo.project, Some(ProjectRef::Local(PathBuf::from("/work/sky"))));
        assert_eq!(repo.head_short_sha, "");
        assert_eq!(repo.branch, "main");
        assert!(repo.branches.is_empty());
    }

    #[test]
    fn detached_head_is_labelled() {
        let repo = git(&[("rev-parse --show-toplevel", "/w/x\n"), ("log -1 --format=%h%x00%s", "abc\0Msg")])
            .inspect(Path::new("/w/x"))
            .expect("repo");
        assert_eq!(repo.branch, "detached@abc");
    }

    #[test]
    fn not_a_repository() {
        let error = git(&[]).inspect(Path::new("/tmp")).expect_err("no repo");
        assert!(matches!(error, WorkspaceError::NotARepository(_)));
    }

    #[test]
    fn pure_helpers() {
        assert_eq!(parse_head("abc\0Subject \n"), ("abc".into(), "Subject".into()));
        assert_eq!(parse_head("abc"), ("abc".into(), String::new()));
        assert_eq!(parse_lines(" a \n\nb\n"), vec!["a", "b"]);
        assert_eq!(repository_slug(Some("not a url/"), Path::new("/w/fallback")), "fallback");
        assert_eq!(repository_slug(None, Path::new("/w/dir")), "dir");
    }
}
