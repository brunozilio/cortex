use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Exclusive {
    _held: File,
}

pub fn beside(path: &Path) -> PathBuf {
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!(".{name}.lock"))
}

impl Exclusive {
    pub fn on(path: &Path) -> io::Result<Self> {
        let lock = beside(path);
        if let Some(parent) = lock.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = crate::private_file::open(OpenOptions::new().read(true).write(true).create(true).truncate(false), &lock)?;
        file.lock()?;
        Ok(Self { _held: file })
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
}
