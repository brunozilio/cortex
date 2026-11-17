#![no_main]

use std::path::PathBuf;
use std::sync::OnceLock;

use agent_engine::sessions::{Entry, FileSessions, Sessions};
use agent_engine::transcript::{Block, Message};
use domain::SessionId;
use libfuzzer_sys::fuzz_target;

const ID: &str = "fuzz";

fn folder() -> Option<&'static PathBuf> {
    static FOLDER: OnceLock<Option<PathBuf>> = OnceLock::new();
    FOLDER.get_or_init(|| tempfile::tempdir().ok().map(tempfile::TempDir::keep)).as_ref()
}

fn said(text: &str, at: usize) -> Message {
    if at.is_multiple_of(2) { Message::user(text) } else { Message::assistant(vec![Block::Text(text.to_owned())]) }
}

fn line(message: &Message, seq: usize) -> Option<String> {
    let mut line = serde_json::to_string(&Entry::Message(message.clone())).ok()?;
    line.pop();
    Some(format!("{line},\"seq\":{seq}}}\n"))
}

fn loaded(dir: &PathBuf, bytes: &[u8]) -> Option<Vec<Message>> {
    for name in [format!("{ID}.jsonl"), format!("{ID}.jsonl.corrupt")] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    std::fs::write(dir.join(format!("{ID}.jsonl")), bytes).ok()?;
    FileSessions::new(dir).load(&SessionId(ID.to_owned()))
}

fuzz_target!(|data: &[u8]| {
    let Some(dir) = folder() else { return };
    let Some((&count, rest)) = data.split_first() else { return };
    let count = usize::from(count % 5);
    let text = String::from_utf8_lossy(rest);
    let mut pieces = text.splitn(count + 1, '\u{1f}');
    let history: Vec<Message> = pieces.by_ref().take(count).enumerate().map(|(at, text)| said(text, at)).collect();
    let garbage = pieces.next().unwrap_or_default();
    let Some(written) =
        history.iter().enumerate().map(|(at, message)| line(message, at + 1)).collect::<Option<String>>()
    else {
        return;
    };

    let _ = loaded(dir, rest);

    let mut bytes = written.into_bytes();
    bytes.extend_from_slice(garbage.as_bytes());
    let back = loaded(dir, &bytes);
    let whole_garbage_is_one_cut_line =
        !garbage.contains(['\n', '\r']) && serde_json::from_str::<serde_json::Value>(garbage).is_err();
    if whole_garbage_is_one_cut_line {
        assert_eq!(back.as_deref(), Some(history.as_slice()), "a valid prefix was not kept before {garbage:?}");
    }
});
