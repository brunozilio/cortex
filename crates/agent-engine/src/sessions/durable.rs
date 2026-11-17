use std::fs::{File, OpenOptions};
use std::hash::{DefaultHasher, Hasher};
use std::io::{self, ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use super::log;
use crate::transcript::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Persisted {
    pub(super) messages: usize,
    pub(super) digest: u64,
    pub(super) bytes: u64,
}

impl Persisted {
    pub(super) fn of(history: &[Message], bytes: u64) -> Self {
        Self { messages: history.len(), digest: Digests::of(history, history.len()).whole, bytes }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Digests {
    pub(super) start: Option<u64>,
    pub(super) whole: u64,
}

impl Digests {
    pub(super) fn of(history: &[Message], start: usize) -> Self {
        let mut hashed = Hashed { hasher: DefaultHasher::new(), pending: Vec::with_capacity(FED_AT) };
        let mut at_start = (start == 0).then(|| hashed.so_far());
        for (index, message) in history.iter().enumerate() {
            if serde_json::to_writer(&mut hashed, message).is_err() {
                hashed.pending.push(0);
            }
            hashed.pending.push(b'\n');
            hashed.feed();
            if index + 1 == start {
                at_start = Some(hashed.so_far());
            }
        }
        Self { start: at_start, whole: hashed.so_far() }
    }
}

const FED_AT: usize = 64 * 1024;

struct Hashed {
    hasher: DefaultHasher,
    pending: Vec<u8>,
}

impl Hashed {
    fn feed(&mut self) {
        self.hasher.write(&self.pending);
        self.pending.clear();
    }

    fn so_far(&self) -> u64 {
        self.hasher.clone().finish()
    }
}

impl Write for Hashed {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(bytes);
        if self.pending.len() >= FED_AT {
            self.feed();
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Written {
    pub appends: u64,
    pub bytes: u64,
    pub syncs: u64,
    pub rereads: u64,
}

#[derive(Debug, Default)]
pub(super) struct Counted {
    appends: AtomicU64,
    bytes: AtomicU64,
    syncs: AtomicU64,
    rereads: AtomicU64,
}

impl Counted {
    pub(super) fn reread(&self) {
        self.rereads.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn so_far(&self) -> Written {
        Written {
            appends: self.appends.load(Ordering::Relaxed),
            bytes: self.bytes.load(Ordering::Relaxed),
            syncs: self.syncs.load(Ordering::Relaxed),
            rereads: self.rereads.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tail {
    KnownWhole,
    Unknown,
}

pub(super) fn length(path: &Path) -> io::Result<Option<u64>> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(Some(metadata.len())),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub(super) fn append(path: &Path, lines: &str, tail: Tail, counted: &Counted) -> io::Result<u64> {
    let unfinished = if tail == Tail::Unknown { end_unfinished_line(path)? } else { "" };
    let created = tail == Tail::Unknown && !path.exists();
    let mut file = platform::private_file::open(OpenOptions::new().create(true).append(true), path)?;
    if !lines.is_empty() || !unfinished.is_empty() {
        let bytes = [unfinished.as_bytes(), lines.as_bytes()].concat();
        file.write_all(&bytes)?;
        file.sync_data()?;
        counted.appends.fetch_add(1, Ordering::Relaxed);
        counted.bytes.fetch_add(bytes.len() as u64, Ordering::Relaxed);
        counted.syncs.fetch_add(1, Ordering::Relaxed);
    }
    if created {
        sync_folder(path);
    }
    Ok(file.metadata()?.len())
}

fn sync_folder(path: &Path) {
    if cfg!(unix)
        && let Some(folder) = path.parent()
        && let Ok(folder) = File::open(folder)
    {
        let _ = folder.sync_all();
    }
}

fn end_unfinished_line(path: &Path) -> io::Result<&'static str> {
    let mut file = match OpenOptions::new().read(true).write(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(""),
        Err(error) => return Err(error),
    };
    let mut last = *b"\n";
    if file.seek(SeekFrom::End(-1)).is_ok() {
        file.read_exact(&mut last)?;
    }
    if last == *b"\n" {
        return Ok("");
    }
    let mut bytes = Vec::new();
    file.seek(SeekFrom::Start(0))?;
    file.read_to_end(&mut bytes)?;
    let last = bytes.iter().rposition(|byte| *byte == b'\n').map_or(0, |at| at + 1);
    let whole = match std::str::from_utf8(&bytes[last..]) {
        Ok(line) => log::written(line).len() == line.len(),
        Err(_) => {
            tracing::warn!(
                bytes = bytes.len() - last,
                "dropping the last line of a saved conversation: it was cut inside a character"
            );
            false
        }
    };
    if whole {
        return Ok("\n");
    }
    file.set_len(last as u64)?;
    Ok("")
}

pub(super) fn ends_mid_line(path: &Path) -> bool {
    let mut last = *b"\n";
    let read = File::open(path).and_then(|mut file| {
        file.seek(SeekFrom::End(-1))?;
        file.read_exact(&mut last)
    });
    read.is_ok() && last != *b"\n"
}
