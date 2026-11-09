//! [`GitHubClient`] backed by the GitHub CLI (`gh`), which brings its own
//! authentication.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use superfik_core::{
    GitHubRepository, PullRequest, PullRequestDetail, PullRequestDraft, ReviewCommentDraft, ReviewVerdict,
};

use super::{GitHubClient, parse, process};
use crate::error::WorkspaceError;
use crate::executable;

pub const ENV_OVERRIDE: &str = "SUPERFIK_GH_BIN";
const TIMEOUT: Duration = Duration::from_secs(8);
const CLONE_TIMEOUT: Duration = Duration::from_secs(10 * 60);

pub trait GhRunner: Send + Sync {
    fn run(&self, cwd: &Path, args: &[&str]) -> Option<String>;

    fn run_slow(&self, cwd: &Path, args: &[&str]) -> Option<String> {
        self.run(cwd, args)
    }

    fn as_user(&self, _token: &str) -> Option<Self>
    where
        Self: Sized,
    {
        None
    }
}

#[derive(Clone)]
pub struct SystemGh {
    program: PathBuf,
    token: Option<String>,
}

impl std::fmt::Debug for SystemGh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemGh").field("program", &self.program).field("as_user", &self.token.is_some()).finish()
    }
}

impl SystemGh {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self { program: program.into(), token: None }
    }

    pub fn with_token(&self, token: &str) -> Self {
        Self { token: Some(token.to_owned()), ..self.clone() }
    }
}

impl SystemGh {
    fn command(&self, args: &[&str]) -> Command {
        let mut command = superfik_process::command(&self.program);
        command
            .args(args)
            .env("GH_PROMPT_DISABLED", "1")
            .env("GH_NO_UPDATE_NOTIFIER", "1")
            .env("NO_COLOR", "1")
            .env("GIT_OPTIONAL_LOCKS", "0");
        if let Some(token) = &self.token {
            command.env("GH_TOKEN", token);
        }
        command
    }
}

impl GhRunner for SystemGh {
    fn run(&self, cwd: &Path, args: &[&str]) -> Option<String> {
        process::output_within(self.command(args), cwd, TIMEOUT)
    }

    fn run_slow(&self, cwd: &Path, args: &[&str]) -> Option<String> {
        process::output_within(self.command(args), cwd, CLONE_TIMEOUT)
    }

    fn as_user(&self, token: &str) -> Option<Self> {
        Some(self.with_token(token))
    }
}

#[derive(Debug, Clone)]
pub struct GhCli<R = SystemGh> {
    runner: R,
}

impl GhCli<SystemGh> {
    pub fn detect() -> Option<Self> {
        executable::locate("gh", ENV_OVERRIDE).map(|program| Self::new(SystemGh::new(program)))
    }
}

impl<R: GhRunner> GhCli<R> {
    pub fn new(runner: R) -> Self {
        Self { runner }
    }
}

impl<R: GhRunner + 'static> GitHubClient for GhCli<R> {
    fn repository(&self, root: &Path) -> Option<String> {
        parse::repository(&self.runner.run(root, &["repo", "view", "--json", parse::REPO_FIELDS])?)
    }

    fn pull_request(&self, root: &Path) -> Option<PullRequest> {
        parse::pull_request(&self.runner.run(root, &["pr", "status", "--json", parse::PR_FIELDS])?)
    }

    fn repositories(&self) -> Option<Vec<GitHubRepository>> {
        let out = self.runner.run(Path::new("/"), &parse::LIST_ARGS)?;
        parse::repositories(&out)
    }

    fn pull_requests(&self, root: &Path) -> Option<Vec<PullRequest>> {
        let args = ["pr", "list", "--state", "open", "--limit", "30", "--json", parse::PR_FIELDS];
        parse::pull_requests(&self.runner.run(root, &args)?)
    }

    fn checkout_pull_request(&self, root: &Path, number: u64) -> Result<(), WorkspaceError> {
        let number = number.to_string();
        let args = ["pr", "checkout", &number];
        self.runner.run_slow(root, &args).map(|_| ()).ok_or_else(|| WorkspaceError::Gh { command: args.join(" ") })
    }

    fn create_pull_request(&self, root: &Path, draft: &PullRequestDraft) -> Result<PullRequest, WorkspaceError> {
        let body = tempfile::NamedTempFile::new()
            .and_then(|file| std::fs::write(file.path(), draft.body.as_bytes()).map(|()| file))
            .map_err(|source| WorkspaceError::Io { path: std::env::temp_dir(), source })?;
        let body_path = body.path().to_string_lossy().into_owned();
        let mut args = vec!["pr", "create", "--title", draft.title.as_str(), "--body-file", body_path.as_str()];
        if !draft.base.is_empty() {
            args.extend(["--base", draft.base.as_str()]);
        }
        if draft.draft {
            args.push("--draft");
        }
        self.runner.run_slow(root, &args).ok_or_else(|| WorkspaceError::Gh { command: "pr create".into() })?;
        let view = self
            .runner
            .run(root, &["pr", "view", "--json", parse::PR_FIELDS])
            .ok_or_else(|| WorkspaceError::Gh { command: "pr view".into() })?;
        parse::pull_request_view(&view).ok_or_else(|| WorkspaceError::Gh { command: "pr view".into() })
    }

    fn pull_request_detail(&self, root: &Path, number: u64) -> Result<PullRequestDetail, WorkspaceError> {
        let gh = |command: &str| WorkspaceError::Gh { command: command.to_owned() };
        let n = number.to_string();
        let view =
            self.runner.run(root, &["pr", "view", &n, "--json", parse::DETAIL_FIELDS]).ok_or_else(|| gh("pr view"))?;
        let (pull_request, description) = parse::pull_request_detail(&view).ok_or_else(|| gh("pr view"))?;
        let patch = self.runner.run_slow(root, &["pr", "diff", &n, "--patch"]).ok_or_else(|| gh("pr diff"))?;
        let files = superfik_core::diff::parse(&patch);
        let slug = self.repository(root).unwrap_or_default();
        let (owner, name) = slug.split_once('/').unwrap_or(("", ""));
        let threads = self.runner.run(
            root,
            &[
                "api",
                "graphql",
                "-f",
                &format!("query={}", parse::THREADS_QUERY),
                "-F",
                &format!("owner={owner}"),
                "-F",
                &format!("name={name}"),
                "-F",
                &format!("number={number}"),
            ],
        );
        let comments = threads.and_then(|json| parse::review_threads(&json)).unwrap_or_default();
        Ok(PullRequestDetail { pull_request, description, files, comments })
    }

    fn submit_review(
        &self,
        root: &Path,
        number: u64,
        verdict: ReviewVerdict,
        body: &str,
        comments: &[ReviewCommentDraft],
    ) -> Result<(), WorkspaceError> {
        let slug = self.repository(root).ok_or_else(|| WorkspaceError::Gh { command: "repo view".into() })?;
        let payload = serde_json::json!({
            "event": verdict.github(),
            "body": body,
            "comments": comments.iter().map(|c| serde_json::json!({
                "path": c.path, "line": c.line, "side": c.side.github(), "body": c.body,
            })).collect::<Vec<_>>(),
        });
        let input = tempfile::NamedTempFile::new()
            .and_then(|file| std::fs::write(file.path(), payload.to_string()).map(|()| file))
            .map_err(|source| WorkspaceError::Io { path: std::env::temp_dir(), source })?;
        let input_path = input.path().to_string_lossy().into_owned();
        let endpoint = format!("repos/{slug}/pulls/{number}/reviews");
        self.runner
            .run(root, &["api", "-X", "POST", &endpoint, "--input", &input_path])
            .map(|_| ())
            .ok_or_else(|| WorkspaceError::Gh { command: format!("api {endpoint}") })
    }

    fn resolve_thread(&self, root: &Path, thread: &str, resolved: bool) -> Result<(), WorkspaceError> {
        let mutation = if resolved { "resolveReviewThread" } else { "unresolveReviewThread" };
        let query = format!("mutation($id:ID!){{{mutation}(input:{{threadId:$id}}){{thread{{id}}}}}}");
        self.runner
            .run(root, &["api", "graphql", "-f", &format!("query={query}"), "-F", &format!("id={thread}")])
            .map(|_| ())
            .ok_or_else(|| WorkspaceError::Gh { command: mutation.to_owned() })
    }

    fn clone_repository(&self, slug: &str, destination: &Path) -> Result<(), WorkspaceError> {
        let parent = destination.parent().unwrap_or(destination);
        std::fs::create_dir_all(parent).map_err(|source| WorkspaceError::Io { path: parent.to_path_buf(), source })?;
        let target = destination.to_string_lossy().into_owned();
        self.runner
            .run_slow(parent, &["repo", "clone", slug, &target])
            .map(|_| ())
            .ok_or_else(|| WorkspaceError::Clone { slug: slug.to_owned() })
    }

    fn as_user(&self, token: &str) -> Option<std::sync::Arc<dyn GitHubClient>> {
        let runner = self.runner.as_user(token)?;
        Some(std::sync::Arc::new(GhCli::new(runner)))
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::PullRequestState;

    use super::*;

    struct ScriptedGh(Vec<(&'static str, &'static str)>);
    impl GhRunner for ScriptedGh {
        fn run(&self, _cwd: &Path, args: &[&str]) -> Option<String> {
            let joined = args.join(" ");
            self.0
                .iter()
                .find(|(key, _)| key.strip_suffix('*').map_or(*key == joined, |prefix| joined.starts_with(prefix)))
                .map(|(_, out)| (*out).to_owned())
        }
    }

    #[test]
    fn a_users_gh_carries_their_token_and_the_rest_stays() {
        let gh = SystemGh::new("/usr/bin/gh");
        let theirs = gh.with_token("ghu_abc");
        let token = |gh: &SystemGh| {
            gh.command(&["api", "user"])
                .get_envs()
                .find(|(key, _)| *key == "GH_TOKEN")
                .and_then(|(_, value)| value.map(|v| v.to_string_lossy().into_owned()))
        };
        assert_eq!(token(&gh), None, "gh's own sign-in");
        assert_eq!(token(&theirs).as_deref(), Some("ghu_abc"));
        assert!(!format!("{theirs:?}").contains("ghu_abc"), "never in logs");
        assert!(GhCli::new(gh).as_user("ghu_abc").is_some());
        assert!(GhCli::new(ScriptedGh(vec![])).as_user("t").is_none(), "a runner that cannot act as a user");
    }

    #[test]
    fn asks_gh_for_repository_and_pull_request() {
        let gh = GhCli::new(ScriptedGh(vec![
            ("repo view --json nameWithOwner", r#"{"nameWithOwner":"acme/rocket"}"#),
            (
                "pr status --json number,title,state,isDraft,author,headRefName,baseRefName,url,updatedAt,additions,deletions,changedFiles,reviewDecision",
                r#"{"currentBranch":{"number":9,"title":"Fly","state":"OPEN","isDraft":true}}"#,
            ),
        ]));
        assert_eq!(gh.repository(Path::new("/r")).as_deref(), Some("acme/rocket"));
        let pr = gh.pull_request(Path::new("/r")).expect("pr");
        assert_eq!((pr.number, pr.state), (9, PullRequestState::Draft));
    }

    #[test]
    fn lists_the_accounts_repositories() {
        let args = parse::LIST_ARGS.join(" ");
        let gh = GhCli::new(ScriptedGh(vec![(
            Box::leak(args.into_boxed_str()),
            r#"[{"full_name":"acme/rocket","private":true},{"full_name":"acme/site","private":false}]"#,
        )]));
        let repos = gh.repositories().expect("listing");
        assert_eq!(repos.iter().map(|r| r.slug.as_str()).collect::<Vec<_>>(), vec!["acme/rocket", "acme/site"]);
        assert!(repos[0].private && !repos[1].private);
    }

    #[test]
    fn clones_into_the_destination_and_reports_failures() {
        let dir = tempfile::tempdir().expect("tempdir");
        let destination = dir.path().join("rocket");
        let target = destination.to_string_lossy().into_owned();
        let key = format!("repo clone acme/rocket {target}");
        let gh = GhCli::new(ScriptedGh(vec![(Box::leak(key.into_boxed_str()), "")]));
        assert!(gh.clone_repository("acme/rocket", &destination).is_ok());
        assert!(
            matches!(gh.clone_repository("acme/other", &destination), Err(WorkspaceError::Clone { slug }) if slug == "acme/other")
        );
    }

    #[test]
    fn lists_and_checks_out_pull_requests() {
        let gh = GhCli::new(ScriptedGh(vec![
            (
                "pr list --state open --limit 30 --json number,title,state,isDraft,author,headRefName,baseRefName,url,updatedAt,additions,deletions,changedFiles,reviewDecision",
                r#"[{"number":3,"title":"Sky","state":"OPEN","isDraft":false,"author":{"login":"ana"}}]"#,
            ),
            ("pr checkout 3", ""),
        ]));
        let prs = gh.pull_requests(Path::new("/r")).expect("listing");
        assert_eq!((prs[0].number, prs[0].author.as_str()), (3, "ana"));
        assert!(gh.checkout_pull_request(Path::new("/r"), 3).is_ok());
        assert!(
            matches!(gh.checkout_pull_request(Path::new("/r"), 4), Err(WorkspaceError::Gh { command }) if command == "pr checkout 4")
        );
    }

    #[test]
    fn creates_a_pull_request_and_reads_it_back() {
        let gh = GhCli::new(ScriptedGh(vec![
            ("pr create --title Sky --body-file *", ""),
            (
                "pr view --json number,title,state,isDraft,author,headRefName,baseRefName,url,updatedAt,additions,deletions,changedFiles,reviewDecision",
                r#"{"number":12,"title":"Sky","state":"OPEN","url":"https://x/12"}"#,
            ),
        ]));
        let draft = PullRequestDraft { title: "Sky".into(), body: "Stars".into(), base: "main".into(), draft: false };
        let pr = gh.create_pull_request(Path::new("/r"), &draft).expect("created");
        assert_eq!((pr.number, pr.url.as_str()), (12, "https://x/12"));
        let other = PullRequestDraft { title: "Moon".into(), ..draft };
        assert!(
            matches!(gh.create_pull_request(Path::new("/r"), &other), Err(WorkspaceError::Gh { command }) if command == "pr create")
        );
    }

    #[test]
    fn loads_a_pull_requests_detail_with_diff_and_threads() {
        let gh = GhCli::new(ScriptedGh(vec![
            ("repo view --json nameWithOwner", r#"{"nameWithOwner":"acme/rocket"}"#),
            ("pr view 5 --json *", r#"{"number":5,"title":"Sky","state":"OPEN","body":"Stars"}"#),
            ("pr diff 5 --patch", "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-x\n+y\n"),
            (
                "api graphql -f query=*",
                r#"{"data":{"repository":{"pullRequest":{"reviewThreads":{"nodes":[{"id":"T1","isResolved":false,"comments":{"nodes":[{"databaseId":1,"body":"hm","path":"a.rs","line":1,"diffSide":"RIGHT","author":{"login":"ana"}}]}}]}}}}}"#,
            ),
        ]));
        let detail = gh.pull_request_detail(Path::new("/r"), 5).expect("detail");
        assert_eq!((detail.pull_request.number, detail.description.as_str()), (5, "Stars"));
        assert_eq!(detail.files[0].hunks[0].lines.len(), 2);
        assert_eq!(detail.comments[0].thread.as_deref(), Some("T1"));
        assert!(
            matches!(gh.pull_request_detail(Path::new("/r"), 6), Err(WorkspaceError::Gh { command }) if command == "pr view")
        );
    }

    #[test]
    fn submits_reviews_and_resolves_threads_through_the_api() {
        let gh = GhCli::new(ScriptedGh(vec![
            ("repo view --json nameWithOwner", r#"{"nameWithOwner":"acme/rocket"}"#),
            ("api -X POST repos/acme/rocket/pulls/5/reviews --input *", "{}"),
            ("api graphql -f query=mutation($id:ID!){resolveReviewThread*", "{}"),
        ]));
        let comment = ReviewCommentDraft {
            path: "a.rs".into(),
            line: 1,
            side: superfik_core::CommentSide::New,
            body: "hm".into(),
        };
        assert!(gh.submit_review(Path::new("/r"), 5, ReviewVerdict::Approve, "LGTM", &[comment]).is_ok());
        assert!(gh.submit_review(Path::new("/r"), 6, ReviewVerdict::Comment, "", &[]).is_err());
        assert!(gh.resolve_thread(Path::new("/r"), "T1", true).is_ok());
        assert!(gh.resolve_thread(Path::new("/r"), "T1", false).is_err());
    }

    #[test]
    fn signed_out_gh_answers_nothing() {
        let gh = GhCli::new(ScriptedGh(vec![]));
        assert_eq!(gh.repository(Path::new("/r")), None);
        assert_eq!(gh.pull_request(Path::new("/r")), None);
        assert_eq!(gh.repositories(), None);
    }

    #[test]
    fn missing_binary_answers_nothing() {
        let gh = GhCli::new(SystemGh::new("/definitely/not/gh"));
        assert_eq!(gh.repository(Path::new("/")), None);
    }
}
