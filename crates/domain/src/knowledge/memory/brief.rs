use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::decision_note::DecisionNote;
use super::{Memory, MemoryEdit, MemoryId, MemoryKind};

pub const MEMORY_SERVER: &str = concat!(brand::command!(), "_memory");
pub const MEMORY_SEARCH_TOOL: &str = "memory_search";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryBrief {
    #[serde(default)]
    pub recalled: Vec<Memory>,
    #[serde(default)]
    pub output: PathBuf,
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(default)]
    pub tool: bool,
    #[serde(default)]
    pub optimizing: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchored: Vec<Memory>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub anchors: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub structured: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub held_out: bool,
}

impl MemoryBrief {
    pub fn new(origin: Option<String>) -> Self {
        Self { origin, ..Self::default() }
    }

    pub fn recall_notes(&self, fresh: bool) -> Option<String> {
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
                    listing.push_str(&format!("\n- {}", super::anchors::line(memory)));
                }
            }
            out.push(listing);
        }
        if fresh && self.tool {
            out.push(format!(
                "This project's memory holds more than is shown here. Search it with the `{MEMORY_SEARCH_TOOL}` \
                 tool when the user refers to earlier work or asks you to recall something, or when the task \
                 hinges on a past decision or convention. Search once per topic: a search that finds nothing is \
                 final, so carry on with the task."
            ));
        }
        (!out.is_empty()).then(|| out.join("\n\n"))
    }

    fn keeps_edits(&self) -> bool {
        !self.output.as_os_str().is_empty()
    }

    pub fn edits_format(&self) -> Option<String> {
        if !self.keeps_edits() {
            return None;
        }
        let mut out = vec![
            "# Project memory\nThis app keeps the project's memory; do not write memory files of your own. When \
             you are done, if the turn taught you something worth keeping for future sessions, write a JSON file \
             at the path the turn's reminder names, with any of these fields: {\"remember\": [{\"kind\": \
             \"episodic\" or \"semantic\" or \"procedural\", \"text\": \"<one or two sentences>\", \"tags\": \
             [\"<topic>\"]}], \"forget\": [<id>], \"revise\": [{\"id\": <id>, \"text\": \"<corrected text>\"}], \
             \"reclassify\": [{\"id\": <id>, \"kind\": \"<kind>\"}]}. Episodic: what happened in this turn, at \
             most one. Semantic: a durable fact about the project or the user. Procedural: how to do something \
             here (commands, workflows). Reuse existing tags. Forget or revise memories that turned out wrong. \
             Skip the file when there is nothing new; write it with the Write tool and do not mention it in your \
             answer."
                .to_owned(),
        ];
        if self.anchors {
            out.push(
                "A remember entry may add \"touches\": [\"<file or folder, relative to the project>\"]: the \
                 files the memory is about. Later sessions are shown it when they read or edit them, so name \
                 them whenever a memory is about specific code."
                    .to_owned(),
            );
        }
        if self.structured {
            out.push(
                "To keep a decision, write a remember entry with \"decided\": \"<what was decided and why>\", \
                 \"ruled_out\": [\"<what was tried or considered, and why it was dropped>\"] and \
                 \"watch_out\": [\"<a trap to keep in mind>\"] in place of \"text\". What was ruled out is \
                 what saves the next session from trying it again."
                    .to_owned(),
            );
        }
        Some(out.join("\n\n"))
    }

    pub fn edits_reminder(&self) -> Option<String> {
        self.keeps_edits().then(|| {
            format!(
                "Memory edits for this turn, if any, go to {} (format under Project memory).",
                self.output.display()
            )
        })
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
    #[serde(default)]
    touches: Vec<String>,
    #[serde(default)]
    decided: String,
    #[serde(default)]
    ruled_out: Vec<String>,
    #[serde(default)]
    watch_out: Vec<String>,
}

impl RememberEntry {
    fn edit(self) -> Option<MemoryEdit> {
        let decision = DecisionNote::new(&self.decided, &self.ruled_out, &self.watch_out);
        let kind = match (MemoryKind::from_key(&self.kind), &decision) {
            (Some(kind), _) => kind,
            (None, Some(_)) => MemoryKind::Semantic,
            (None, None) => return None,
        };
        let text = decision.map_or(self.text, |decision| decision.text());
        (!text.trim().is_empty()).then_some(MemoryEdit::Remember { kind, text, tags: self.tags, touches: self.touches })
    }
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
    let remember = file.remember.into_iter().filter_map(RememberEntry::edit);
    let reclassify = file
        .reclassify
        .into_iter()
        .filter_map(|r| Some(MemoryEdit::Reclassify { id: MemoryId(r.id), kind: MemoryKind::from_key(&r.kind)? }));
    Some(revise.chain(reclassify).chain(forget).chain(remember).collect())
}
