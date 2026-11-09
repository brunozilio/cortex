//! The repository a workspace lives in: git facts plus optional GitHub
//! enrichment (the open pull request of the current branch).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const ORIGIN: &str = "origin";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repository {
    pub root: PathBuf,
    pub slug: String,
    #[serde(default)]
    pub project: Option<crate::project::ProjectRef>,
    pub head_short_sha: String,
    pub head_subject: String,
    pub branch: String,
    pub branches: Vec<String>,
    #[serde(default)]
    pub remote_branches: Vec<String>,
    #[serde(default)]
    pub pull_request: Option<PullRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubRepository {
    pub slug: String,
    pub private: bool,
}

impl GitHubRepository {
    pub fn name(&self) -> &str {
        self.slug.rsplit('/').next().unwrap_or(&self.slug)
    }
}

impl Repository {
    pub fn name(&self) -> &str {
        self.slug.rsplit('/').next().unwrap_or(&self.slug)
    }

    pub fn default_base(&self) -> String {
        ["main", "master"]
            .iter()
            .find(|b| self.branches.iter().any(|x| x == *b))
            .map_or_else(String::new, |b| (*b).to_owned())
    }

    pub fn branch_caption(&self) -> String {
        format!("{}/{}", self.name(), self.branch)
    }

    pub fn head_caption(&self) -> String {
        format!("{} · {}", self.head_short_sha, self.head_subject)
    }

    pub fn branch_choices(&self) -> Vec<BranchRef> {
        let local = self.branches.iter().map(|name| BranchRef::local(name.clone()));
        let remote = self
            .remote_branches
            .iter()
            .filter(|name| !self.branches.contains(name))
            .map(|name| BranchRef::remote(name.clone()));
        local.chain(remote).collect()
    }

    pub fn branch_label(&self, branch: &str) -> String {
        match &self.pull_request {
            Some(pr) if branch == self.branch => format!("{branch} · {}", pr.badge()),
            _ => branch.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BranchRef {
    pub name: String,
    pub remote: bool,
}

impl BranchRef {
    pub fn local(name: impl Into<String>) -> Self {
        Self { name: name.into(), remote: false }
    }

    pub fn remote(name: impl Into<String>) -> Self {
        Self { name: name.into(), remote: true }
    }

    pub fn label(&self) -> String {
        if self.remote { format!("{ORIGIN}/{}", self.name) } else { self.name.clone() }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PullRequestState {
    #[default]
    Open,
    Draft,
    Merged,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReviewDecision {
    Approved,
    ChangesRequested,
    ReviewRequired,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub state: PullRequestState,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub head_branch: String,
    #[serde(default)]
    pub base_branch: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub additions: u64,
    #[serde(default)]
    pub deletions: u64,
    #[serde(default)]
    pub changed_files: u64,
    #[serde(default)]
    pub review_decision: Option<ReviewDecision>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestDraft {
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub base: String,
    #[serde(default)]
    pub draft: bool,
}

impl PullRequest {
    pub fn size_caption(&self) -> String {
        let files = if self.changed_files == 1 { "file" } else { "files" };
        format!("+{} −{} · {} {files}", self.additions, self.deletions, self.changed_files)
    }

    pub fn branches_caption(&self) -> String {
        format!("{} ← {}", self.base_branch, self.head_branch)
    }

    pub fn badge(&self) -> String {
        match self.state {
            PullRequestState::Open => format!("#{}", self.number),
            PullRequestState::Draft => format!("#{} draft", self.number),
            PullRequestState::Merged => format!("#{} merged", self.number),
            PullRequestState::Closed => format!("#{} closed", self.number),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> Repository {
        Repository {
            root: PathBuf::from("/tmp/dither-minis"),
            slug: "zzzzshawn/dither-minis".into(),
            head_short_sha: "a1f3c2e".into(),
            head_subject: "Tighten the outline".into(),
            branch: "main".into(),
            branches: vec!["main".into()],
            ..Repository::default()
        }
    }

    #[test]
    fn captions_match_design_copy() {
        assert_eq!(repo().branch_caption(), "dither-minis/main");
        assert_eq!(repo().head_caption(), "a1f3c2e · Tighten the outline");
    }

    #[test]
    fn branch_choices_list_local_then_remote_only() {
        let repo = Repository {
            branches: vec!["dev".into(), "main".into()],
            remote_branches: vec!["main".into(), "spike".into()],
            ..repo()
        };
        let choices = repo.branch_choices();
        assert_eq!(choices, vec![BranchRef::local("dev"), BranchRef::local("main"), BranchRef::remote("spike")]);
        assert_eq!(choices[2].label(), "origin/spike");
        assert_eq!(choices[0].label(), "dev");
    }

    #[test]
    fn branch_label_shows_the_current_branch_pull_request() {
        let pr = PullRequest { number: 42, title: "Sky".into(), ..PullRequest::default() };
        let with_pr = Repository { pull_request: Some(pr), ..repo() };
        assert_eq!(with_pr.branch_label("main"), "main · #42");
        assert_eq!(with_pr.branch_label("dev"), "dev");
        assert_eq!(repo().branch_label("main"), "main");
    }

    #[test]
    fn pull_request_badges() {
        let pr = |state| PullRequest { number: 7, state, ..PullRequest::default() };
        assert_eq!(pr(PullRequestState::Draft).badge(), "#7 draft");
        assert_eq!(pr(PullRequestState::Merged).badge(), "#7 merged");
        assert_eq!(pr(PullRequestState::Closed).badge(), "#7 closed");
    }

    #[test]
    fn older_snapshots_without_new_fields_deserialize() {
        let json = r#"{"root":"/r","slug":"a/b","head_short_sha":"","head_subject":"","branch":"main","branches":[]}"#;
        let repo: Repository = serde_json::from_str(json).expect("deserialize");
        assert!(repo.remote_branches.is_empty() && repo.pull_request.is_none());
    }
}
