use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

pub fn open(options: &mut OpenOptions, path: &Path) -> io::Result<File> {
    crate::system::restrict_open(options);
    let file = options.open(path)?;
    crate::system::restrict(&file, path)?;
    Ok(file)
}

pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    io::Write::write_all(&mut open(OpenOptions::new().write(true).create(true).truncate(true), path)?, bytes)
}

pub fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = temporary_beside(path);
    let written = (|| {
        let mut file = open(OpenOptions::new().write(true).create_new(true), &temporary)?;
        io::Write::write_all(&mut file, bytes)?;
        crate::system::keep_owner(&file, path)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written
}

fn temporary_beside(path: &Path) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let unique = format!(".{name}.{}.{}.tmp", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
    path.with_file_name(unique)
}
