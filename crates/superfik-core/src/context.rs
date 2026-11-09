//! Context a user attaches to a prompt: the nodes on the left of the canvas.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::repository::Repository;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub repository: Option<Repository>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SkillIcon {
    Cube,
    Bolt,
    Sparkle,
    Book,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub title: String,
    pub description: String,
    pub icon: SkillIcon,
}

impl Skill {
    pub fn slash(&self) -> String {
        format!("/{}", self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResearchKind {
    Article,
    Document,
    Discussion,
    Video,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchItem {
    pub title: String,
    pub url: Option<String>,
    pub kind: ResearchKind,
}

impl ResearchKind {
    pub fn from_url(url: &str) -> Self {
        let url = url.to_ascii_lowercase();
        if url.contains("youtube.") || url.contains("vimeo.") {
            Self::Video
        } else if url.contains("reddit.") || url.contains("news.ycombinator") || url.contains("/discussions") {
            Self::Discussion
        } else if url.ends_with(".pdf") || url.contains("/docs") || url.contains("developer.") {
            Self::Document
        } else {
            Self::Article
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn research_kind_from_url() {
        assert_eq!(ResearchKind::from_url("https://youtube.com/watch?v=1"), ResearchKind::Video);
        assert_eq!(ResearchKind::from_url("https://reddit.com/r/x"), ResearchKind::Discussion);
        assert_eq!(ResearchKind::from_url("https://x.dev/docs/a"), ResearchKind::Document);
        assert_eq!(ResearchKind::from_url("https://blog.x.dev/a"), ResearchKind::Article);
    }
}
