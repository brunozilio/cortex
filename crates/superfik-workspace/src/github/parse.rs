//! Parsing `gh … --json` output. Pure.

use serde::Deserialize;
use superfik_core::{PullRequest, PullRequestState, ReviewDecision};

pub const REPO_FIELDS: &str = "nameWithOwner";
pub const PR_FIELDS: &str = "number,title,state,isDraft,author,headRefName,baseRefName,url,updatedAt,additions,deletions,changedFiles,reviewDecision";

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

pub fn pull_request(json: &str) -> Option<PullRequest> {
    let status: PrStatus = serde_json::from_str(json).ok()?;
    Some(status.current_branch?.into_pull_request())
}

#[cfg(test)]
mod tests {
    use super::*;

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
