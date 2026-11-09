//! Structured results an agent hands back to the app through a JSON file:
//! a pull request draft, a code review. The engine tells the agent where
//! to write it (see [`PromptRequest::compose`]); the backend reads and
//! parses it when the run completes. Parsing is strict and never panics:
//! a malformed file is simply no result.
//!
//! [`PromptRequest::compose`]: crate::PromptRequest::compose

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::repository::PullRequestDraft;
use crate::review::{CommentSide, ReviewCommentDraft};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HandoffKind {
    PullRequestDraft,
    Review { number: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handoff {
    pub kind: HandoffKind,
    #[serde(default)]
    pub output: PathBuf,
}

impl Handoff {
    pub fn new(kind: HandoffKind) -> Self {
        Self { kind, output: PathBuf::new() }
    }

    pub fn instructions(&self) -> Option<String> {
        if self.output.as_os_str().is_empty() {
            return None;
        }
        let path = self.output.display();
        Some(match self.kind {
            HandoffKind::PullRequestDraft => format!(
                "When you are done, write a JSON file at {path} with exactly these fields: \
                 {{\"title\": \"<pull request title, under 70 characters>\", \"body\": \"<pull request \
                 description in Markdown: what changed and why, how it was tested>\"}}. Describe the \
                 changes of the current branch against its base. Write the file with the Write tool; do \
                 not print the JSON in your answer."
            ),
            HandoffKind::Review { number } => format!(
                "Review pull request #{number} of this repository: run `gh pr diff {number}` to read its \
                 diff (and `gh pr view {number}` for its description), read the surrounding code where \
                 needed, and look for bugs, risks and clear improvements. When you are done, write a JSON \
                 file at {path} with exactly these fields: {{\"summary\": \"<one paragraph>\", \
                 \"comments\": [{{\"path\": \"<file path as in the diff>\", \"line\": <line number in the \
                 new file, or in the old file for removed lines>, \"side\": \"new\" or \"old\", \"body\": \
                 \"<the comment, Markdown>\", \"severity\": \"issue\" or \"suggestion\" or \"nit\"}}]}}. \
                 Only comment on lines that appear in the diff. Write the file with the Write tool; keep \
                 your answer to a short summary and do not print the JSON."
            ),
        })
    }
}

#[derive(Deserialize)]
struct DraftFile {
    title: String,
    #[serde(default)]
    body: String,
}

#[derive(Deserialize)]
struct ReviewFile {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    comments: Vec<ReviewFileComment>,
}

#[derive(Deserialize)]
struct ReviewFileComment {
    #[serde(default)]
    path: String,
    #[serde(default)]
    line: Option<u32>,
    #[serde(default)]
    side: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    severity: String,
}

pub fn parse_review(json: &str) -> Option<(String, Vec<ReviewCommentDraft>)> {
    let file: ReviewFile = serde_json::from_str(json).ok()?;
    let comments = file
        .comments
        .into_iter()
        .filter_map(|c| {
            let line = c.line.filter(|n| *n > 0)?;
            let body = c.body.trim();
            if c.path.trim().is_empty() || body.is_empty() {
                return None;
            }
            let side = match c.side.trim().to_ascii_lowercase().as_str() {
                "old" | "left" => CommentSide::Old,
                _ => CommentSide::New,
            };
            let severity = c.severity.trim().to_ascii_lowercase();
            let body = match severity.as_str() {
                "nit" => format!("nit: {body}"),
                "suggestion" => format!("suggestion: {body}"),
                _ => body.to_owned(),
            };
            Some(ReviewCommentDraft { path: c.path.trim().to_owned(), line, side, body })
        })
        .collect();
    Some((file.summary.trim().to_owned(), comments))
}

pub fn parse_pull_request_draft(json: &str) -> Option<PullRequestDraft> {
    let file: DraftFile = serde_json::from_str(json).ok()?;
    let title = file.title.trim();
    (!title.is_empty()).then(|| PullRequestDraft {
        title: title.to_owned(),
        body: file.body.trim().to_owned(),
        ..PullRequestDraft::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instructions_name_the_file_only_once_assigned() {
        let mut handoff = Handoff::new(HandoffKind::PullRequestDraft);
        assert_eq!(handoff.instructions(), None);
        handoff.output = "/tmp/h.json".into();
        let text = handoff.instructions().expect("instructions");
        assert!(text.contains("/tmp/h.json") && text.contains("\"title\""));
    }

    #[test]
    fn reviews_keep_valid_comments_and_prefix_severity() {
        let json = r#"{"summary":" Fine. ","comments":[{"path":"a.rs","line":3,"side":"NEW","body":"Off by one","severity":"issue"},{"path":"a.rs","line":4,"side":"old","body":"rename","severity":"nit"},{"path":"","line":1,"body":"x"},{"path":"b.rs","line":0,"body":"x"},{"path":"b.rs","line":2,"body":"  "}]}"#;
        let (summary, comments) = parse_review(json).expect("review");
        assert_eq!(summary, "Fine.");
        assert_eq!(comments.len(), 2);
        assert_eq!(
            (comments[0].path.as_str(), comments[0].line, comments[0].side, comments[0].body.as_str()),
            ("a.rs", 3, CommentSide::New, "Off by one")
        );
        assert_eq!((comments[1].side, comments[1].body.as_str()), (CommentSide::Old, "nit: rename"));
        assert_eq!(parse_review("not json"), None);
        let text = Handoff { output: "/tmp/r.json".into(), ..Handoff::new(HandoffKind::Review { number: 7 }) }
            .instructions()
            .expect("text");
        assert!(text.contains("gh pr diff 7") && text.contains("/tmp/r.json"));
    }

    #[test]
    fn drafts_parse_strictly() {
        let draft =
            parse_pull_request_draft(r#"{"title":" Night sky ","body":"Adds stars.\n","extra":1}"#).expect("draft");
        assert_eq!((draft.title.as_str(), draft.body.as_str()), ("Night sky", "Adds stars."));
        assert_eq!(parse_pull_request_draft(r#"{"title":"  "}"#), None);
        assert_eq!(parse_pull_request_draft("not json"), None);
        assert_eq!(parse_pull_request_draft(r#"{"body":"x"}"#), None);
    }
}
