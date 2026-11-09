//! What members asked the server to do, kept append-only: one JSON line
//! per command (who, what, when, and whether their role allowed it),
//! never the command's content. The file rotates by size, keeping a few
//! old ones; the export reads it a page at a time.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use superfik_core::Principal;
use superfik_core::access::Ability;
use superfik_protocol::ClientCommand;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Allowed,
    Refused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub at_unix: u64,
    pub principal: Principal,
    pub command: String,
    pub ability: Ability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Outcome>,
}

impl Entry {
    pub fn of(principal: &Principal, command: &ClientCommand, at_unix: u64, outcome: Outcome) -> Self {
        Self {
            at_unix,
            principal: principal.clone(),
            command: command.name().to_owned(),
            ability: command.ability(),
            outcome: Some(outcome),
        }
    }
}

pub trait AuditLog: Send + Sync {
    fn record(&self, entry: &Entry);
    fn page(&self, at_unix: u64, skip: usize, limit: usize) -> Vec<Entry>;
}

pub fn next_page(since: u64, skip: usize, page: &[Entry], limit: usize) -> Option<(u64, usize)> {
    let last = page.last().filter(|_| page.len() >= limit)?.at_unix;
    if last == since {
        return Some((since, skip + page.len()));
    }
    Some((last, page.iter().filter(|entry| entry.at_unix == last).count()))
}

#[derive(Debug, Default)]
pub struct MemoryAudit(pub Mutex<Vec<Entry>>);

impl AuditLog for MemoryAudit {
    fn record(&self, entry: &Entry) {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).push(entry.clone());
    }

    fn page(&self, at_unix: u64, skip: usize, limit: usize) -> Vec<Entry> {
        let entries = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        entries.iter().filter(|entry| entry.at_unix >= at_unix).skip(skip).take(limit).cloned().collect()
    }
}

const MAX_BYTES: u64 = 16 * 1024 * 1024;
const KEEP: usize = 5;

#[derive(Debug)]
pub struct FileAudit {
    path: PathBuf,
    max_bytes: u64,
    keep: usize,
    writing: Mutex<()>,
}

impl FileAudit {
    pub fn new(path: &Path) -> Self {
        Self::rotating(path, MAX_BYTES, KEEP)
    }

    pub fn rotating(path: &Path, max_bytes: u64, keep: usize) -> Self {
        Self { path: path.to_path_buf(), max_bytes, keep, writing: Mutex::new(()) }
    }

    fn old(&self, n: usize) -> PathBuf {
        let mut name = self.path.as_os_str().to_owned();
        name.push(format!(".{n}"));
        PathBuf::from(name)
    }

    fn rotate(&self) -> std::io::Result<()> {
        let gone = |result: std::io::Result<()>| match result {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
        if self.keep == 0 {
            return gone(std::fs::remove_file(&self.path));
        }
        gone(std::fs::remove_file(self.old(self.keep)))?;
        for n in (1..self.keep).rev() {
            gone(std::fs::rename(self.old(n), self.old(n + 1)))?;
        }
        gone(std::fs::rename(&self.path, self.old(1)))
    }

    fn append(&self, line: &str) -> std::io::Result<()> {
        let size = std::fs::metadata(&self.path).map_or(0, |m| m.len());
        if size > 0 && size + line.len() as u64 + 1 > self.max_bytes {
            self.rotate()?;
        }
        let mut file = superfik_private_file::open(std::fs::OpenOptions::new().create(true).append(true), &self.path)?;
        writeln!(file, "{line}")
    }
}

impl AuditLog for FileAudit {
    fn record(&self, entry: &Entry) {
        let _writing = self.writing.lock().unwrap_or_else(PoisonError::into_inner);
        let written = serde_json::to_string(entry).map_err(std::io::Error::other).and_then(|line| self.append(&line));
        if let Err(error) = written {
            tracing::error!(%error, "could not write the audit log");
        }
    }

    fn page(&self, at_unix: u64, skip: usize, limit: usize) -> Vec<Entry> {
        let files = (1..=self.keep).rev().map(|n| self.old(n)).chain([self.path.clone()]);
        files
            .filter_map(|path| std::fs::File::open(path).ok())
            .flat_map(|file| std::io::BufReader::new(file).lines().map_while(Result::ok))
            .filter_map(|line| serde_json::from_str::<Entry>(&line).ok())
            .filter(|entry| entry.at_unix >= at_unix)
            .skip(skip)
            .take(limit)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::RunId;

    use super::*;

    fn entry(at: u64) -> Entry {
        Entry::of(&Principal::LocalOwner, &ClientCommand::ListModels, at, Outcome::Allowed)
    }

    #[test]
    fn entries_are_appended_one_json_line_each() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("audit.jsonl");
        let log = FileAudit::new(&path);
        let first = entry(1);
        let second =
            Entry::of(&Principal::LocalOwner, &ClientCommand::CancelRun { run: RunId::new() }, 2, Outcome::Refused);
        log.record(&first);
        log.record(&second);
        let text = std::fs::read_to_string(&path).expect("read");
        let read: Vec<Entry> = text.lines().map(|line| serde_json::from_str(line).expect("entry")).collect();
        assert_eq!(read[1].ability, Ability::Run);
        assert_eq!(read[1].outcome, Some(Outcome::Refused));
        assert_eq!(log.page(2, 0, 10), std::slice::from_ref(&second));
        assert_eq!(read, [first, second]);
    }

    #[test]
    fn an_entry_written_before_outcomes_were_kept_still_reads() {
        let old = r#"{"at_unix":1,"principal":"LocalOwner","command":"list_models","ability":"read"}"#;
        let read: Entry = serde_json::from_str(old).expect("entry");
        assert_eq!(read.outcome, None);
    }

    #[test]
    fn an_entry_names_the_command_not_its_content() {
        let secret = ClientCommand::CreateTeam { name: "Project Falcon".into() };
        let line =
            serde_json::to_string(&Entry::of(&Principal::LocalOwner, &secret, 1, Outcome::Allowed)).expect("json");
        assert!(line.contains("create_team"));
        assert!(!line.contains("Falcon"));
    }

    #[test]
    fn a_full_log_rotates_and_keeps_only_so_many_files() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("audit.jsonl");
        let line = serde_json::to_string(&entry(1)).expect("json").len() as u64 + 1;
        let log = FileAudit::rotating(&path, 2 * line, 2);
        for at in 1..=9 {
            log.record(&entry(at));
        }
        let mut files: Vec<String> = std::fs::read_dir(dir.path())
            .expect("dir")
            .map(|f| f.expect("file").file_name().into_string().expect("name"))
            .collect();
        files.sort();
        assert_eq!(files, ["audit.jsonl", "audit.jsonl.1", "audit.jsonl.2"]);
        let kept: Vec<u64> = log.page(0, 0, 100).iter().map(|e| e.at_unix).collect();
        assert_eq!(kept, [5, 6, 7, 8, 9], "the oldest files went, the rest read oldest first");
    }

    #[test]
    fn a_page_holds_at_most_its_limit_from_where_the_last_one_ended() {
        let log = MemoryAudit::default();
        for at in [10, 20, 20, 20, 30] {
            log.record(&entry(at));
        }
        let first = log.page(20, 0, 2);
        assert_eq!(first.iter().map(|e| e.at_unix).collect::<Vec<_>>(), [20, 20]);
        let next = next_page(20, 0, &first, 2).expect("more may follow");
        assert_eq!(next, (20, 2), "the same second, past what was read");
        let second = log.page(next.0, next.1, 2);
        assert_eq!(second.iter().map(|e| e.at_unix).collect::<Vec<_>>(), [20, 30]);
        assert_eq!(next_page(next.0, next.1, &second, 2), Some((30, 1)));
        assert_eq!(log.page(30, 1, 2), []);
        assert_eq!(next_page(30, 1, &[], 2), None, "a short page is the last");
    }
}
