//! Parsing `gh … --json` output. Pure.

use serde::Deserialize;
use superfik_core::{
    CommentOrigin, CommentSide, CommentState, GitHubRepository, PullRequest, PullRequestState, ReviewComment,
    ReviewDecision,
};

pub const REPO_FIELDS: &str = "nameWithOwner";
pub const PR_FIELDS: &str = "number,title,state,isDraft,author,headRefName,baseRefName,url,updatedAt,additions,deletions,changedFiles,reviewDecision";
pub const LIST_ARGS: [&str; 8] = [
    "api",
    "-X",
    "GET",
    "user/repos",
    "-f",
    "affiliation=owner,collaborator,organization_member",
    "-f",
    "sort=pushed&per_page=100",
];

#[derive(Deserialize)]
struct RepoListing {
    full_name: String,
    #[serde(default)]
    private: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepoView {
    name_with_owner: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrStatus {
    #[serde(default)]
    current_branch: Option<PrView>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PrView {
    number: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    is_draft: bool,
    #[serde(default)]
    author: Option<Author>,
    #[serde(default)]
    head_ref_name: String,
    #[serde(default)]
    base_ref_name: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    updated_at: String,
    #[serde(default)]
    additions: u64,
    #[serde(default)]
    deletions: u64,
    #[serde(default)]
    changed_files: u64,
    #[serde(default)]
    review_decision: String,
}

#[derive(Deserialize, Default)]
struct Author {
    #[serde(default)]
    login: String,
}

impl PrView {
    fn into_pull_request(self) -> PullRequest {
        let state = match self.state.to_ascii_uppercase().as_str() {
            "MERGED" => PullRequestState::Merged,
            "CLOSED" => PullRequestState::Closed,
            _ if self.is_draft => PullRequestState::Draft,
            _ => PullRequestState::Open,
        };
        let review_decision = match self.review_decision.to_ascii_uppercase().as_str() {
            "APPROVED" => Some(ReviewDecision::Approved),
            "CHANGES_REQUESTED" => Some(ReviewDecision::ChangesRequested),
            "REVIEW_REQUIRED" => Some(ReviewDecision::ReviewRequired),
            _ => None,
        };
        PullRequest {
            number: self.number,
            title: self.title,
            state,
            author: self.author.map(|a| a.login).unwrap_or_default(),
            head_branch: self.head_ref_name,
            base_branch: self.base_ref_name,
            url: self.url,
            updated_at: self.updated_at,
            additions: self.additions,
            deletions: self.deletions,
            changed_files: self.changed_files,
            review_decision,
        }
    }
}

pub fn repository(json: &str) -> Option<String> {
    let view: RepoView = serde_json::from_str(json).ok()?;
    let slug = view.name_with_owner.trim();
    slug.contains('/').then(|| slug.to_owned())
}

pub fn repositories(json: &str) -> Option<Vec<GitHubRepository>> {
    let listing: Vec<RepoListing> = serde_json::from_str(json).ok()?;
    Some(
        listing
            .into_iter()
            .filter(|repo| repo.full_name.contains('/'))
            .map(|repo| GitHubRepository { slug: repo.full_name, private: repo.private })
            .collect(),
    )
}

pub fn pull_request(json: &str) -> Option<PullRequest> {
    let status: PrStatus = serde_json::from_str(json).ok()?;
    Some(status.current_branch?.into_pull_request())
}

pub const DETAIL_FIELDS: &str = "number,title,state,isDraft,author,headRefName,baseRefName,url,updatedAt,additions,deletions,changedFiles,reviewDecision,body";

pub const THREADS_QUERY: &str = "query($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name){pullRequest(number:$number){reviewThreads(first:100){nodes{id isResolved comments(first:50){nodes{databaseId body path line startLine diffSide author{login}}}}}}}}";

#[derive(Deserialize)]
struct PrDetail {
    #[serde(flatten)]
    view: PrView,
    #[serde(default)]
    body: String,
}

pub fn pull_request_detail(json: &str) -> Option<(PullRequest, String)> {
    let detail: PrDetail = serde_json::from_str(json).ok()?;
    Some((detail.view.into_pull_request(), detail.body))
}

#[derive(Deserialize)]
struct ThreadsResponse {
    data: ThreadsData,
}
#[derive(Deserialize)]
struct ThreadsData {
    repository: ThreadsRepository,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadsRepository {
    pull_request: ThreadsPullRequest,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadsPullRequest {
    review_threads: Nodes<Thread>,
}
#[derive(Deserialize)]
struct Nodes<T> {
    nodes: Vec<T>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Thread {
    id: String,
    #[serde(default)]
    is_resolved: bool,
    comments: Nodes<ThreadComment>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadComment {
    #[serde(default)]
    database_id: u64,
    #[serde(default)]
    body: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    line: Option<u32>,
    #[serde(default)]
    start_line: Option<u32>,
    #[serde(default)]
    diff_side: String,
    #[serde(default)]
    author: Option<Author>,
}

pub fn review_threads(json: &str) -> Option<Vec<ReviewComment>> {
    let response: ThreadsResponse = serde_json::from_str(json).ok()?;
    let mut comments = Vec::new();
    for thread in response.data.repository.pull_request.review_threads.nodes {
        for comment in thread.comments.nodes {
            let Some(line) = comment.line.or(comment.start_line) else { continue };
            comments.push(ReviewComment {
                id: comment.database_id,
                path: comment.path,
                line,
                side: if comment.diff_side.eq_ignore_ascii_case("LEFT") { CommentSide::Old } else { CommentSide::New },
                body: comment.body,
                author: comment.author.map(|a| a.login).unwrap_or_default(),
                origin: CommentOrigin::User,
                state: if thread.is_resolved { CommentState::Resolved } else { CommentState::Published },
                thread: Some(thread.id.clone()),
            });
        }
    }
    Some(comments)
}

pub fn pull_request_view(json: &str) -> Option<PullRequest> {
    let view: PrView = serde_json::from_str(json).ok()?;
    Some(view.into_pull_request())
}

pub fn pull_requests(json: &str) -> Option<Vec<PullRequest>> {
    let views: Vec<PrView> = serde_json::from_str(json).ok()?;
    Some(views.into_iter().map(PrView::into_pull_request).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repositories_keep_slug_and_visibility() {
        let json = r#"[{"full_name":"acme/rocket","private":true,"other":1},{"full_name":"broken"}]"#;
        let repos = repositories(json).expect("parsed");
        assert_eq!(repos, vec![GitHubRepository { slug: "acme/rocket".into(), private: true }]);
        assert_eq!(repositories("not json"), None);
    }

    #[test]
    fn repository_view() {
        assert_eq!(repository(r#"{"nameWithOwner":"acme/rocket"}"#).as_deref(), Some("acme/rocket"));
        assert_eq!(repository(r#"{"nameWithOwner":""}"#), None);
        assert_eq!(repository("not json"), None);
    }

    #[test]
    fn current_branch_pull_request() {
        let json = r#"{"createdBy":[],"currentBranch":{"isDraft":false,"number":42,"state":"OPEN","title":"Sky"},"needsReview":[]}"#;
        let pr = pull_request(json).expect("pr");
        assert_eq!((pr.number, pr.title.as_str(), pr.state), (42, "Sky", PullRequestState::Open));
    }

    #[test]
    fn pull_request_listing_keeps_the_facts_a_card_shows() {
        let json = r#"[{"additions":188,"author":{"id":"x","is_bot":false,"login":"tidy-dev","name":"T"},"baseRefName":"trunk","changedFiles":2,"deletions":0,"headRefName":"tidy-dev-skill","isDraft":false,"number":14846,"reviewDecision":"REVIEW_REQUIRED","state":"OPEN","title":"Add skill","updatedAt":"2026-10-28T02:11:55Z","url":"https://github.com/cli/cli/pull/14846"},{"number":7,"title":"Draft","state":"OPEN","isDraft":true,"reviewDecision":""}]"#;
        let prs = pull_requests(json).expect("parsed");
        assert_eq!(prs.len(), 2);
        let pr = &prs[0];
        assert_eq!(
            (pr.number, pr.author.as_str(), pr.head_branch.as_str(), pr.base_branch.as_str()),
            (14846, "tidy-dev", "tidy-dev-skill", "trunk")
        );
        assert_eq!((pr.additions, pr.deletions, pr.changed_files), (188, 0, 2));
        assert_eq!(pr.review_decision, Some(ReviewDecision::ReviewRequired));
        assert_eq!(pr.size_caption(), "+188 −0 · 2 files");
        assert_eq!(prs[1].state, PullRequestState::Draft);
        assert_eq!(prs[1].review_decision, None);
        assert_eq!(pull_requests("nope"), None);
    }

    #[test]
    fn a_single_view_parses_like_a_listing_row() {
        let pr =
            pull_request_view(r#"{"number":5,"title":"Sky","state":"OPEN","isDraft":false,"url":"u"}"#).expect("pr");
        assert_eq!((pr.number, pr.url.as_str()), (5, "u"));
        assert_eq!(pull_request_view("[]"), None);
    }

    #[test]
    fn detail_carries_the_description_and_threads_their_ids() {
        let (pr, body) =
            pull_request_detail(r#"{"number":5,"title":"Sky","state":"OPEN","body":"Adds stars"}"#).expect("detail");
        assert_eq!((pr.number, body.as_str()), (5, "Adds stars"));
        let json = r#"{"data":{"repository":{"pullRequest":{"reviewThreads":{"nodes":[{"id":"PRRT_1","isResolved":true,"comments":{"nodes":[{"databaseId":9,"body":"Nit","path":"src/a.rs","line":12,"startLine":null,"diffSide":"RIGHT","author":{"login":"ana"}},{"databaseId":10,"body":"outdated","path":"src/a.rs","line":null,"startLine":null,"diffSide":"LEFT","author":null}]}}]}}}}}"#;
        let comments = review_threads(json).expect("threads");
        assert_eq!(comments.len(), 1, "comments without a line are skipped");
        let c = &comments[0];
        assert_eq!(
            (c.id, c.path.as_str(), c.line, c.side, c.author.as_str()),
            (9, "src/a.rs", 12, CommentSide::New, "ana")
        );
        assert_eq!((c.state, c.thread.as_deref()), (CommentState::Resolved, Some("PRRT_1")));
        assert_eq!(review_threads("{}"), None);
    }

    #[test]
    fn states() {
        let state = |state: &str, draft: bool| {
            let json = format!(r#"{{"currentBranch":{{"number":1,"title":"t","state":"{state}","isDraft":{draft}}}}}"#);
            pull_request(&json).map(|pr| pr.state)
        };
        assert_eq!(state("OPEN", true), Some(PullRequestState::Draft));
        assert_eq!(state("MERGED", false), Some(PullRequestState::Merged));
        assert_eq!(state("CLOSED", false), Some(PullRequestState::Closed));
    }

    #[test]
    fn no_pull_request_for_the_branch() {
        assert_eq!(pull_request(r#"{"createdBy":[],"currentBranch":null,"needsReview":[]}"#), None);
        assert_eq!(pull_request(r#"{"createdBy":[],"needsReview":[]}"#), None);
        assert_eq!(pull_request(""), None);
    }
}
