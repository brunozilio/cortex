use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde_json::{Value, json};

use super::McpServer;
use crate::model::ToolSpec;
use crate::tools::{Access, Outcome, Tool, Workspace, clip, field, required};

const SHOWN: usize = 60_000;

pub fn resource_tools(servers: &[Arc<McpServer>]) -> Vec<Box<dyn Tool>> {
    let with: Vec<Arc<McpServer>> = servers.iter().filter(|server| server.offers("resources")).cloned().collect();
    if with.is_empty() {
        return Vec::new();
    }
    vec![Box::new(ListResources(with.clone())), Box::new(ReadResource(with))]
}

fn server<'a>(servers: &'a [Arc<McpServer>], name: &str) -> Result<&'a Arc<McpServer>, String> {
    servers.iter().find(|server| server.name() == name).ok_or_else(|| {
        let names: Vec<&str> = servers.iter().map(|server| server.name()).collect();
        format!("No MCP server {name} with resources; these have some: {}", names.join(", "))
    })
}

pub struct ListResources(Vec<Arc<McpServer>>);

impl std::fmt::Debug for ListResources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ListMcpResources")
    }
}

impl Tool for ListResources {
    fn spec(&self) -> ToolSpec {
        let names: Vec<&str> = self.0.iter().map(|server| server.name()).collect();
        ToolSpec {
            name: "ListMcpResources".into(),
            description: format!(
                "List the resources (files, records, documents) MCP servers offer: {}. Read one with \
                 ReadMcpResource.",
                names.join(", ")
            ),
            schema: json!({
                "type": "object",
                "properties": { "server": { "type": "string", "description": "The server whose resources to list." } },
                "required": ["server"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Executes
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "server").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let wanted = match required(input, "server") {
            Ok(name) => name,
            Err(error) => return error,
        };
        if let Err(error) = server(&self.0, wanted) {
            return Outcome::error(error);
        }
        let mut out = String::new();
        for server in self.0.iter().filter(|server| server.name() == wanted) {
            match server.ask("resources/list", json!({}), &workspace.stop) {
                Ok(listed) => {
                    for resource in listed["resources"].as_array().into_iter().flatten() {
                        let uri = resource["uri"].as_str().unwrap_or_default();
                        let name = resource["name"].as_str().unwrap_or(uri);
                        let about = resource["description"].as_str().unwrap_or_default();
                        out.push_str(&format!("{}: {name} <{uri}> {about}\n", server.name()));
                    }
                }
                Err(error) => out.push_str(&format!("{}: could not list: {error}\n", server.name())),
            }
        }
        if out.is_empty() { Outcome::ok("No resources.") } else { Outcome::ok(clip(out.trim_end(), SHOWN)) }
    }
}

pub struct ReadResource(Vec<Arc<McpServer>>);

impl std::fmt::Debug for ReadResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReadMcpResource")
    }
}

impl Tool for ReadResource {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "ReadMcpResource".into(),
            description: "Read a resource of an MCP server by its URI (ListMcpResources lists them).".into(),
            schema: json!({
                "type": "object",
                "properties": { "server": { "type": "string" }, "uri": { "type": "string" } },
                "required": ["server", "uri"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Executes
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "uri").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let (name, uri) = match (required(input, "server"), required(input, "uri")) {
            (Ok(name), Ok(uri)) => (name, uri),
            (Err(error), _) | (_, Err(error)) => return error,
        };
        let server = match server(&self.0, name) {
            Ok(server) => server,
            Err(error) => return Outcome::error(error),
        };
        match server.ask("resources/read", json!({ "uri": uri }), &workspace.stop) {
            Ok(read) => {
                let text: Vec<String> = read["contents"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|content| match content["text"].as_str() {
                        Some(text) => text.to_owned(),
                        None => format!(
                            "[{} bytes of {}]",
                            content["blob"].as_str().map_or(0, str::len),
                            content["mimeType"].as_str().unwrap_or("data")
                        ),
                    })
                    .collect();
                Outcome::ok(clip(&text.join("\n\n"), SHOWN))
            }
            Err(error) => Outcome::error(error),
        }
    }
}

pub fn expand_prompt(servers: &[Arc<McpServer>], typed: &str, stop: &AtomicBool) -> Option<Result<String, String>> {
    let rest = typed.trim().strip_prefix("/mcp__")?;
    let (name, args) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let (server_name, prompt) = name.split_once("__")?;
    let server = servers.iter().find(|server| server.name() == server_name && server.offers("prompts"))?;
    Some(prompt_text(server, prompt, args.trim(), stop))
}

fn prompt_text(server: &McpServer, prompt: &str, args: &str, stop: &AtomicBool) -> Result<String, String> {
    let listed = server.ask("prompts/list", json!({}), stop)?;
    let declared = listed["prompts"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|found| found["name"] == prompt)
        .ok_or_else(|| format!("{} has no prompt {prompt}", server.name()))?;
    let names: Vec<&str> =
        declared["arguments"].as_array().into_iter().flatten().filter_map(|arg| arg["name"].as_str()).collect();
    let mut words = args.split_whitespace().collect::<Vec<_>>();
    let mut arguments = serde_json::Map::new();
    for (at, name) in names.iter().enumerate() {
        if words.is_empty() {
            break;
        }
        let value =
            if at + 1 == names.len() { std::mem::take(&mut words).join(" ") } else { words.remove(0).to_owned() };
        arguments.insert((*name).to_owned(), Value::String(value));
    }
    let got = server.ask("prompts/get", json!({ "name": prompt, "arguments": arguments }), stop)?;
    let text: Vec<&str> = got["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|message| message["content"]["text"].as_str())
        .collect();
    if text.is_empty() {
        return Err(format!("{}'s prompt {prompt} came back empty", server.name()));
    }
    Ok(text.join("\n\n"))
}

#[cfg(test)]
#[path = "extras_tests.rs"]
mod tests;
