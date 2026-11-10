//! A server's tool as the model's: its access, its step summary, and its
//! result turned into an [`Outcome`].

use std::sync::Arc;

use serde_json::Value;

use super::McpServer;
use crate::model::ToolSpec;
use crate::tools::{Access, Outcome, Tool, Workspace, field};

pub(super) fn outcome(result: &Value) -> Outcome {
    let mut text = Vec::new();
    let mut images = Vec::new();
    let mut problems = Vec::new();
    for part in result["content"].as_array().into_iter().flatten() {
        match part["type"].as_str() {
            Some("text") => {
                if let Some(value) = part["text"].as_str() {
                    text.push(value.to_owned());
                }
            }
            Some("image") => match crate::image::Image::from_mcp(
                part["mimeType"].as_str().unwrap_or_default(),
                part["data"].as_str().unwrap_or_default(),
            ) {
                Ok(image) => images.push(image),
                Err(error) => problems.push(error),
            },
            Some("resource") => {
                let resource = &part["resource"];
                let uri = resource["uri"].as_str().unwrap_or_default();
                if let Some(value) = resource["text"].as_str() {
                    text.push(format!("Resource {uri}:\n{value}"));
                } else if let Some(blob) = resource["blob"].as_str() {
                    let mime = resource["mimeType"].as_str().unwrap_or("application/octet-stream");
                    if mime.starts_with("image/") {
                        match crate::image::Image::from_mcp(mime, blob) {
                            Ok(image) => images.push(image),
                            Err(error) => problems.push(error),
                        }
                    } else if mime.starts_with("text/") || mime == "application/json" {
                        use base64::Engine;
                        match base64::engine::general_purpose::STANDARD
                            .decode(blob)
                            .ok()
                            .and_then(|b| String::from_utf8(b).ok())
                        {
                            Some(value) => text.push(format!("Resource {uri}:\n{value}")),
                            None => problems.push(format!("Invalid text resource {uri}")),
                        }
                    } else {
                        problems.push(format!("Unsupported binary resource {uri} ({mime})"));
                    }
                } else {
                    problems.push(format!("Resource {uri} has no text or blob"));
                }
            }
            Some("resource_link") => {
                if let Some(uri) = part["uri"].as_str() {
                    text.push(format!(
                        "Resource link: {} ({uri})\n{}",
                        part["name"].as_str().unwrap_or(uri),
                        part["description"].as_str().unwrap_or_default()
                    ));
                } else {
                    problems.push("Resource link has no URI".into());
                }
            }
            Some(kind) => problems.push(format!("Unsupported MCP content type: {kind}")),
            None => problems.push("MCP content part has no type".into()),
        }
    }
    if let Some(structured) = result.get("structuredContent") {
        let formatted = serde_json::to_string_pretty(structured).unwrap_or_else(|_| structured.to_string());
        if !text.iter().any(|part| serde_json::from_str::<Value>(part).is_ok_and(|value| value == *structured)) {
            text.push(format!("Structured result:\n{formatted}"));
        }
    }
    let failed = result["isError"].as_bool().unwrap_or(false) || !problems.is_empty();
    text.extend(problems);
    if text.is_empty() {
        text.push(if images.is_empty() {
            "(tool returned no content)".into()
        } else {
            format!("{} image(s) returned", images.len())
        });
    }
    Outcome { content: text.join("\n\n"), is_error: failed, images }
}

pub struct McpTool {
    pub(super) server: Arc<McpServer>,
    pub(super) name: String,
    pub(super) spec: ToolSpec,
}

impl std::fmt::Debug for McpTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpTool").field("name", &self.spec.name).finish_non_exhaustive()
    }
}

impl Tool for McpTool {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn access(&self) -> Access {
        builtin_access(&self.server.name, &self.name)
    }

    fn summary(&self, input: &Value) -> String {
        ["query", "url", "command", "item", "id", "text", "title"]
            .into_iter()
            .find_map(|key| field(input, key))
            .unwrap_or_default()
            .chars()
            .take(80)
            .collect()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        self.server.call(&self.name, input, &workspace.stop)
    }
}

pub fn builtin_access(server: &str, name: &str) -> Access {
    use superfik_core::memory;
    match server {
        memory::MEMORY_SERVER if name == memory::MEMORY_SEARCH_TOOL => Access::Reads,
        _ => Access::Executes,
    }
}
