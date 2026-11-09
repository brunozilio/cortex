//! A gateway whose local provider answers every prompt with a fixed text
//! and, when tools are offered, asks to run `echo gateway-ok` first: enough
//! to check a real `claude` against the stream format.
//! `cargo run -p superfik-gateway --example echo`, then use the printed env.

use std::sync::Arc;

use serde_json::json;
use superfik_core::ModelProvider;
use superfik_gateway::messages::{Block, MessagesRequest};
use superfik_gateway::server::{Gateway, Upstream, UpstreamError, Upstreams};
use superfik_gateway::stream::{ChatStream, Event};

struct Echo;

impl Upstream for Echo {
    fn messages(&self, request: &MessagesRequest, emit: &mut dyn FnMut(Event)) -> Result<(), UpstreamError> {
        let ran =
            request.messages.iter().any(|m| m.content.blocks().iter().any(|b| matches!(b, Block::ToolResult { .. })));
        let wants_tool = !ran && request.tools.iter().any(|t| t.name == "Bash");
        let mut stream = ChatStream::new(&request.model);
        let mut events = stream.start();
        let delta = if wants_tool {
            json!({ "tool_calls": [{ "index": 0, "id": "call_1", "function": { "name": "Bash", "arguments": "{\"command\":\"echo gateway-ok\",\"description\":\"check\"}" } }] })
        } else {
            json!({ "content": if ran { "The tool ran." } else { "Hello from the gateway." } })
        };
        events.extend(stream.push(&json!({ "choices": [{ "delta": delta, "finish_reason": "stop" }] })));
        events.extend(stream.finish());
        events.into_iter().for_each(emit);
        Ok(())
    }
}

fn main() -> std::io::Result<()> {
    let gateway = Gateway::start(Upstreams::new().with(ModelProvider::Local, Arc::new(Echo)))?;
    let access = gateway.admit(ModelProvider::Local);
    println!("ANTHROPIC_BASE_URL={} ANTHROPIC_AUTH_TOKEN={}", access.base_url, access.token);
    loop {
        std::thread::park();
    }
}
