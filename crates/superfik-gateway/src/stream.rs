//! Chat Completions chunks → the events of a Messages stream. Pure: one
//! [`ChatStream`] per answer, fed chunk by chunk.

use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub name: &'static str,
    pub data: Value,
}

impl Event {
    fn new(name: &'static str, data: Value) -> Self {
        Self { name, data }
    }

    pub fn to_sse(&self) -> String {
        format!("event: {}\ndata: {}\n\n", self.name, self.data)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Open {
    Thinking,
    Text,
    Tool(u64),
}

#[derive(Debug)]
pub struct ChatStream {
    model: String,
    id: String,
    open: Option<(Open, usize)>,
    next: usize,
    used_tools: bool,
    finish: Option<String>,
    input_tokens: u64,
    output_tokens: u64,
}

impl ChatStream {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
            open: None,
            next: 0,
            used_tools: false,
            finish: None,
            input_tokens: 0,
            output_tokens: 0,
        }
    }

    pub fn start(&self) -> Vec<Event> {
        vec![Event::new(
            "message_start",
            json!({
                "type": "message_start",
                "message": {
                    "id": self.id,
                    "type": "message",
                    "role": "assistant",
                    "model": self.model,
                    "content": [],
                    "stop_reason": null,
                    "stop_sequence": null,
                    "usage": { "input_tokens": 0, "output_tokens": 0 },
                },
            }),
        )]
    }

    pub fn push(&mut self, chunk: &Value) -> Vec<Event> {
        let mut events = Vec::new();
        if let Some(usage) = chunk.get("usage").filter(|u| u.is_object()) {
            self.input_tokens = usage["prompt_tokens"].as_u64().unwrap_or(self.input_tokens);
            self.output_tokens = usage["completion_tokens"].as_u64().unwrap_or(self.output_tokens);
        }
        let Some(choice) = chunk["choices"].get(0) else {
            return events;
        };
        let delta = &choice["delta"];
        for key in ["reasoning_content", "reasoning"] {
            if let Some(text) = delta[key].as_str().filter(|t| !t.is_empty()) {
                let index = self.open_block(
                    Open::Thinking,
                    &mut events,
                    || json!({ "type": "thinking", "thinking": "", "signature": "" }),
                );
                events.push(block_delta(index, json!({ "type": "thinking_delta", "thinking": text })));
            }
        }
        if let Some(text) = delta["content"].as_str().filter(|t| !t.is_empty()) {
            let index = self.open_block(Open::Text, &mut events, || json!({ "type": "text", "text": "" }));
            events.push(block_delta(index, json!({ "type": "text_delta", "text": text })));
        }
        for call in delta["tool_calls"].as_array().into_iter().flatten() {
            self.tool_call(call, &mut events);
        }
        if let Some(reason) = choice["finish_reason"].as_str() {
            self.finish = Some(reason.to_owned());
        }
        events
    }

    pub fn finish(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        self.close(&mut events);
        let stop_reason = match self.finish.as_deref() {
            _ if self.used_tools => "tool_use",
            Some("length") => "max_tokens",
            _ => "end_turn",
        };
        events.push(Event::new(
            "message_delta",
            json!({
                "type": "message_delta",
                "delta": { "stop_reason": stop_reason, "stop_sequence": null },
                "usage": { "input_tokens": self.input_tokens, "output_tokens": self.output_tokens },
            }),
        ));
        events.push(Event::new("message_stop", json!({ "type": "message_stop" })));
        events
    }

    fn tool_call(&mut self, call: &Value, events: &mut Vec<Event>) {
        let slot = call["index"].as_u64().unwrap_or(0);
        if self.open.map(|(open, _)| open) != Some(Open::Tool(slot)) {
            let id = call["id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("toolu_{}", uuid::Uuid::new_v4().simple()));
            let name = call["function"]["name"].as_str().unwrap_or_default().to_owned();
            self.used_tools = true;
            self.open_block(
                Open::Tool(slot),
                events,
                || json!({ "type": "tool_use", "id": id, "name": name, "input": {} }),
            );
        }
        let Some((_, index)) = self.open else { return };
        if let Some(arguments) = call["function"]["arguments"].as_str().filter(|a| !a.is_empty()) {
            events.push(block_delta(index, json!({ "type": "input_json_delta", "partial_json": arguments })));
        }
    }

    fn open_block(&mut self, kind: Open, events: &mut Vec<Event>, block: impl FnOnce() -> Value) -> usize {
        if let Some((open, index)) = self.open
            && open == kind
        {
            return index;
        }
        self.close(events);
        let index = self.next;
        self.next += 1;
        self.open = Some((kind, index));
        events.push(Event::new(
            "content_block_start",
            json!({ "type": "content_block_start", "index": index, "content_block": block() }),
        ));
        index
    }

    fn close(&mut self, events: &mut Vec<Event>) {
        if let Some((_, index)) = self.open.take() {
            events.push(Event::new("content_block_stop", json!({ "type": "content_block_stop", "index": index })));
        }
    }
}

fn block_delta(index: usize, delta: Value) -> Event {
    Event::new("content_block_delta", json!({ "type": "content_block_delta", "index": index, "delta": delta }))
}

pub fn message_from(events: &[Event]) -> Value {
    let mut message = json!({});
    let mut content: Vec<Value> = Vec::new();
    let mut partial: Vec<String> = Vec::new();
    for event in events {
        let data = &event.data;
        match event.name {
            "message_start" => message = data["message"].clone(),
            "content_block_start" => {
                content.push(data["content_block"].clone());
                partial.push(String::new());
            }
            "content_block_delta" => {
                let (Some(block), Some(json)) = (content.last_mut(), partial.last_mut()) else { continue };
                let delta = &data["delta"];
                match delta["type"].as_str() {
                    Some("text_delta") => append(&mut block["text"], &delta["text"]),
                    Some("thinking_delta") => append(&mut block["thinking"], &delta["thinking"]),
                    Some("input_json_delta") => json.push_str(delta["partial_json"].as_str().unwrap_or_default()),
                    _ => {}
                }
            }
            "content_block_stop" => {
                if let (Some(block), Some(json)) = (content.last_mut(), partial.last())
                    && block["type"] == "tool_use"
                {
                    block["input"] = serde_json::from_str(json).unwrap_or_else(|_| json!({}));
                }
            }
            "message_delta" => {
                message["stop_reason"] = data["delta"]["stop_reason"].clone();
                message["usage"] = data["usage"].clone();
            }
            _ => {}
        }
    }
    message["content"] = Value::Array(content);
    message
}

fn append(target: &mut Value, text: &Value) {
    let joined = format!("{}{}", target.as_str().unwrap_or_default(), text.as_str().unwrap_or_default());
    *target = Value::String(joined);
}
