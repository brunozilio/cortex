//! Server-sent events: lines in, `(event, data)` pairs out.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
}

#[derive(Debug, Default)]
pub struct SseDecoder {
    event: String,
    data: Vec<String>,
}

impl SseDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn line(&mut self, line: &str) -> Option<SseEvent> {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            return self.finish();
        }
        if line.starts_with(':') {
            return None;
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => self.event = value.to_owned(),
            "data" => self.data.push(value.to_owned()),
            _ => {}
        }
        None
    }

    pub fn finish(&mut self) -> Option<SseEvent> {
        if self.data.is_empty() {
            self.event.clear();
            return None;
        }
        let event = SseEvent { event: std::mem::take(&mut self.event), data: self.data.join("\n") };
        self.data.clear();
        Some(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(text: &str) -> Vec<SseEvent> {
        let mut decoder = SseDecoder::new();
        let mut events: Vec<SseEvent> = text.lines().filter_map(|line| decoder.line(line)).collect();
        events.extend(decoder.finish());
        events
    }

    #[test]
    fn names_events_and_joins_data() {
        let events = decode("event: ping\ndata: {}\n\n: comment\ndata: a\ndata: b\r\n\ndata:last");
        assert_eq!(
            events,
            vec![
                SseEvent { event: "ping".into(), data: "{}".into() },
                SseEvent { event: String::new(), data: "a\nb".into() },
                SseEvent { event: String::new(), data: "last".into() },
            ]
        );
    }

    #[test]
    fn a_blank_line_without_data_is_nothing() {
        assert!(decode("event: x\n\n\n").is_empty());
    }
}
