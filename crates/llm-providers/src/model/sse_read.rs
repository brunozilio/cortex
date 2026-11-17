use std::sync::atomic::{AtomicBool, Ordering};

use super::http::{Response, read_line_stopped};
use super::sse::SseDecoder;
use super::{Delta, ModelError, Reply};

pub trait SseFold {
    fn event(&mut self, data: &str, on: &mut dyn FnMut(Delta)) -> Result<(), ModelError>;
    fn finish(self, on: &mut dyn FnMut(Delta)) -> Result<Reply, ModelError>;
}

pub fn read_sse(
    response: Response,
    mut folder: impl SseFold,
    on: &mut dyn FnMut(Delta),
    stop: &AtomicBool,
) -> Result<Reply, ModelError> {
    let mut decoder = SseDecoder::new();
    let mut body = response.body;
    let mut line = String::new();
    loop {
        if stop.load(Ordering::Relaxed) {
            return Err(ModelError::Cancelled);
        }
        line.clear();
        let read = read_line_stopped(body.as_mut(), &mut line, stop)?;
        let event = if read == 0 { decoder.finish() } else { decoder.line(line.trim_end_matches('\n')) };
        if let Some(event) = event {
            folder.event(&event.data, on)?;
        }
        if read == 0 {
            return folder.finish(on);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::{Block, Message, StopReason, Usage};

    #[derive(Default)]
    struct Collect(Vec<String>);

    impl SseFold for Collect {
        fn event(&mut self, data: &str, on: &mut dyn FnMut(Delta)) -> Result<(), ModelError> {
            on(Delta::Text(data.to_owned()));
            self.0.push(data.to_owned());
            Ok(())
        }
        fn finish(self, _on: &mut dyn FnMut(Delta)) -> Result<Reply, ModelError> {
            let message = Message::assistant(vec![Block::Text(self.0.join(","))]);
            Ok(Reply { message, stop: StopReason::EndTurn, usage: Usage::default() })
        }
    }

    fn response(text: &str) -> Response {
        Response { status: 200, headers: Vec::new(), body: Box::new(std::io::Cursor::new(text.as_bytes().to_vec())) }
    }

    #[test]
    fn every_event_is_folded_until_the_stream_ends() {
        let mut seen = Vec::new();
        let reply = read_sse(
            response("data: a\n\ndata: b\n\ndata: c"),
            Collect::default(),
            &mut |d| seen.push(d),
            &AtomicBool::new(false),
        );
        assert_eq!(reply.expect("reply").message.text(), "a,b,c", "the last event needs no blank line");
        assert_eq!(seen.len(), 3);
    }

    #[test]
    fn a_stopped_run_reads_no_further() {
        let stopped = read_sse(response("data: a\n\n"), Collect::default(), &mut |_| {}, &AtomicBool::new(true));
        assert!(matches!(stopped, Err(ModelError::Cancelled)));
    }
}
