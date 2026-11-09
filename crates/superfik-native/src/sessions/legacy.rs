//! Conversations of the desktop's earlier CLI engine, read from their
//! JSONL transcripts directly. No CLI process is involved.

use std::collections::HashMap;

use superfik_core::{PromptRequest, SessionId};

use crate::transcript::Message;

pub(super) fn legacy(request: &PromptRequest, id: &SessionId) -> Result<Option<Vec<Message>>, String> {
    if id.0.is_empty() || !id.0.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err("invalid legacy session id".into());
    }
    let Ok(cwd) = request.cwd.canonicalize() else {
        return Ok(None);
    };
    let Some(cwd) = cwd.to_str() else {
        return Ok(None);
    };
    let project: String = cwd.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let Some(home) = std::env::home_dir() else {
        return Ok(None);
    };
    let path = home.join(".claude/projects").join(project).join(format!("{}.jsonl", id.0));
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("legacy conversation could not be read: {e}")),
    };
    import_jsonl(&text).map(Some)
}

pub fn import_jsonl(text: &str) -> Result<Vec<Message>, String> {
    use crate::transcript::{Block, Role};
    use serde_json::Value;
    let records: Vec<Value> = text
        .lines()
        .filter(|s| !s.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("invalid transcript: {e}"))?;
    let mut selected = std::collections::HashSet::new();
    let branched = records.iter().any(|r| r.get("parentUuid").is_some());
    if branched {
        let nodes: HashMap<_, _> = records.iter().filter_map(|r| Some((r["uuid"].as_str()?, r))).collect();
        let mut current = records
            .iter()
            .rev()
            .find(|r| r["isSidechain"] != true && matches!(r["type"].as_str(), Some("user" | "assistant")))
            .and_then(|r| r["uuid"].as_str());
        while let Some(uuid) = current {
            if !selected.insert(uuid.to_owned()) {
                return Err("cycle in transcript ancestry".into());
            }
            current = nodes.get(uuid).and_then(|r| r["parentUuid"].as_str());
        }
    }
    let mut messages = Vec::new();
    for (line, raw) in text.lines().enumerate().filter(|(_, s)| !s.trim().is_empty()) {
        let record: Value = serde_json::from_str(raw).map_err(|e| format!("transcript line {}: {e}", line + 1))?;
        if record["isSidechain"] == true {
            continue;
        }
        if branched && !record["uuid"].as_str().is_some_and(|id| selected.contains(id)) {
            continue;
        }
        let role = match record["type"].as_str() {
            Some("user") => Role::User,
            Some("assistant") => Role::Assistant,
            _ => continue,
        };
        let content = &record["message"]["content"];
        let mut blocks = Vec::new();
        if let Some(text) = content.as_str() {
            blocks.push(Block::Text(text.into()));
        } else if let Some(parts) = content.as_array() {
            for part in parts {
                let required = |key: &str| {
                    part[key].as_str().map(str::to_owned).ok_or_else(|| format!("missing {key} at line {}", line + 1))
                };
                blocks.push(match part["type"].as_str() {
                    Some("text") => Block::Text(required("text")?),
                    Some("thinking") => Block::Thinking {
                        text: required("thinking")?,
                        signature: part["signature"].as_str().map(str::to_owned),
                        origin: None,
                    },
                    Some("tool_use") => {
                        Block::ToolUse { id: required("id")?, name: required("name")?, input: part["input"].clone() }
                    }
                    Some("tool_result") => {
                        let content = &part["content"];
                        let mut texts = Vec::new();
                        let mut images = Vec::new();
                        if let Some(text) = content.as_str() {
                            texts.push(text.to_owned());
                        } else if let Some(parts) = content.as_array() {
                            for item in parts {
                                match item["type"].as_str() {
                                    Some("text") => {
                                        texts.push(item["text"].as_str().ok_or("invalid text result")?.into())
                                    }
                                    Some("image") => images.push(import_image(&item["source"])?),
                                    Some("tool_reference") => texts.push(format!(
                                        "Previously discovered tool: {}",
                                        item["tool_name"].as_str().ok_or("missing legacy tool name")?
                                    )),
                                    _ => return Err(format!("unsupported tool result at line {}", line + 1)),
                                }
                            }
                        }
                        let id = required("tool_use_id")?;
                        let content = texts.join("\n");
                        let is_error = part["is_error"].as_bool().unwrap_or(false);
                        if images.is_empty() {
                            Block::ToolResult { id, content, is_error }
                        } else {
                            Block::ToolResultRich { id, content, images, is_error }
                        }
                    }
                    Some("image") => Block::Image(import_image(&part["source"])?),
                    Some("redacted_thinking") => continue,
                    _ => return Err(format!("unsupported transcript block at line {}", line + 1)),
                });
            }
        } else {
            return Err(format!("missing transcript content at line {}", line + 1));
        }
        messages.push(Message { role, blocks });
    }
    if messages.is_empty() {
        return Err("no conversation in the transcript".into());
    }
    Ok(messages)
}
fn import_image(source: &serde_json::Value) -> Result<crate::image::Image, String> {
    crate::image::Image::from_mcp(
        source["media_type"].as_str().ok_or("missing image type")?,
        source["data"].as_str().ok_or("missing image data")?,
    )
}
