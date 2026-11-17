#![no_main]

use std::io::{BufRead, Read};

use libfuzzer_sys::fuzz_target;
use llm_providers::model::http::Body;
use llm_providers::model::sse::SseDecoder;
use llm_providers::model::wires::{chat, messages, responses};
use llm_providers::model::{CancellationToken, Delta, ModelError};
use llm_providers::providers::kiro::wire::{self as kiro, next_frame};

#[derive(Debug)]
struct Pieces {
    bytes: Vec<u8>,
    at: usize,
    sizes: Vec<usize>,
    turn: usize,
}

impl Read for Pieces {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let available = self.fill_buf()?;
        let count = available.len().min(out.len());
        out[..count].copy_from_slice(&available[..count]);
        self.consume(count);
        Ok(count)
    }
}

impl BufRead for Pieces {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        let size = self.sizes[self.turn % self.sizes.len()];
        let end = (self.at + size).min(self.bytes.len());
        Ok(&self.bytes[self.at..end])
    }

    fn consume(&mut self, count: usize) {
        self.at += count;
        self.turn += 1;
    }
}

async fn lines(mut body: Body, most: usize, cancel: &CancellationToken) -> Vec<Result<String, ModelError>> {
    let mut read = Vec::new();
    loop {
        let mut line = String::new();
        match body.read_line_within(&mut line, cancel, most).await {
            Ok(0) => return read,
            Ok(count) => {
                assert_eq!(count, line.len());
                assert!(count <= most, "a line of {count} bytes passed a cap of {most}");
                read.push(Ok(line));
            }
            Err(error) => {
                read.push(Err(error));
                return read;
            }
        }
    }
}

fn events(read: &[Result<String, ModelError>], most: usize) -> Vec<String> {
    let mut decoder = SseDecoder::within(most);
    let mut found = Vec::new();
    for line in read.iter().flatten() {
        if let Ok(Some(event)) = decoder.line(line.trim_end_matches('\n')) {
            found.push(event.data);
        }
    }
    found.extend(decoder.finish().map(|event| event.data));
    for data in &found {
        assert!(data.len() < most, "an event of {} bytes passed a cap of {most}", data.len());
    }
    found
}

fn folded(wire: u8, found: &[String]) {
    let mut on = |_: Delta| {};
    match wire % 3 {
        0 => {
            let mut folder = chat::Folder::new();
            if found.iter().all(|data| folder.event(data, &mut on).is_ok()) {
                let _ = folder.finish(&mut on);
            }
        }
        1 => {
            let mut folder = messages::Folder::new();
            if found.iter().all(|data| folder.event(data, &mut on).is_ok()) {
                let _ = folder.finish();
            }
        }
        _ => {
            let mut folder = responses::Folder::new();
            if found.iter().all(|data| folder.event(data, &mut on).is_ok()) {
                let _ = folder.finish();
            }
        }
    }
}

async fn frames(bytes: &[u8], cancel: &CancellationToken) {
    let mut body = Body::from(bytes.to_vec());
    let mut folder = kiro::Folder::new(Some(200_000));
    let mut on = |_: Delta| {};
    for _ in 0..1024 {
        match next_frame(&mut body, cancel).await {
            Ok(Some(frame)) => {
                if folder.frame(&frame, &mut on).is_err() {
                    return;
                }
            }
            Ok(None) => {
                let _ = folder.finish();
                return;
            }
            Err(_) => return,
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let [wire, size, cap, stream @ ..] = data else { return };
    let most = usize::from(*cap).max(1) * 4;
    let sizes = vec![usize::from(*size % 7) + 1, usize::from(*size / 7 % 5) + 1, 64];
    let cancel = CancellationToken::new();
    let run = async {
        let whole = lines(Body::from(stream.to_vec()), most, &cancel).await;
        let pieces = Pieces { bytes: stream.to_vec(), at: 0, sizes, turn: 0 };
        let chunked = lines(Body::reader(pieces), most, &cancel).await;
        assert_eq!(whole, chunked, "the lines read depend on how the stream was cut");
        let found = events(&whole, most);
        folded(*wire, &found);
        frames(stream, &cancel).await;
    };
    let _ = platform::runtime::block_on(run);
});
