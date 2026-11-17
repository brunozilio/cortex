use std::io;
use std::path::{Path, PathBuf};

pub fn write(path: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    let target = resolved(path)?;
    crate::private_file::replace_keeping_mode(&target, bytes)?;
    Ok(target)
}

pub fn replace_entry(path: &Path, bytes: &[u8]) -> io::Result<()> {
    crate::private_file::replace_keeping_mode(path, bytes)
}

pub fn resolved(path: &Path) -> io::Result<PathBuf> {
    let mut at = path.to_path_buf();
    for _ in 0..40 {
        match std::fs::symlink_metadata(&at) {
            Ok(found) if found.file_type().is_symlink() => {
                let points = std::fs::read_link(&at)?;
                at = match at.parent() {
                    Some(parent) if points.is_relative() => parent.join(points),
                    _ => points,
                };
            }
            _ => return Ok(at),
        }
    }
    Err(io::Error::other(format!("{} is a chain of links that does not end", path.display())))
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    #[test]
    fn a_reader_never_sees_a_file_half_written() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.txt");
        let (one, two) = ("1".repeat(256 * 1024), "2".repeat(256 * 1024));
        write(&path, one.as_bytes()).expect("first");
        let done = Arc::new(AtomicBool::new(false));
        let reader = {
            let (path, done) = (path.clone(), Arc::clone(&done));
            std::thread::spawn(move || {
                let mut torn = 0;
                while !done.load(Ordering::Relaxed) {
                    let read = std::fs::read(&path).expect("read");
                    let whole = read.len() == 256 * 1024 && read.iter().all(|byte| *byte == read[0]);
                    torn += usize::from(!whole);
                }
                torn
            })
        };
        for round in 0..200 {
            write(&path, if round % 2 == 0 { two.as_bytes() } else { one.as_bytes() }).expect("write");
        }
        done.store(true, Ordering::Relaxed);
        assert_eq!(reader.join().expect("reader"), 0, "a reader saw a torn or empty file");
        let left: Vec<_> = std::fs::read_dir(dir.path()).expect("dir").collect();
        assert_eq!(left.len(), 1, "no temporary file is left behind");
    }

    #[test]
    fn a_file_keeps_its_mode_and_a_link_keeps_pointing_at_the_file_it_names() {
        let dir = tempfile::tempdir().expect("dir");
        let script = dir.path().join("run.sh");
        std::fs::write(&script, "old").expect("script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("mode");
        let link = dir.path().join("link.sh");
        std::os::unix::fs::symlink("run.sh", &link).expect("link");

        assert_eq!(write(&link, b"new").expect("write"), script);

        assert_eq!(std::fs::read_to_string(&script).expect("read"), "new");
        assert!(std::fs::symlink_metadata(&link).expect("link").file_type().is_symlink());
        assert_eq!(std::fs::metadata(&script).expect("meta").permissions().mode() & 0o777, 0o755);
        let fresh = dir.path().join("sub/new.txt");
        write(&fresh, b"x").expect("a new file in a new folder");
        assert_eq!(std::fs::read_to_string(fresh).expect("read"), "x");
    }

    #[test]
    fn replacing_an_entry_replaces_a_link_instead_of_writing_where_it_points() {
        let dir = tempfile::tempdir().expect("dir");
        let target = dir.path().join("elsewhere.txt");
        std::fs::write(&target, "kept").expect("target");
        let link = dir.path().join("a.txt");
        std::os::unix::fs::symlink(&target, &link).expect("link");

        replace_entry(&link, b"restored").expect("replace");

        assert_eq!(std::fs::read_to_string(&target).expect("target"), "kept");
        assert!(!std::fs::symlink_metadata(&link).expect("entry").file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&link).expect("entry"), "restored");
    }
}
