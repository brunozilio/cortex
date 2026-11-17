use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const WAIT: Duration = Duration::from_secs(10);
const FIRST_PAUSE: Duration = Duration::from_millis(1);
const LONGEST_PAUSE: Duration = Duration::from_millis(8);

#[derive(Debug)]
pub struct Exclusive {
    held: File,
    removed_when_released: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kept {
    Beside,
    OnlyWhileHeld,
}

pub fn beside(path: &Path) -> PathBuf {
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!(".{name}.lock"))
}

impl Exclusive {
    pub fn on(path: &Path) -> io::Result<Self> {
        Self::within(path, WAIT, Kept::Beside)?.ok_or_else(|| {
            let lock = beside(path);
            io::Error::new(io::ErrorKind::TimedOut, format!("{} is held by another process", lock.display()))
        })
    }

    pub fn within(path: &Path, wait: Duration, kept: Kept) -> io::Result<Option<Self>> {
        let lock = beside(path);
        if let Some(parent) = lock.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let deadline = Instant::now() + wait;
        loop {
            let file = crate::private_file::open(
                OpenOptions::new().read(true).write(true).create(true).truncate(false),
                &lock,
            )?;
            if !locked_before(&file, deadline)? {
                return Ok(None);
            }
            if crate::system::is_file_at(&file, &lock) {
                let transient = kept == Kept::OnlyWhileHeld && crate::system::LOCK_FILES_CAN_BE_REMOVED;
                return Ok(Some(Self { held: file, removed_when_released: transient.then_some(lock) }));
            }
        }
    }
}

impl Drop for Exclusive {
    fn drop(&mut self) {
        if let Some(lock) = self.removed_when_released.take()
            && crate::system::is_file_at(&self.held, &lock)
        {
            let _ = std::fs::remove_file(lock);
        }
    }
}

fn locked_before(file: &File, deadline: Instant) -> io::Result<bool> {
    let mut pause = FIRST_PAUSE;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(true),
            Err(TryLockError::Error(error)) => return Err(error),
            Err(TryLockError::WouldBlock) => {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Ok(false);
                }
                std::thread::sleep(pause.min(left));
                pause = (pause * 2).min(LONGEST_PAUSE);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use super::*;

    #[test]
    fn the_lock_file_sits_hidden_beside_the_data() {
        assert_eq!(beside(Path::new("/d/secrets.json")), Path::new("/d/.secrets.json.lock"));
    }

    #[test]
    fn a_second_holder_waits_for_the_first() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("book.json");
        let first = Exclusive::on(&path).expect("first");
        let entered = Arc::new(AtomicBool::new(false));
        let (seen, waiting_path) = (entered.clone(), path.clone());
        let waiter = std::thread::spawn(move || {
            let _second = Exclusive::on(&waiting_path).expect("second");
            seen.store(true, Ordering::SeqCst);
        });
        std::thread::sleep(Duration::from_millis(100));
        assert!(!entered.load(Ordering::SeqCst), "the second holder got in while the first held it");
        drop(first);
        waiter.join().expect("waiter");
        assert!(entered.load(Ordering::SeqCst));
    }

    #[test]
    fn a_lock_still_held_after_the_wait_is_given_up_on() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("book.json");
        let _first = Exclusive::on(&path).expect("first");
        let started = std::time::Instant::now();

        let second = Exclusive::within(&path, Duration::from_millis(60), Kept::Beside).expect("no io error");

        assert!(second.is_none(), "the lock is still the first holder's");
        assert!(started.elapsed() >= Duration::from_millis(60), "it waited the whole bound first");
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[cfg(unix)]
    #[test]
    fn a_lock_kept_only_while_held_leaves_no_file_and_still_excludes() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("notes.md");
        let counted = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    for _ in 0..25 {
                        let _held = Exclusive::within(&path, WAIT, Kept::OnlyWhileHeld).expect("io").expect("in time");
                        let seen = counted.load(Ordering::SeqCst);
                        std::thread::yield_now();
                        counted.store(seen + 1, Ordering::SeqCst);
                    }
                });
            }
        });
        assert_eq!(counted.load(Ordering::SeqCst), 200, "no two holders were inside at once");
        assert!(!beside(&path).exists(), "nothing is left beside the file");
    }
}
