use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use domain::lsp::ServerSpec;

use super::installs::{Installer, Installs, missing};
use super::{LanguageServer, LanguageServers, Opened, Wait};

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
    installs: Option<Installs>,
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
        Self::with(Box::new(starter), idle, None)
    }

    pub fn installing(
        starter: impl Starter + 'static,
        idle: Duration,
        dir: PathBuf,
        installer: impl Installer + 'static,
    ) -> Self {
        Self::with(Box::new(starter), idle, Some(Installs::new(dir, Arc::new(installer))))
    }

    fn with(starter: Box<dyn Starter>, idle: Duration, installs: Option<Installs>) -> Self {
        let inner = Inner { starter, installs, idle, slots: Mutex::default() };
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
            if Arc::strong_count(slot) > 1 {
                return true;
            }
            let Ok(slot) = slot.try_lock() else { return true };
            let since = match &*slot {
                Slot::Empty => return false,
                Slot::Running { used, .. } => *used,
                Slot::Failed { at, .. } => *at,
            };
            now.saturating_duration_since(since) < self.idle
        });
        drop(slots);
        if let Some(installs) = &self.installs {
            installs.sweep(now, self.idle);
        }
    }
}

impl LanguageServers for LspPool {
    fn open(
        &self,
        file: &Path,
        specs: &[ServerSpec],
        cwd: &Path,
        stop: &AtomicBool,
        wait: Wait,
    ) -> Result<Opened, String> {
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
        let opened = |server| Opened { server, language: language.to_owned(), root: root.clone() };
        if let Some(server) = Self::ready(&slot)? {
            return Ok(opened(server));
        }
        self.keep_sweeping();
        let program = self.program(spec, wait, stop)?;
        let mut slot = slot.lock().map_err(|_| "the language server is unavailable")?;
        let server = match &*slot {
            Slot::Running { server, .. } if server.is_alive() => Arc::clone(server),
            Slot::Failed { reason, .. } => return Err(reason.clone()),
            Slot::Empty | Slot::Running { .. } => match self.inner.starter.start(spec, &program, &root, stop) {
                Ok(server) => {
                    self.keep_sweeping();
                    server
                }
                Err(reason) => {
                    *slot = Slot::Failed { reason: reason.clone(), at: Instant::now() };
                    return Err(reason);
                }
            },
        };
        *slot = Slot::Running { server: Arc::clone(&server), used: Instant::now() };
        Ok(opened(server))
    }

    fn available(&self, specs: &[ServerSpec]) -> Vec<ServerSpec> {
        let on_path = |command: &str| self.inner.starter.find(command).is_some();
        let installable = |spec: &ServerSpec| {
            self.inner.installs.as_ref().is_some_and(|installs| installs.can_install(spec, on_path))
        };
        specs.iter().filter(|spec| on_path(&spec.command) || installable(spec)).cloned().collect()
    }

    fn installing(&self, file: &Path, specs: &[ServerSpec]) -> Option<String> {
        let (spec, _) = domain::lsp::server_for(file, specs)?;
        let installs = self.inner.installs.as_ref()?;
        installs.running(&spec.name).then(|| spec.name.clone())
    }
}

impl LspPool {
    fn ready(slot: &Mutex<Slot>) -> Result<Option<Arc<dyn LanguageServer>>, String> {
        let mut slot = slot.lock().map_err(|_| "the language server is unavailable")?;
        match &*slot {
            Slot::Running { server, .. } if server.is_alive() => {
                let server = Arc::clone(server);
                *slot = Slot::Running { server: Arc::clone(&server), used: Instant::now() };
                Ok(Some(server))
            }
            Slot::Failed { reason, .. } => Err(reason.clone()),
            Slot::Empty | Slot::Running { .. } => Ok(None),
        }
    }

    fn program(&self, spec: &ServerSpec, wait: Wait, stop: &AtomicBool) -> Result<PathBuf, String> {
        if let Some(program) = self.inner.starter.find(&spec.command) {
            return Ok(program);
        }
        match &self.inner.installs {
            Some(installs) => installs.program(spec, wait, stop),
            None => Err(missing(spec)),
        }
    }
}

#[cfg(test)]
#[path = "pool_tests.rs"]
mod tests;
