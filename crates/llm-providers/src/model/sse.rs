use super::ModelError;

pub const EVENT_AT_MOST: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
}

#[derive(Debug)]
pub struct SseDecoder {
    event: String,
    data: Vec<String>,
    held: usize,
    most: usize,
}

impl Default for SseDecoder {
    fn default() -> Self {
        Self::within(EVENT_AT_MOST)
    }
}

impl SseDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn within(most: usize) -> Self {
        Self { event: String::new(), data: Vec::new(), held: 0, most }
    }

    pub fn line(&mut self, line: &str) -> Result<Option<SseEvent>, ModelError> {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            return Ok(self.finish());
        }
        if line.starts_with(':') {
            return Ok(None);
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => self.event = value.to_owned(),
            "data" => {
                self.held += value.len() + 1;
                if self.held > self.most {
                    self.data.clear();
                    self.held = 0;
                    return Err(ModelError::Oversized(self.most));
                }
                self.data.push(value.to_owned());
            }
            _ => {}
        }
        Ok(None)
    }

    pub fn finish(&mut self) -> Option<SseEvent> {
        self.held = 0;
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
        let mut events: Vec<SseEvent> =
            text.lines().filter_map(|line| decoder.line(line).expect("within the cap")).collect();
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

    #[test]
    fn an_event_whose_data_never_ends_is_refused_at_its_cap() {
        let mut decoder = SseDecoder::within(1024);
        let line = format!("data: {}", "x".repeat(100));
        let refused = (0..20).find_map(|_| decoder.line(&line).err());
        assert_eq!(refused, Some(ModelError::Oversized(1024)));
        assert_eq!(decoder.finish(), None, "what was held is let go");
        let mut fresh = SseDecoder::within(1024);
        for _ in 0..9 {
            assert_eq!(fresh.line(&line), Ok(None));
        }
        assert!(fresh.line("").expect("ends").is_some(), "an event under the cap still arrives");
        assert_eq!(SseDecoder::new().most, EVENT_AT_MOST);
    }
}
