use serde_json::{Value, json};

use super::{DRAFT, list_of, map_of, reference, reference_to};

pub fn schema() -> Value {
    json!({
        "$schema": DRAFT,
        "title": "MCP servers",
        "description": "An mcp.json file of the app: the MCP servers a user or a project declares.",
        "type": "object",
        "properties": {
            "$schema": reference(),
            "mcpServers": map_of(reference_to("server"), "MCP servers by name.")
        },
        "additionalProperties": false,
        "$defs": { "server": server(), "sandbox": sandbox() }
    })
}

fn server() -> Value {
    let text = json!({ "type": "string" });
    json!({
        "type": "object",
        "description": "A server with a url is remote; a server with a command is a local process. \
                        Text may read ${NAME} or ${NAME:-default} from the environment.",
        "properties": {
            "type": {
                "enum": ["stdio", "http", "sse"],
                "description": "http (Streamable HTTP), sse (HTTP + SSE) or stdio (a local process)."
            },
            "url": { "type": "string", "description": "The server's address (http and sse)." },
            "headers": map_of(text.clone(), "Request headers (http and sse)."),
            "command": { "type": "string", "description": "The program to start (stdio)." },
            "args": list_of(text.clone()),
            "env": map_of(text, "Environment variables added to the process (stdio)."),
            "sandbox": reference_to("sandbox")
        },
        "additionalProperties": false
    })
}

fn sandbox() -> Value {
    json!({
        "type": ["object", "boolean", "null"],
        "description": "Where the process may write and whether it reaches the network, or false to start it \
                        outside the sandbox.",
        "properties": {
            "writable": list_of(json!({ "type": "string" })),
            "network": { "type": "boolean" }
        },
        "additionalProperties": false
    })
}
