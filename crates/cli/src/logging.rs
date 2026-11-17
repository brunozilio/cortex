use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tracing_subscriber::EnvFilter;

const DEFAULT_FILTER: &str = "warn";
const KEPT_BYTES: u64 = 5 * 1024 * 1024;

pub fn file_in(data: &Path) -> PathBuf {
    data.join("logs").join(format!("{}.log", brand::COMMAND))
}

pub fn filter(asked: Option<&str>) -> EnvFilter {
    asked
        .filter(|text| !text.trim().is_empty())
        .and_then(|text| EnvFilter::try_new(text).ok())
        .unwrap_or_else(|| EnvFilter::new(DEFAULT_FILTER))
}

pub fn open(file: &Path) -> std::io::Result<File> {
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }
    if std::fs::metadata(file).is_ok_and(|meta| meta.len() > KEPT_BYTES) {
        std::fs::rename(file, file.with_extension("log.1"))?;
    }
    OpenOptions::new().create(true).append(true).open(file)
}

pub fn install() -> Option<PathBuf> {
    let data = platform::local_data_dir(brand::NAME, brand::COMMAND)?;
    let path = file_in(&data);
    let file = open(&path).ok()?;
    let asked = std::env::var(brand::env("LOG")).ok();
    tracing_subscriber::fmt()
        .with_env_filter(filter(asked.as_deref()))
        .with_writer(Mutex::new(file))
        .with_ansi(false)
        .with_thread_names(true)
        .try_init()
        .ok()?;
    log_panics();
    Some(path)
}

fn log_panics() {
    let earlier = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let thread = std::thread::current();
        tracing::error!(thread = thread.name().unwrap_or("unnamed"), "{panic}");
        earlier(panic);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_sits_in_the_data_folder_under_the_command() {
        let file = file_in(Path::new("/data"));
        assert_eq!(file, Path::new("/data/logs").join(format!("{}.log", brand::COMMAND)));
    }

    #[test]
    fn warnings_are_kept_unless_another_filter_is_asked() {
        assert_eq!(filter(None).to_string(), "warn");
        assert_eq!(filter(Some("  ")).to_string(), "warn");
        assert_eq!(filter(Some("app=debug")).to_string(), "app=debug");
        assert_eq!(filter(Some("=[")).to_string(), "warn");
    }

    #[test]
    fn a_large_log_is_moved_aside_before_writing() {
        let dir = tempfile::tempdir().expect("dir");
        let file = file_in(dir.path());
        open(&file).expect("first");
        std::fs::write(&file, vec![b'x'; (KEPT_BYTES + 1) as usize]).expect("fill");
        open(&file).expect("again");
        assert_eq!(std::fs::metadata(&file).expect("new").len(), 0);
        assert!(file.with_extension("log.1").exists());
    }
}
