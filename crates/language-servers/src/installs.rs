use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::install::{Install, install_for};
use domain::lsp::ServerSpec;

use super::Wait;

pub trait Installer: Send + Sync {
    fn install(&self, install: &Install, dir: &Path, stop: &AtomicBool) -> Result<(), String>;
}

const CHECK: Duration = Duration::from_millis(100);

enum State {
    Running,
    Done { outcome: Result<PathBuf, String>, at: Instant },
}

struct Installation {
    state: Mutex<State>,
    done: Condvar,
    stop: AtomicBool,
    waiting: AtomicUsize,
}

type Known = Arc<Mutex<HashMap<String, Arc<Installation>>>>;

pub(super) struct Installs {
    dir: PathBuf,
    installer: Arc<dyn Installer>,
    known: Known,
    pub(super) retry: Duration,
}

pub(super) fn missing(spec: &ServerSpec) -> String {
    format!("{} is not installed (no `{}` on PATH).", spec.name, spec.command)
}

fn not_yet(spec: &ServerSpec) -> String {
    format!("{} is not installed yet; the LSP tool installs it on first use.", spec.name)
}

fn recipe(spec: &ServerSpec) -> Option<Install> {
    install_for(spec, std::env::consts::OS, std::env::consts::ARCH)
}

impl Installs {
    pub(super) fn new(dir: PathBuf, installer: Arc<dyn Installer>) -> Self {
        Self { dir, installer, known: Arc::default(), retry: super::pool::RETRY }
    }

    pub(super) fn can_install(&self, spec: &ServerSpec, on_path: impl Fn(&str) -> bool) -> bool {
        let Some(install) = recipe(spec) else { return false };
        if self.dir.join(&spec.name).join(&install.program).is_file() {
            return true;
        }
        install.recipe.tool().is_none_or(|(tool, _)| on_path(tool)) && !self.failed(&spec.name)
    }

    pub(super) fn pending(&self, spec: &ServerSpec, on_path: impl Fn(&str) -> bool) -> Option<String> {
        let install = recipe(spec)?;
        if self.dir.join(&spec.name).join(&install.program).is_file() || self.running(&spec.name) {
            return None;
        }
        let possible = install.recipe.tool().is_none_or(|(tool, _)| on_path(tool)) && !self.failed(&spec.name);
        possible.then(|| format!("install the {} language server: {}", spec.name, install.recipe.described()))
    }

    fn failed(&self, name: &str) -> bool {
        let Ok(known) = self.known.lock() else { return false };
        known.get(name).is_some_and(|installation| installation.failed_within(self.retry))
    }

    pub(super) fn program(&self, spec: &ServerSpec, wait: Wait, stop: &AtomicBool) -> Result<PathBuf, String> {
        let install = recipe(spec).ok_or_else(|| missing(spec))?;
        let home = self.dir.join(&spec.name);
        let program = home.join(&install.program);
        let installation = {
            let mut known = self.known.lock().map_err(|_| "the installs are unavailable")?;
            match known.get(&spec.name).filter(|installation| !installation.may_be_retried(self.retry)) {
                Some(installation) => Arc::clone(installation),
                None if program.is_file() => return Ok(program),
                None if wait == Wait::Never => return Err(not_yet(spec)),
                None => {
                    let installation = Arc::new(Installation {
                        state: Mutex::new(State::Running),
                        done: Condvar::new(),
                        stop: AtomicBool::new(false),
                        waiting: AtomicUsize::new(0),
                    });
                    known.insert(spec.name.clone(), Arc::clone(&installation));
                    self.start(&spec.name, install, home, program, Arc::clone(&installation));
                    installation
                }
            }
        };
        installation.outcome(&spec.name, wait, stop)
    }

    fn start(&self, name: &str, install: Install, home: PathBuf, program: PathBuf, installation: Arc<Installation>) {
        let installer = Arc::clone(&self.installer);
        let known = Arc::clone(&self.known);
        let server = name.to_owned();
        let unstarted = Arc::clone(&installation);
        let work = move || {
            let installing = || installer.install(&install, &home, &installation.stop);
            let installed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(installing));
            let outcome = match installed {
                Ok(Ok(())) if program.is_file() => Ok(program),
                Ok(Ok(())) => {
                    Err(format!("Could not install {server}: {} was not found after installing.", program.display()))
                }
                Ok(Err(reason)) => Err(format!("Could not install {server}: {reason}")),
                Err(_) => Err(format!("Could not install {server}: the installer crashed.")),
            };
            if outcome.is_err() && installation.stop.load(Ordering::Relaxed) {
                forget(&known, &server, &installation);
            }
            installation.finish(outcome);
        };
        let thread = format!(concat!(brand::command!(), "-lsp-install-{}"), name);
        if let Err(error) = std::thread::Builder::new().name(thread).spawn(work) {
            unstarted.finish(Err(format!("Could not start installing {name}: {error}")));
        }
    }

    pub(super) fn running(&self, name: &str) -> bool {
        let Ok(known) = self.known.lock() else { return false };
        known
            .get(name)
            .is_some_and(|installation| installation.state.lock().is_ok_and(|state| matches!(*state, State::Running)))
    }

    pub(super) fn sweep(&self, now: Instant, idle: Duration) {
        let Ok(mut known) = self.known.lock() else { return };
        known.retain(|_, installation| {
            let Ok(state) = installation.state.lock() else { return true };
            match &*state {
                State::Running => true,
                State::Done { at, .. } => now.saturating_duration_since(*at) < idle,
            }
        });
    }
}

fn forget(known: &Known, name: &str, installation: &Arc<Installation>) {
    if let Ok(mut known) = known.lock()
        && known.get(name).is_some_and(|kept| Arc::ptr_eq(kept, installation))
    {
        known.remove(name);
    }
}

impl Installation {
    fn finish(&self, outcome: Result<PathBuf, String>) {
        if let Ok(mut state) = self.state.lock() {
            *state = State::Done { outcome, at: Instant::now() };
        }
        self.done.notify_all();
    }

    fn failed_at(&self) -> Option<Instant> {
        match &*self.state.lock().ok()? {
            State::Done { outcome: Err(_), at } => Some(*at),
            State::Running | State::Done { .. } => None,
        }
    }

    fn failed_within(&self, retry: Duration) -> bool {
        self.failed_at().is_some_and(|at| at.elapsed() < retry)
    }

    fn may_be_retried(&self, retry: Duration) -> bool {
        self.failed_at().is_some_and(|at| at.elapsed() >= retry)
    }

    fn outcome(&self, name: &str, wait: Wait, stop: &AtomicBool) -> Result<PathBuf, String> {
        if wait == Wait::Never {
            return match &*self.state.lock().map_err(|_| "the install is unavailable")? {
                State::Done { outcome, .. } => outcome.clone(),
                State::Running => Err(format!("{name} is installing in the background; ask again shortly.")),
            };
        }
        self.waiting.fetch_add(1, Ordering::SeqCst);
        let awaited = self.awaited(stop);
        let last = self.waiting.fetch_sub(1, Ordering::SeqCst) == 1;
        match awaited {
            Some(outcome) => outcome,
            None => {
                if last {
                    self.stop.store(true, Ordering::Relaxed);
                }
                Err(format!("Stopped while {name} was installing."))
            }
        }
    }

    fn awaited(&self, stop: &AtomicBool) -> Option<Result<PathBuf, String>> {
        let unavailable = || Some(Err("the install is unavailable".to_owned()));
        let Ok(mut state) = self.state.lock() else { return unavailable() };
        loop {
            match &*state {
                State::Done { outcome, .. } => return Some(outcome.clone()),
                State::Running if stop.load(Ordering::Relaxed) => return None,
                State::Running => {
                    let Ok(waited) = self.done.wait_timeout(state, CHECK) else { return unavailable() };
                    state = waited.0;
                }
            }
        }
    }
}
