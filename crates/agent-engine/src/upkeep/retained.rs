use std::path::{Path, PathBuf};

use super::message_tokens;
use crate::tools::skill::SKILL_TOOL;
use crate::transcript::{Block, Message, Role};

const SKILLS_KEPT_CHARS: usize = 20_000;

fn is_prompt(message: &Message) -> bool {
    message.role == Role::User && message.blocks.iter().all(|b| matches!(b, Block::Text(_) | Block::Image(_)))
}

pub fn split(history: &[Message], budget: u64) -> Option<usize> {
    let mut kept = 0;
    let mut best = None;
    for at in (2..history.len()).rev() {
        kept += message_tokens(&history[at]);
        if kept > budget {
            break;
        }
        if history[at].role == Role::Assistant || is_prompt(&history[at]) {
            best = Some(at);
        }
    }
    best
}

pub fn records(history: &[Message]) -> String {
    let mut text = String::new();
    for message in history {
        text.push_str(&format!("\n{:?}:\n", message.role));
        for block in &message.blocks {
            match block {
                Block::Text(s) | Block::Thinking { text: s, .. } => text.push_str(s),
                Block::Reminder(_) => {}
                Block::ToolUse { id, name, input } => text.push_str(&format!("Tool {id} {name}: {input}")),
                Block::ToolResult { id, content, is_error } | Block::ToolResultRich { id, content, is_error, .. } => {
                    text.push_str(&format!("Result {id} (error={is_error}): {content}"))
                }
                Block::Image(image) => text.push_str(&format!(
                    "[Visual attachment: {}. Re-open the original reference if needed.]",
                    image.media_type
                )),
            }
            text.push('\n');
        }
    }
    text
}

pub fn save_transcript(history: &[Message], dir: &Path) -> Option<PathBuf> {
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let path = dir.join(format!("transcript-{at}.txt"));
    let saved = std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&path, records(history)));
    match saved {
        Ok(()) => Some(path),
        Err(error) => {
            tracing::warn!(%error, "could not save the transcript before compacting");
            None
        }
    }
}

pub fn transcript_pointer(path: &Path) -> String {
    format!(
        "If you need specific details from before compaction (exact code, error messages, or content you wrote), read \
         the full transcript at: {}",
        path.display()
    )
}

pub fn invoked_skills(history: &[Message], kept: &[Message]) -> Option<String> {
    let still_there = |id: &str| {
        kept.iter().flat_map(|m| &m.blocks).any(|b| matches!(b, Block::ToolResult { id: kept, .. } if kept == id))
    };
    let mut skills: Vec<(String, String)> = Vec::new();
    for (at, message) in history.iter().enumerate() {
        for block in &message.blocks {
            let Block::ToolUse { id, name, input } = block else { continue };
            if name != SKILL_TOOL || still_there(id) {
                continue;
            }
            let Some(skill) = input.get("skill").and_then(serde_json::Value::as_str) else { continue };
            let result = history[at..].iter().flat_map(|m| &m.blocks).find_map(|b| match b {
                Block::ToolResult { id: done, content, is_error: false } if done == id => Some(content.clone()),
                _ => None,
            });
            if let Some(content) = result {
                skills.retain(|(name, _)| name != skill);
                skills.push((skill.to_owned(), content));
            }
        }
    }
    if skills.is_empty() {
        return None;
    }
    let mut out = String::from(
        "These skills were invoked earlier in this session, before it was compacted. Keep following their guidance. \
         Do not invoke them again or repeat their one-time setup.",
    );
    for (name, content) in skills {
        let room = SKILLS_KEPT_CHARS.saturating_sub(out.len());
        if room < 200 {
            out.push_str(&format!("\n\n### Skill: {name}\n(not repeated here; invoke it again if you need it)"));
            continue;
        }
        out.push_str(&format!("\n\n### Skill: {name}\n{}", crate::tools::clip(&content, room)));
    }
    Some(out)
}

#[cfg(test)]
#[path = "retained_tests.rs"]
mod tests;
