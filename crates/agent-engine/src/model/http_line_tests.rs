use std::collections::VecDeque;
use std::io::{self, BufRead, Read};
use std::sync::atomic::AtomicBool;

use super::read_line_stopped;
use crate::model::ModelError;

enum Step {
    Bytes(&'static [u8]),
    Wait,
}

struct Chunked {
    steps: VecDeque<Step>,
    current: &'static [u8],
}

impl Chunked {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self { steps: steps.into_iter().collect(), current: &[] }
    }
}

impl Read for Chunked {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let data = self.fill_buf()?;
        let count = data.len().min(out.len());
        out[..count].copy_from_slice(&data[..count]);
        self.consume(count);
        Ok(count)
    }
}

impl BufRead for Chunked {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.current.is_empty() {
            match self.steps.pop_front() {
                Some(Step::Bytes(bytes)) => self.current = bytes,
                Some(Step::Wait) => return Err(io::Error::new(io::ErrorKind::TimedOut, "waiting for stream")),
                None => {}
            }
        }
        Ok(self.current)
    }

    fn consume(&mut self, count: usize) {
        self.current = &self.current[count.min(self.current.len())..];
    }
}

#[test]
fn a_character_split_across_a_pause_in_the_stream_arrives_whole() {
    let text = "data: café\n".as_bytes();
    let (head, tail) = text.split_at(text.len() - 2);
    let mut body = Chunked::new([Step::Bytes(head), Step::Wait, Step::Bytes(tail), Step::Bytes(b"next\n")]);
    let mut line = String::new();
    let read = read_line_stopped(&mut body, &mut line, &AtomicBool::new(false)).expect("line");
    assert_eq!((line.as_str(), read), ("data: café\n", text.len()));
    line.clear();
    read_line_stopped(&mut body, &mut line, &AtomicBool::new(false)).expect("next line");
    assert_eq!(line, "next\n");
}

#[test]
fn text_already_in_the_line_is_kept_and_only_the_new_bytes_are_counted() {
    let mut body = Chunked::new([Step::Bytes(b"one\ntw"), Step::Wait, Step::Bytes(b"o")]);
    let mut text = String::from("kept ");
    let stop = AtomicBool::new(false);
    assert_eq!(read_line_stopped(&mut body, &mut text, &stop).expect("first"), 4);
    assert_eq!(read_line_stopped(&mut body, &mut text, &stop).expect("second"), 3);
    assert_eq!(read_line_stopped(&mut body, &mut text, &stop).expect("end"), 0);
    assert_eq!(text, "kept one\ntwo");
}

#[test]
fn bytes_that_are_not_text_fail_the_read_and_a_stop_cancels_it() {
    let mut body = Chunked::new([Step::Bytes(b"\xff\xfe\n")]);
    let read = read_line_stopped(&mut body, &mut String::new(), &AtomicBool::new(false));
    assert!(matches!(read, Err(ModelError::Network(_))), "{read:?}");
    let mut waiting = Chunked::new([Step::Wait]);
    let stopped = read_line_stopped(&mut waiting, &mut String::new(), &AtomicBool::new(true));
    assert_eq!(stopped, Err(ModelError::Cancelled));
}
