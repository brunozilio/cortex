use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use domain::lsp::ServerSpec;

use super::{LanguageServer, LanguageServers, Opened};

pub trait Starter: Send + Sync {
    fn find(&self, command: &str) -> Option<PathBuf>;
    fn start(
        &self,
        spec: &ServerSpec,
        program: &Path,
        root: &Path,
        stop: &AtomicBool,
    ) -> Result<Arc<dyn LanguageServer>, String>;
}

#[derive(Debug, Default)]
pub struct Processes;

impl Starter for Processes {
    fn find(&self, command: &str) -> Option<PathBuf> {
        let given = Path::new(command);
        if given.components().count() > 1 {
            return given.is_file().then(|| given.to_path_buf());
        }
        let dirs: Vec<PathBuf> =
            std::env::var_os("PATH").map(|path| std::env::split_paths(&path).collect()).unwrap_or_default();
        domain::executable::find(std::env::consts::OS, &[command], &dirs, Path::is_file)
    }

    fn start(
        &self,
        spec: &ServerSpec,
        program: &Path,
        root: &Path,
        stop: &AtomicBool,
    ) -> Result<Arc<dyn LanguageServer>, String> {
        Ok(Arc::new(super::LspClient::spawn(spec, program, root, stop)?))
    }
}

pub const IDLE: Duration = Duration::from_secs(10 * 60);

enum Slot {
    Empty,
    Running { server: Arc<dyn LanguageServer>, used: Instant },
    Failed { reason: String, at: Instant },
}

type Key = (String, PathBuf);

struct Inner {
    starter: Box<dyn Starter>,
    idle: Duration,
    slots: Mutex<HashMap<Key, Arc<Mutex<Slot>>>>,
}

pub struct LspPool {
    inner: Arc<Inner>,
    sweeping: AtomicBool,
}

impl std::fmt::Debug for LspPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspPool").finish_non_exhaustive()
    }
}

impl Default for LspPool {
    fn default() -> Self {
        Self::new(Processes, IDLE)
    }
}

impl LspPool {
    pub fn new(starter: impl Starter + 'static, idle: Duration) -> Self {
        let inner = Inner { starter: Box::new(starter), idle, slots: Mutex::default() };
        Self { inner: Arc::new(inner), sweeping: AtomicBool::new(false) }
    }

    pub fn sweep(&self, now: Instant) {
        self.inner.sweep(now);
    }

    fn keep_sweeping(&self) {
        if self.sweeping.swap(true, Ordering::Relaxed) {
            return;
        }
        let inner = Arc::downgrade(&self.inner);
        let _ = std::thread::Builder::new().name(concat!(brand::command!(), "-lsp-sweep").into()).spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(60));
                let Some(inner) = inner.upgrade() else { return };
                inner.sweep(Instant::now());
            }
        });
    }
}

impl Inner {
    fn sweep(&self, now: Instant) {
        let Ok(mut slots) = self.slots.lock() else { return };
        slots.retain(|_, slot| {
            let Ok(slot) = slot.try_lock() else { return true };
            let since = match &*slot {
                Slot::Empty => return false,
                Slot::Running { used, .. } => *used,
                Slot::Failed { at, .. } => *at,
            };
            now.saturating_duration_since(since) < self.idle
        });
    }
}

impl LanguageServers for LspPool {
    fn open(&self, file: &Path, specs: &[ServerSpec], cwd: &Path, stop: &AtomicBool) -> Result<Opened, String> {
        let Some((spec, language)) = domain::lsp::server_for(file, specs) else {
            let kind = file.extension().map_or_else(
                || file.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                |ext| format!(".{} files", ext.to_string_lossy()),
            );
            return Err(format!("No language server handles {kind}."));
        };
        let root = domain::lsp::root_for(file, &spec.root_markers, cwd, Path::exists);
        let slot = {
            let mut slots = self.inner.slots.lock().map_err(|_| "the language servers are unavailable")?;
            let key = (spec.name.clone(), root.clone());
            Arc::clone(slots.entry(key).or_insert_with(|| Arc::new(Mutex::new(Slot::Empty))))
        };
        let mut slot = slot.lock().map_err(|_| "the language server is unavailable")?;
        let server = match &*slot {
            Slot::Running { server, .. } if server.is_alive() => Arc::clone(server),
            Slot::Failed { reason, .. } => return Err(reason.clone()),
            Slot::Empty | Slot::Running { .. } => {
                let program = self
                    .inner
                    .starter
                    .find(&spec.command)
                    .ok_or_else(|| format!("{} is not installed (no `{}` on PATH).", spec.name, spec.command))?;
                match self.inner.starter.start(spec, &program, &root, stop) {
                    Ok(server) => {
                        self.keep_sweeping();
                        server
                    }
                    Err(reason) => {
                        *slot = Slot::Failed { reason: reason.clone(), at: Instant::now() };
                        return Err(reason);
                    }
                }
            }
        };
        *slot = Slot::Running { server: Arc::clone(&server), used: Instant::now() };
        Ok(Opened { server, language: language.to_owned(), root })
    }

    fn any_installed(&self, specs: &[ServerSpec]) -> bool {
        specs.iter().any(|spec| self.inner.starter.find(&spec.command).is_some())
    }
}

#[cfg(test)]
#[path = "pool_tests.rs"]
mod tests;
