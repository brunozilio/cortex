//! The memory a run starts with, the instructions for writing edits, and
//! the parser for the file the agent writes.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{Memory, MemoryEdit, MemoryId, MemoryKind};

pub const MEMORY_SERVER: &str = "superfik_memory";
pub const MEMORY_SEARCH_TOOL: &str = "memory_search";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryTool {
    pub url: String,
    pub token: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryBrief {
    #[serde(default)]
    pub recalled: Vec<Memory>,
    #[serde(default)]
    pub output: PathBuf,
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(default)]
    pub tool: Option<MemoryTool>,
    #[serde(default)]
    pub optimizing: bool,
}

impl MemoryBrief {
    pub fn new(origin: Option<String>) -> Self {
        Self { origin, ..Self::default() }
    }

    pub fn instructions(&self, fresh: bool) -> Option<String> {
        let mut out = Vec::new();
        if fresh && !self.recalled.is_empty() {
            let mut listing = String::from("Memory from earlier sessions in this project (ids in brackets):");
            for (kind, title) in [
                (MemoryKind::Semantic, "Facts"),
                (MemoryKind::Procedural, "Procedures"),
                (MemoryKind::Episodic, "Episodes"),
            ] {
                let group: Vec<&Memory> = self.recalled.iter().filter(|m| m.kind == kind).collect();
                if group.is_empty() {
                    continue;
                }
                listing.push_str(&format!("\n{title}:"));
                for memory in group {
                    listing.push_str(&format!("\n- [{}] {}", memory.id.0, memory.text));
                }
            }
            out.push(listing);
        }
        if self.tool.is_some() {
            out.push(format!(
                "Search this project's memory with the `{MEMORY_SEARCH_TOOL}` tool whenever earlier work, \
                 decisions or conventions might matter; sub-agents can use it too. What is listed above is only \
                 a short index."
            ));
        }
        if !self.output.as_os_str().is_empty() {
            out.push(format!(
                "This app keeps the project's memory; do not write memory files of your own. When you are \
                 done, if this turn taught you something worth keeping for future sessions, write a JSON file \
                 at {path} with any of these fields: {{\"remember\": [{{\"kind\": \"episodic\" or \"semantic\" \
                 or \"procedural\", \"text\": \"<one or two sentences>\", \"tags\": [\"<topic>\"]}}], \
                 \"forget\": [<id>], \"revise\": [{{\"id\": <id>, \"text\": \"<corrected text>\"}}], \
                 \"reclassify\": [{{\"id\": <id>, \"kind\": \"<kind>\"}}]}}. Episodic: \
                 what happened in this turn, at most one. Semantic: a durable fact about the project or the \
                 user. Procedural: how to do something here (commands, workflows). Reuse existing tags. Forget \
                 or revise memories that turned out wrong. Skip the file when there is nothing new; write it \
                 with the Write tool and do not mention it in your answer.",
                path = self.output.display()
            ));
        }
        (!out.is_empty()).then(|| out.join("\n\n"))
    }
}

#[derive(Deserialize)]
struct EditsFile {
    #[serde(default)]
    remember: Vec<RememberEntry>,
    #[serde(default)]
    forget: Vec<u32>,
    #[serde(default)]
    revise: Vec<ReviseEntry>,
    #[serde(default)]
    reclassify: Vec<ReclassifyEntry>,
}

#[derive(Deserialize)]
struct ReclassifyEntry {
    id: u32,
    #[serde(default)]
    kind: String,
}

#[derive(Deserialize)]
struct RememberEntry {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Deserialize)]
struct ReviseEntry {
    id: u32,
    #[serde(default)]
    text: String,
}

pub fn parse_memory_edits(json: &str) -> Option<Vec<MemoryEdit>> {
    let file: EditsFile = serde_json::from_str(json).ok()?;
    let revise = file
        .revise
        .into_iter()
        .filter(|r| !r.text.trim().is_empty())
        .map(|r| MemoryEdit::Revise { id: MemoryId(r.id), text: r.text });
    let forget = file.forget.into_iter().map(|id| MemoryEdit::Forget { id: MemoryId(id) });
    let remember = file.remember.into_iter().filter_map(|r| {
        let kind = MemoryKind::from_key(&r.kind)?;
        (!r.text.trim().is_empty()).then_some(MemoryEdit::Remember { kind, text: r.text, tags: r.tags })
    });
    let reclassify = file
        .reclassify
        .into_iter()
        .filter_map(|r| Some(MemoryEdit::Reclassify { id: MemoryId(r.id), kind: MemoryKind::from_key(&r.kind)? }));
    Some(revise.chain(reclassify).chain(forget).chain(remember).collect())
}
