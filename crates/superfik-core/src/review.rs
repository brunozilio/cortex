//! Reviewing a pull request: the comments on its diff, where they came
//! from and whether they are published yet, and the verdict a review ends
//! with.

use serde::{Deserialize, Serialize};

use crate::diff::FileDiff;
use crate::repository::PullRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommentSide {
    Old,
    New,
}

impl CommentSide {
    pub fn github(self) -> &'static str {
        match self {
            Self::Old => "LEFT",
            Self::New => "RIGHT",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommentOrigin {
    User,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommentState {
    Draft,
    Published,
    Resolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewComment {
    pub id: u64,
    pub path: String,
    pub line: u32,
    pub side: CommentSide,
    pub body: String,
    pub author: String,
    pub origin: CommentOrigin,
    pub state: CommentState,
    #[serde(default)]
    pub thread: Option<String>,
}

impl ReviewComment {
    pub fn is_draft(&self) -> bool {
        self.state == CommentState::Draft
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReviewVerdict {
    Approve,
    RequestChanges,
    Comment,
}

impl ReviewVerdict {
    pub fn github(self) -> &'static str {
        match self {
            Self::Approve => "APPROVE",
            Self::RequestChanges => "REQUEST_CHANGES",
            Self::Comment => "COMMENT",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewCommentDraft {
    pub path: String,
    pub line: u32,
    pub side: CommentSide,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestDetail {
    pub pull_request: PullRequest,
    pub description: String,
    pub files: Vec<FileDiff>,
    pub comments: Vec<ReviewComment>,
}
