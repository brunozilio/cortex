use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};

pub(super) const ROTATE_AT: u64 = 64 * 1024 * 1024;
const OPEN_AT_MOST: usize = 16;

pub(super) struct TraceFile {
    out: BufWriter<File>,
    bytes: u64,
}

impl TraceFile {
    fn open(path: &Path) -> io::Result<Self> {
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let cut_mid_write = super::durable::ends_mid_line(path);
        let file = platform::private_file::open(OpenOptions::new().create(true).append(true), path)?;
        let bytes = file.metadata()?.len();
        let mut trace = Self { out: BufWriter::new(file), bytes };
        if cut_mid_write {
            trace.append("\n")?;
        }
        Ok(trace)
    }

    fn append(&mut self, line: &str) -> io::Result<()> {
        self.out.write_all(line.as_bytes())?;
        self.bytes += line.len() as u64;
        Ok(())
    }
}

pub(super) struct Traces {
    open: HashMap<String, TraceFile>,
    rotate_at: u64,
}

impl Default for Traces {
    fn default() -> Self {
        Self { open: HashMap::new(), rotate_at: ROTATE_AT }
    }
}

impl std::fmt::Debug for Traces {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Traces").field("open", &self.open.len()).field("rotate_at", &self.rotate_at).finish()
    }
}

pub(super) struct Paths {
    pub(super) current: PathBuf,
    pub(super) previous: PathBuf,
}

impl Traces {
    pub(super) fn rotating_at(&mut self, bytes: u64) {
        self.rotate_at = bytes;
    }

    pub(super) fn append(&mut self, id: &str, paths: &Paths, line: &str) -> io::Result<()> {
        if self.open.get(id).is_some_and(|trace| trace.bytes >= self.rotate_at) {
            self.close(id);
            std::fs::rename(&paths.current, &paths.previous)?;
        }
        if !self.open.contains_key(id) {
            if self.open.len() >= OPEN_AT_MOST {
                self.open.clear();
            }
            self.open.insert(id.to_owned(), TraceFile::open(&paths.current)?);
        }
        match self.open.get_mut(id) {
            Some(trace) => trace.append(line),
            None => Err(io::Error::new(ErrorKind::NotFound, "the trace file is not open")),
        }
    }

    pub(super) fn flush(&mut self, id: &str) {
        if let Some(trace) = self.open.get_mut(id)
            && let Err(error) = trace.out.flush()
        {
            tracing::warn!(%error, "the trace of this run could not be written");
        }
    }

    pub(super) fn close(&mut self, id: &str) {
        self.flush(id);
        self.open.remove(id);
    }
}
