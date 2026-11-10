//! `MlxRuntime` serves one downloaded model at a time: it installs the
//! runtime when missing, starts the model's server, waits for it, swaps it
//! for another model and lets it go when idle.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use superfik_mlx::{Clock, IDLE, Launcher, MlxError, MlxRuntime, Served};

const QWEN: &str = "mlx-community/Qwen3.8-27B-8bit";
const CODER: &str = "mlx-community/Qwen3-Coder-Next-4bit";

#[derive(Default)]
struct Log {
    installs: u32,
    launched: Vec<PathBuf>,
    stopped: Vec<String>,
}

#[derive(Default)]
struct FakeLauncher {
    installed: AtomicBool,
    install_fails: bool,
    slow: u32,
    log: Arc<Mutex<Log>>,
    crashed: Arc<AtomicBool>,
}

struct FakeServed {
    url: String,
    checks: u32,
    slow: u32,
    log: Arc<Mutex<Log>>,
    crashed: Arc<AtomicBool>,
}

impl Launcher for FakeLauncher {
    fn is_installed(&self) -> bool {
        self.installed.load(Ordering::SeqCst)
    }

    fn install(&self) -> Result<(), String> {
        self.log.lock().expect("log").installs += 1;
        if self.install_fails {
            return Err("uv is not installed".into());
        }
        self.installed.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn launch(&self, model_dir: &Path) -> io::Result<Box<dyn Served>> {
        let mut log = self.log.lock().expect("log");
        log.launched.push(model_dir.to_path_buf());
        self.crashed.store(false, Ordering::SeqCst);
        Ok(Box::new(FakeServed {
            url: format!("http://127.0.0.1:{}", 5000 + log.launched.len()),
            checks: 0,
            slow: self.slow,
            log: self.log.clone(),
            crashed: self.crashed.clone(),
        }))
    }
}

impl Served for FakeServed {
    fn base_url(&self) -> String {
        self.url.clone()
    }

    fn is_ready(&mut self) -> bool {
        self.checks += 1;
        self.checks > self.slow
    }

    fn is_running(&mut self) -> bool {
        !self.crashed.load(Ordering::SeqCst)
    }

    fn stop(&mut self) {
        self.log.lock().expect("log").stopped.push(self.url.clone());
    }
}

#[derive(Default)]
struct FakeClock(AtomicU64);

impl FakeClock {
    fn advance(&self, by: Duration) {
        self.0.fetch_add(by.as_millis() as u64, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Duration {
        Duration::from_millis(self.0.load(Ordering::SeqCst))
    }

    fn sleep(&self, by: Duration) {
        self.advance(by);
    }
}

struct Setup {
    runtime: MlxRuntime,
    launcher: Arc<FakeLauncher>,
    clock: Arc<FakeClock>,
    models: tempfile::TempDir,
}

fn setup(launcher: FakeLauncher) -> Setup {
    let models = tempfile::tempdir().expect("models");
    for model in [QWEN, CODER] {
        std::fs::create_dir_all(models.path().join(model)).expect("model dir");
        std::fs::write(models.path().join(model).join("config.json"), "{}").expect("config");
    }
    let launcher = Arc::new(launcher);
    let clock = Arc::new(FakeClock::default());
    let runtime = MlxRuntime::new(models.path().to_path_buf(), launcher.clone(), clock.clone());
    Setup { runtime, launcher, clock, models }
}

fn installed() -> FakeLauncher {
    FakeLauncher { installed: AtomicBool::new(true), ..FakeLauncher::default() }
}

#[test]
fn the_first_request_starts_the_models_server() {
    let s = setup(installed());
    let endpoint = s.runtime.endpoint(QWEN).expect("endpoint");
    assert_eq!(endpoint.base_url, "http://127.0.0.1:5001");
    let dir = s.models.path().join(QWEN);
    assert_eq!(endpoint.model, dir.to_string_lossy());
    assert_eq!(s.launcher.log.lock().expect("log").launched, [dir]);
}

#[test]
fn the_same_model_keeps_its_server() {
    let s = setup(installed());
    s.runtime.endpoint(QWEN).expect("first");
    s.runtime.endpoint(QWEN).expect("second");
    assert_eq!(s.launcher.log.lock().expect("log").launched.len(), 1);
}

#[test]
fn another_model_takes_the_place_of_the_first() {
    let s = setup(installed());
    s.runtime.endpoint(QWEN).expect("qwen");
    let coder = s.runtime.endpoint(CODER).expect("coder");
    let log = s.launcher.log.lock().expect("log");
    assert_eq!(log.stopped, ["http://127.0.0.1:5001"]);
    assert_eq!(coder.base_url, "http://127.0.0.1:5002");
}

#[test]
fn a_server_that_died_starts_again() {
    let s = setup(installed());
    s.runtime.endpoint(QWEN).expect("first");
    s.launcher.crashed.store(true, Ordering::SeqCst);
    s.runtime.endpoint(QWEN).expect("again");
    assert_eq!(s.launcher.log.lock().expect("log").launched.len(), 2);
}

#[test]
fn a_model_not_downloaded_is_refused() {
    let s = setup(installed());
    let refused = s.runtime.endpoint("mlx-community/Nothing-4bit");
    assert!(matches!(refused, Err(MlxError::NotDownloaded(why)) if why.contains("not downloaded")));
    assert!(s.launcher.log.lock().expect("log").launched.is_empty());
}

#[test]
fn a_model_id_cannot_climb_out_of_the_models_folder() {
    let s = setup(installed());
    assert!(matches!(s.runtime.endpoint("../../etc"), Err(MlxError::NotDownloaded(_))));
    assert!(matches!(s.runtime.endpoint("/etc"), Err(MlxError::NotDownloaded(_))));
}

#[test]
fn the_runtime_is_installed_once_before_the_first_server() {
    let s = setup(FakeLauncher::default());
    s.runtime.endpoint(QWEN).expect("first");
    s.runtime.endpoint(CODER).expect("second");
    let log = s.launcher.log.lock().expect("log");
    assert_eq!((log.installs, log.launched.len()), (1, 2));
}

#[test]
fn a_failed_install_says_why() {
    let s = setup(FakeLauncher { install_fails: true, ..FakeLauncher::default() });
    let failed = s.runtime.endpoint(QWEN);
    assert!(matches!(failed, Err(MlxError::Failed(why)) if why.contains("uv is not installed")));
}

#[test]
fn a_server_slow_to_answer_is_unavailable_and_kept() {
    let s = setup(FakeLauncher { slow: u32::MAX, ..installed() });
    assert!(matches!(s.runtime.endpoint(QWEN), Err(MlxError::Starting(_))));
    assert!(matches!(s.runtime.endpoint(QWEN), Err(MlxError::Starting(_))));
    assert_eq!(s.launcher.log.lock().expect("log").launched.len(), 1);
}

#[test]
fn an_idle_server_is_let_go() {
    let s = setup(installed());
    s.runtime.endpoint(QWEN).expect("endpoint");
    s.clock.advance(IDLE - Duration::from_secs(1));
    s.runtime.reap();
    assert!(s.launcher.log.lock().expect("log").stopped.is_empty());
    s.clock.advance(Duration::from_secs(2));
    s.runtime.reap();
    assert_eq!(s.launcher.log.lock().expect("log").stopped.len(), 1);
    s.runtime.endpoint(QWEN).expect("again");
    assert_eq!(s.launcher.log.lock().expect("log").launched.len(), 2);
}
