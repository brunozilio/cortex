use std::io::Write;

use app::EngineEvent;
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
            EngineEvent::Notice(text) if self.format == Format::Text => {
                eprintln!("\x1b[2m{text}\x1b[0m");
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

    pub fn note(&mut self, text: &str) {
        match self.format {
            Format::Text => eprintln!("\x1b[2m{text}\x1b[0m"),
            Format::StreamJson => {
                let _ = writeln!(self.out, "{}", json!({ "type": "note", "text": text }));
            }
            Format::Json => {}
        }
    }

    pub fn push_text(&mut self, text: &str) {
        self.text.push_str(text);
        if self.format == Format::Text {
            let _ = writeln!(self.out, "{text}");
        }
    }

    pub fn abandon(&mut self, error: &str) {
        if self.format != Format::Text {
            self.error = Some(error.to_owned());
            self.finish(false);
        }
    }

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

pub fn prompts(lines: impl Iterator<Item = std::io::Result<String>>) -> impl Iterator<Item = Result<String, String>> {
    let mut ended = false;
    lines
        .enumerate()
        .map_while(move |(at, line)| {
            if ended {
                return None;
            }
            let read = line
                .map_err(|e| format!("input line {}: could not be read: {e}", at + 1))
                .and_then(|line| prompt(&line).map_err(|e| format!("input line {}: {e}", at + 1)));
            ended = read.is_err();
            Some(read.transpose())
        })
        .flatten()
}

fn prompt(line: &str) -> Result<Option<String>, String> {
    if line.trim().is_empty() {
        return Ok(None);
    }
    let value: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    if value["type"] != "user" {
        return Ok(None);
    }
    match &value["message"]["content"] {
        serde_json::Value::String(text) => Ok(Some(text.clone())),
        serde_json::Value::Array(blocks) => {
            Ok(Some(blocks.iter().filter_map(|block| block["text"].as_str()).collect::<Vec<_>>().join("\n")))
        }
        _ => Err("message.content is not text".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use app::SessionId;

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
    fn a_run_that_could_not_go_on_still_ends_with_a_failed_result_in_json() {
        let abandoned = |format: Format| {
            let mut out = Vec::new();
            Output::new(format, &mut out).abandon("no model is available");
            String::from_utf8(out).expect("utf8")
        };
        let result: serde_json::Value = serde_json::from_str(&abandoned(Format::Json)).expect("one object");
        assert_eq!(result["is_error"], true);
        assert_eq!(result["subtype"], "error");
        assert_eq!(result["error"], "no model is available");
        let streamed = abandoned(Format::StreamJson);
        let last: serde_json::Value = serde_json::from_str(streamed.lines().last().expect("a line")).expect("json");
        assert_eq!(last["type"], "result");
        assert_eq!(last["is_error"], true);
        assert_eq!(abandoned(Format::Text), "", "text leaves the error to the caller's stderr line");
    }

    #[test]
    fn stream_json_input_gives_the_user_s_prompts() {
        let lines = [
            r#"{"type":"user","message":{"content":"first"}}"#,
            "",
            r#"{"type":"user","message":{"content":[{"type":"text","text":"second"}]}}"#,
            r#"{"type":"control","x":1}"#,
        ];
        let read: Vec<_> = prompts(lines.iter().map(|l| Ok((*l).to_owned()))).collect();
        assert_eq!(read, [Ok("first".to_owned()), Ok("second".to_owned())]);
        let bad: Vec<_> = prompts([Ok("not json".to_owned())].into_iter()).collect();
        assert!(matches!(bad.as_slice(), [Err(message)] if message.starts_with("input line 1:")), "{bad:?}");
    }

    #[test]
    fn each_prompt_is_given_as_soon_as_its_line_arrives() {
        let pulled = std::cell::Cell::new(0);
        let lines =
            [r#"{"type":"user","message":{"content":"first"}}"#, r#"{"type":"user","message":{"content":"x"}}"#];
        let mut read = prompts(lines.iter().map(|l| Ok((*l).to_owned())).inspect(|_| pulled.set(pulled.get() + 1)));
        assert_eq!(read.next(), Some(Ok("first".to_owned())));
        assert_eq!(pulled.get(), 1, "the second line was not waited for");
    }

    #[test]
    fn a_line_that_cannot_be_read_ends_the_input_with_an_error() {
        let lines = vec![
            Ok(r#"{"type":"user","message":{"content":"first"}}"#.to_owned()),
            Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "stream did not contain valid UTF-8")),
            Ok(r#"{"type":"user","message":{"content":"never"}}"#.to_owned()),
        ];
        let read: Vec<_> = prompts(lines.into_iter()).collect();
        assert_eq!(read.len(), 2, "{read:?}");
        assert_eq!(read[0], Ok("first".to_owned()));
        assert!(matches!(&read[1], Err(message) if message.contains("input line 2") && message.contains("UTF-8")));
    }
}
