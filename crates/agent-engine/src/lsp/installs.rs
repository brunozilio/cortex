use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use domain::lsp::ServerSpec;
use domain::lsp::install::{Install, install_for};

use super::Wait;

pub trait Installer: Send + Sync {
    fn install(&self, install: &Install, dir: &Path) -> Result<(), String>;
}

const CHECK: Duration = Duration::from_millis(100);

enum State {
    Running,
    Done { outcome: Result<PathBuf, String>, at: Instant },
}

struct Installation {
    state: Mutex<State>,
    done: Condvar,
}

pub(super) struct Installs {
    dir: PathBuf,
    installer: Arc<dyn Installer>,
    known: Mutex<HashMap<String, Arc<Installation>>>,
}

pub(super) fn missing(spec: &ServerSpec) -> String {
    format!("{} is not installed (no `{}` on PATH).", spec.name, spec.command)
}

fn recipe(spec: &ServerSpec) -> Option<Install> {
    install_for(spec, std::env::consts::OS, std::env::consts::ARCH)
}

impl Installs {
    pub(super) fn new(dir: PathBuf, installer: Arc<dyn Installer>) -> Self {
        Self { dir, installer, known: Mutex::default() }
    }

    pub(super) fn can_install(spec: &ServerSpec) -> bool {
        recipe(spec).is_some()
    }

    pub(super) fn program(&self, spec: &ServerSpec, wait: Wait, stop: &AtomicBool) -> Result<PathBuf, String> {
        let install = recipe(spec).ok_or_else(|| missing(spec))?;
        let home = self.dir.join(&spec.name);
        let program = home.join(&install.program);
        let installation = {
            let mut known = self.known.lock().map_err(|_| "the installs are unavailable")?;
            match known.get(&spec.name) {
                Some(installation) => Arc::clone(installation),
                None if program.is_file() => return Ok(program),
                None => {
                    let installation =
                        Arc::new(Installation { state: Mutex::new(State::Running), done: Condvar::new() });
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
        let server = name.to_owned();
        let unstarted = Arc::clone(&installation);
        let work = move || {
            let outcome = match installer.install(&install, &home) {
                Ok(()) if program.is_file() => Ok(program),
                Ok(()) => {
                    Err(format!("Could not install {server}: {} was not found after installing.", program.display()))
                }
                Err(reason) => Err(format!("Could not install {server}: {reason}")),
            };
            installation.finish(outcome);
        };
        let thread = format!(concat!(brand::command!(), "-lsp-install-{}"), name);
        if let Err(error) = std::thread::Builder::new().name(thread).spawn(work) {
            unstarted.finish(Err(format!("Could not start installing {name}: {error}")));
        }
    }

    pub(super) fn running(&self, name: &str) -> bool {
        let Ok(known) = self.known.lock() else { return false };
        known.get(name).is_some_and(|installation| {
            installation.state.lock().is_ok_and(|state| matches!(*state, State::Running))
        })
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

impl Installation {
    fn finish(&self, outcome: Result<PathBuf, String>) {
        if let Ok(mut state) = self.state.lock() {
            *state = State::Done { outcome, at: Instant::now() };
        }
        self.done.notify_all();
    }

    fn outcome(&self, name: &str, wait: Wait, stop: &AtomicBool) -> Result<PathBuf, String> {
        let mut state = self.state.lock().map_err(|_| "the install is unavailable")?;
        loop {
            match &*state {
                State::Done { outcome, .. } => return outcome.clone(),
                State::Running if wait == Wait::Never => {
                    return Err(format!("{name} is installing in the background; ask again shortly."));
                }
                State::Running if stop.load(Ordering::Relaxed) => {
                    return Err(format!("Stopped while {name} was installing."));
                }
                State::Running => {
                    state = self.done.wait_timeout(state, CHECK).map_err(|_| "the install is unavailable")?.0;
                }
            }
        }
    }
}
