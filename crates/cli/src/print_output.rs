//! How `{command} -p` writes a run: `text` (the reply on stdout, steps on
//! stderr), `json` (one result object at the end) or `stream-json` (every
//! event as a line, then the result).

use std::io::Write;

use sdk::EngineEvent;
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    #[default]
    Text,
    Json,
    StreamJson,
}

impl Format {
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            "stream-json" => Ok(Self::StreamJson),
            other => Err(format!("unknown output format {other}: text, json or stream-json")),
        }
    }
}

/// A run being written out.
pub struct Output<W: Write> {
    format: Format,
    out: W,
    text: String,
    error: Option<String>,
    session: Option<String>,
    cost: f64,
    duration_ms: u64,
}

impl<W: Write> Output<W> {
    pub fn new(format: Format, out: W) -> Self {
        Self { format, out, text: String::new(), error: None, session: None, cost: 0.0, duration_ms: 0 }
    }

    pub fn event(&mut self, event: &EngineEvent) {
        match event {
            EngineEvent::TextDelta(text) => {
                self.text.push_str(text);
                if self.format == Format::Text {
                    let _ = self.out.write_all(text.as_bytes());
                    let _ = self.out.flush();
                }
            }
            EngineEvent::ToolStarted(call) if self.format == Format::Text => {
                eprintln!("\x1b[2m● {}({})\x1b[0m", call.name, call.summary);
            }
            EngineEvent::SessionStarted(id) => self.session = Some(id.0.clone()),
            EngineEvent::Completed { duration_ms, cost_usd } => {
                self.duration_ms += duration_ms;
                self.cost += cost_usd.unwrap_or(0.0);
            }
            EngineEvent::Failed(message) => self.error = Some(message.clone()),
            EngineEvent::Cancelled => self.error = Some("cancelled".into()),
            _ => {}
        }
        if self.format == Format::StreamJson {
            let _ = writeln!(self.out, "{}", json!({ "type": "event", "event": event }));
        }
    }

    /// A note for whoever reads the output (a denied ask, a plan).
    pub fn note(&mut self, text: &str) {
        match self.format {
            Format::Text => eprintln!("\x1b[2m{text}\x1b[0m"),
            Format::StreamJson => {
                let _ = writeln!(self.out, "{}", json!({ "type": "note", "text": text }));
            }
            Format::Json => {}
        }
    }

    /// The text the agent wrote, for a plan shown in place of the reply.
    pub fn push_text(&mut self, text: &str) {
        self.text.push_str(text);
        if self.format == Format::Text {
            let _ = writeln!(self.out, "{text}");
        }
    }

    /// Writes the end (the result object, or a last new line); whether the
    /// run failed.
    pub fn finish(&mut self, failed_check: bool) -> bool {
        let failed = self.error.is_some() || failed_check;
        let result = json!({
            "type": "result",
            "subtype": if failed { "error" } else { "success" },
            "is_error": failed,
            "result": self.text,
            "error": self.error,
            "session_id": self.session,
            "total_cost_usd": self.cost,
            "duration_ms": self.duration_ms,
        });
        match self.format {
            Format::Text => {
                if let Some(error) = &self.error {
                    eprintln!("{brand_command}: {error}", brand_command = brand::COMMAND);
                }
                let _ = writeln!(self.out);
            }
            Format::Json | Format::StreamJson => {
                let _ = writeln!(self.out, "{result}");
            }
        }
        failed
    }
}

/// The prompts of `--input-format stream-json`: one JSON object per line,
/// `{"type":"user","message":{"content":"…"}}` (content as text or as
/// text blocks).
pub fn prompts(lines: impl Iterator<Item = String>) -> Result<Vec<String>, String> {
    let mut prompts = Vec::new();
    for (at, line) in lines.enumerate().filter(|(_, line)| !line.trim().is_empty()) {
        let value: serde_json::Value =
            serde_json::from_str(&line).map_err(|e| format!("input line {}: {e}", at + 1))?;
        if value["type"] != "user" {
            continue;
        }
        let content = &value["message"]["content"];
        let text = match content {
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Array(blocks) => {
                blocks.iter().filter_map(|block| block["text"].as_str()).collect::<Vec<_>>().join("\n")
            }
            _ => return Err(format!("input line {}: message.content is not text", at + 1)),
        };
        prompts.push(text);
    }
    Ok(prompts)
}

#[cfg(test)]
mod tests {
    use sdk::SessionId;

    use super::*;

    fn written(format: Format, events: &[EngineEvent]) -> String {
        let mut out = Vec::new();
        let mut output = Output::new(format, &mut out);
        for event in events {
            output.event(event);
        }
        output.finish(false);
        String::from_utf8(out).expect("utf8")
    }

    fn run() -> Vec<EngineEvent> {
        vec![
            EngineEvent::SessionStarted(SessionId("s-1".into())),
            EngineEvent::TextDelta("Hel".into()),
            EngineEvent::TextDelta("lo".into()),
            EngineEvent::Completed { duration_ms: 7, cost_usd: Some(0.25) },
        ]
    }

    #[test]
    fn json_writes_one_result_at_the_end() {
        let out: serde_json::Value = serde_json::from_str(&written(Format::Json, &run())).expect("one object");
        assert_eq!(out["result"], "Hello");
        assert_eq!(out["session_id"], "s-1");
        assert_eq!(out["is_error"], false);
        assert_eq!(out["total_cost_usd"], 0.25);
    }

    #[test]
    fn stream_json_writes_every_event_then_the_result() {
        let streamed = written(Format::StreamJson, &run());
        let lines: Vec<serde_json::Value> =
            streamed.lines().map(|line| serde_json::from_str(line).expect("json line")).collect();
        assert_eq!(lines.len(), 5);
        assert!(lines[..4].iter().all(|line| line["type"] == "event"));
        assert_eq!(lines[4]["type"], "result");
        assert_eq!(written(Format::Text, &run()), "Hello\n");
    }

    #[test]
    fn stream_json_input_gives_the_user_s_prompts() {
        let lines = [
            r#"{"type":"user","message":{"content":"first"}}"#,
            "",
            r#"{"type":"user","message":{"content":[{"type":"text","text":"second"}]}}"#,
            r#"{"type":"control","x":1}"#,
        ];
        assert_eq!(prompts(lines.iter().map(|l| (*l).to_owned())), Ok(vec!["first".into(), "second".into()]));
        assert!(prompts(["not json".to_owned()].into_iter()).is_err());
    }
}
