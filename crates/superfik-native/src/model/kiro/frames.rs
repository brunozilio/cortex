//! AWS event-stream framing (`application/vnd.amazon.eventstream`): each
//! message is a 12-byte prelude (total length, headers length, prelude
//! CRC), typed headers, a payload and a message CRC. TLS already guards
//! the bytes, so the CRCs are not checked.

use std::io::{BufRead, ErrorKind};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::model::ModelError;

/// The largest message accepted; Kiro's are a few hundred bytes.
const MAX_MESSAGE: usize = 16 << 20;
const PRELUDE: usize = 12;
const TRAILER: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub headers: Vec<(String, String)>,
    pub payload: Vec<u8>,
}

impl Frame {
    /// A string header's value.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str())
    }
}

/// The next message, or `None` at a clean end of the stream. A body that
/// times out while waiting (so the reader can see `stop`) is read again.
pub fn next(reader: &mut dyn BufRead, stop: &AtomicBool) -> Result<Option<Frame>, ModelError> {
    let mut prelude = [0u8; PRELUDE];
    if fill(reader, &mut prelude, stop)? == 0 {
        return Ok(None);
    }
    let total = u32::from_be_bytes([prelude[0], prelude[1], prelude[2], prelude[3]]) as usize;
    let header_len = u32::from_be_bytes([prelude[4], prelude[5], prelude[6], prelude[7]]) as usize;
    if total > MAX_MESSAGE || total < PRELUDE + TRAILER + header_len {
        return Err(ModelError::Network(format!("invalid event stream message length {total}")));
    }
    let mut rest = vec![0u8; total - PRELUDE];
    fill(reader, &mut rest, stop)?;
    let headers = headers(&rest[..header_len]).map_err(ModelError::Network)?;
    let payload = rest[header_len..rest.len() - TRAILER].to_vec();
    Ok(Some(Frame { headers, payload }))
}

/// Fills `buffer`, or reads nothing at the end of the stream; an end in the
/// middle is a cut stream.
fn fill(reader: &mut dyn BufRead, buffer: &mut [u8], stop: &AtomicBool) -> Result<usize, ModelError> {
    let mut filled = 0;
    while filled < buffer.len() {
        if stop.load(Ordering::Relaxed) {
            return Err(ModelError::Cancelled);
        }
        match reader.read(&mut buffer[filled..]) {
            Ok(0) if filled == 0 => return Ok(0),
            Ok(0) => return Err(ModelError::Network("the event stream was cut short".into())),
            Ok(count) => filled += count,
            Err(error)
                if matches!(error.kind(), ErrorKind::Interrupted | ErrorKind::TimedOut | ErrorKind::WouldBlock) => {}
            Err(error) => return Err(ModelError::Network(error.to_string())),
        }
    }
    Ok(filled)
}

/// The string headers; other types are skipped.
fn headers(mut bytes: &[u8]) -> Result<Vec<(String, String)>, String> {
    let invalid = || "invalid event stream header".to_owned();
    let mut out = Vec::new();
    while let Some((&name_len, rest)) = bytes.split_first() {
        let (name, rest) = split(rest, usize::from(name_len)).ok_or_else(invalid)?;
        let (&kind, rest) = rest.split_first().ok_or_else(invalid)?;
        let value_len = match kind {
            0 | 1 => 0,
            2 => 1,
            3 => 2,
            4 => 4,
            5 | 8 => 8,
            9 => 16,
            6 | 7 => {
                let (len, _) = split(rest, 2).ok_or_else(invalid)?;
                2 + usize::from(u16::from_be_bytes([len[0], len[1]]))
            }
            _ => return Err(invalid()),
        };
        let (value, rest) = split(rest, value_len).ok_or_else(invalid)?;
        if kind == 7 {
            let name = String::from_utf8(name.to_vec()).map_err(|_| invalid())?;
            out.push((name, String::from_utf8(value[2..].to_vec()).map_err(|_| invalid())?));
        }
        bytes = rest;
    }
    Ok(out)
}

fn split(bytes: &[u8], at: usize) -> Option<(&[u8], &[u8])> {
    (bytes.len() >= at).then(|| bytes.split_at(at))
}

#[cfg(test)]
pub(crate) fn encode(headers: &[(&str, &str)], payload: &str) -> Vec<u8> {
    let mut head = Vec::new();
    for (name, value) in headers {
        head.push(name.len() as u8);
        head.extend(name.as_bytes());
        head.push(7);
        head.extend((value.len() as u16).to_be_bytes());
        head.extend(value.as_bytes());
    }
    let total = PRELUDE + head.len() + payload.len() + TRAILER;
    let mut out = Vec::new();
    out.extend((total as u32).to_be_bytes());
    out.extend((head.len() as u32).to_be_bytes());
    out.extend([0; 4]);
    out.extend(head);
    out.extend(payload.as_bytes());
    out.extend([0; 4]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(bytes: &[u8]) -> Result<Vec<Frame>, ModelError> {
        let mut reader = std::io::BufReader::new(bytes);
        let stop = AtomicBool::new(false);
        std::iter::from_fn(|| next(&mut reader, &stop).transpose()).collect()
    }

    /// A body that hands out a few bytes at a time and times out between
    /// them, as the cancellable HTTP body does while it waits.
    struct Trickle(Vec<u8>, usize, bool);

    impl std::io::Read for Trickle {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            self.2 = !self.2;
            if self.2 {
                return Err(std::io::Error::new(ErrorKind::TimedOut, "waiting for stream"));
            }
            let count = out.len().min(3).min(self.0.len() - self.1);
            out[..count].copy_from_slice(&self.0[self.1..self.1 + count]);
            self.1 += count;
            Ok(count)
        }
    }

    #[test]
    fn a_body_that_waits_between_pieces_is_read_whole() {
        let bytes = include_bytes!("testdata/text.bin").to_vec();
        let mut reader = std::io::BufReader::with_capacity(1, Trickle(bytes, 0, false));
        let stop = AtomicBool::new(false);
        let frames: Result<Vec<_>, _> = std::iter::from_fn(|| next(&mut reader, &stop).transpose()).collect();
        assert_eq!(frames.map(|f| f.len()), Ok(9));
        let stopped = AtomicBool::new(true);
        let mut reader = std::io::BufReader::new(Trickle(encode(&[], "{}"), 0, false));
        assert_eq!(next(&mut reader, &stopped), Err(ModelError::Cancelled));
    }

    #[test]
    fn reads_kiros_recorded_stream_frame_by_frame() {
        let frames = all(include_bytes!("testdata/text.bin")).unwrap_or_default();
        let kinds: Vec<_> = frames.iter().filter_map(|f| f.header(":event-type")).collect();
        assert_eq!(kinds[0], "initial-response");
        assert_eq!(kinds.iter().filter(|k| **k == "assistantResponseEvent").count(), 5);
        assert_eq!(kinds.last(), Some(&"meteringEvent"));
        assert!(frames.iter().all(|f| f.header(":message-type") == Some("event")));
    }

    #[test]
    fn skips_headers_of_other_types() {
        let mut bytes = encode(&[(":event-type", "x")], "{}");
        let mut extra = vec![4u8];
        extra.extend(b"flag");
        extra.push(0);
        let header_len = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize + extra.len();
        bytes.splice(PRELUDE..PRELUDE, extra.iter().copied());
        let total = bytes.len() as u32;
        bytes[..4].copy_from_slice(&total.to_be_bytes());
        bytes[4..8].copy_from_slice(&(header_len as u32).to_be_bytes());
        let frames = all(&bytes).unwrap_or_default();
        assert_eq!(frames[0].headers, [(":event-type".to_owned(), "x".to_owned())]);
        assert_eq!(frames[0].payload, b"{}");
    }

    #[test]
    fn a_cut_or_oversized_message_is_an_error() {
        let bytes = encode(&[(":event-type", "x")], "{}");
        assert!(all(&bytes[..bytes.len() - 3]).is_err());
        let mut huge = bytes.clone();
        huge[..4].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(all(&huge).is_err());
        assert_eq!(all(&[]), Ok(Vec::new()));
    }
}
