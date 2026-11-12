use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use domain::lsp::install::{Install, Recipe, install_for};
use domain::lsp::{ServerSpec, defaults};
use serde_json::Value;

use super::{Installer, LspPool, Starter};
use crate::lsp::{LanguageServer, LanguageServers, Wait};

const IDLE: Duration = Duration::from_secs(600);

struct Fake {
    root: PathBuf,
    alive: AtomicBool,
}

impl LanguageServer for Fake {
    fn request(&self, _: &str, _: Value, _: &AtomicBool, _: Duration) -> Result<Value, String> {
        Ok(Value::Null)
    }
    fn sync(&self, _: &Path, _: &str, _: &str) -> Result<i64, String> {
        Ok(1)
    }
    fn diagnostics(&self, _: &Path, _: i64, _: Duration) -> Option<Vec<Value>> {
        None
    }
    fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }
}

#[derive(Clone, Default)]
struct Starts {
    installed: Vec<&'static str>,
    broken: Vec<&'static str>,
    started: Arc<Mutex<Vec<(String, PathBuf)>>>,
    programs: Arc<Mutex<Vec<PathBuf>>>,
    servers: Arc<Mutex<Vec<Arc<Fake>>>>,
}

impl Starts {
    fn count(&self) -> usize {
        self.started.lock().expect("starts").len()
    }
}

impl Starter for Starts {
    fn find(&self, command: &str) -> Option<PathBuf> {
        self.installed.contains(&command).then(|| PathBuf::from("/bin").join(command))
    }
    fn start(
        &self,
        spec: &ServerSpec,
        program: &Path,
        root: &Path,
        _: &AtomicBool,
    ) -> Result<Arc<dyn LanguageServer>, String> {
        assert!(program.is_absolute() && program.to_string_lossy().contains(&spec.command), "{program:?}");
        self.started.lock().expect("starts").push((spec.name.clone(), root.to_path_buf()));
        self.programs.lock().expect("programs").push(program.to_path_buf());
        if self.broken.contains(&spec.command.as_str()) {
            return Err(format!("{} crashed", spec.name));
        }
        let server = Arc::new(Fake { root: root.to_path_buf(), alive: AtomicBool::new(true) });
        self.servers.lock().expect("servers").push(Arc::clone(&server));
        Ok(server)
    }
}

fn pool(starts: &Starts) -> LspPool {
    LspPool::new(starts.clone(), IDLE)
}

fn open(pool: &LspPool, file: &str) -> Result<crate::lsp::Opened, String> {
    open_waiting(pool, file, Wait::ForInstall)
}

fn open_waiting(pool: &LspPool, file: &str, wait: Wait) -> Result<crate::lsp::Opened, String> {
    pool.open(Path::new(file), &defaults(), Path::new("/no-such-w"), &AtomicBool::new(false), wait)
}

#[derive(Clone)]
struct Installs {
    outcome: Result<(), String>,
    gate: Arc<(Mutex<bool>, Condvar)>,
    asked: Arc<Mutex<Vec<(Recipe, PathBuf)>>>,
}

impl Installs {
    fn succeeding() -> Self {
        Self { outcome: Ok(()), gate: Arc::new((Mutex::new(true), Condvar::new())), asked: Arc::default() }
    }
    fn failing(reason: &str) -> Self {
        Self { outcome: Err(reason.into()), ..Self::succeeding() }
    }
    fn held() -> Self {
        Self { gate: Arc::new((Mutex::new(false), Condvar::new())), ..Self::succeeding() }
    }
    fn release(&self) {
        *self.gate.0.lock().expect("gate") = true;
        self.gate.1.notify_all();
    }
    fn count(&self) -> usize {
        self.asked.lock().expect("asked").len()
    }
}

impl Installer for Installs {
    fn install(&self, install: &Install, dir: &Path) -> Result<(), String> {
        self.asked.lock().expect("asked").push((install.recipe.clone(), dir.to_path_buf()));
        let mut open = self.gate.0.lock().expect("gate");
        while !*open {
            open = self.gate.1.wait(open).expect("gate");
        }
        self.outcome.clone()?;
        let program = dir.join(&install.program);
        std::fs::create_dir_all(program.parent().expect("folder")).map_err(|e| e.to_string())?;
        std::fs::write(&program, "").map_err(|e| e.to_string())
    }
}

fn typescript() -> Install {
    let spec = defaults().into_iter().find(|spec| spec.name == "typescript").expect("built in");
    install_for(&spec, std::env::consts::OS, std::env::consts::ARCH).expect("installable")
}

fn names(specs: &[ServerSpec]) -> Vec<&str> {
    specs.iter().map(|spec| spec.name.as_str()).collect()
}

#[test]
fn a_missing_server_is_installed_once_in_the_background_and_used_after() {
    let data = tempfile::tempdir().expect("data");
    let starts = Starts::default();
    let installs = Installs::held();
    let pool = LspPool::installing(starts.clone(), IDLE, data.path().into(), installs.clone());
    assert!(names(&pool.available(&defaults())).contains(&"typescript"));
    let busy = open_waiting(&pool, "/no-such-w/app.ts", Wait::Never).expect_err("still installing");
    assert!(busy.contains("typescript") && busy.contains("installing"), "{busy}");
    assert!(open_waiting(&pool, "/no-such-w/b.ts", Wait::Never).is_err());
    installs.release();
    let opened = open(&pool, "/no-such-w/app.ts").expect("installed");
    assert_eq!(opened.language, "typescript");
    let home = data.path().join("typescript");
    assert_eq!(*installs.asked.lock().expect("asked"), [(typescript().recipe, home.clone())]);
    assert_eq!(*starts.programs.lock().expect("programs"), [home.join(typescript().program)]);
    let again = LspPool::installing(Starts::default(), IDLE, data.path().into(), Installs::failing("no"));
    open(&again, "/no-such-w/app.ts").expect("already installed");
}

#[test]
fn a_failed_install_says_why_and_is_tried_again_only_after_a_while() {
    let data = tempfile::tempdir().expect("data");
    let installs = Installs::failing("npm was not found");
    let pool = LspPool::installing(Starts::default(), IDLE, data.path().into(), installs.clone());
    let failed = open(&pool, "/no-such-w/app.ts").expect_err("failed");
    assert!(failed.contains("typescript") && failed.contains("npm was not found"), "{failed}");
    assert!(open(&pool, "/no-such-w/app.ts").expect_err("still").contains("npm was not found"));
    assert_eq!(installs.count(), 1);
    pool.sweep(Instant::now() + IDLE * 2);
    assert!(open(&pool, "/no-such-w/app.ts").is_err());
    assert_eq!(installs.count(), 2);
}

#[test]
fn without_installing_only_the_servers_on_path_are_available() {
    let starts = Starts { installed: vec!["gopls"], ..Default::default() };
    assert_eq!(names(&pool(&starts).available(&defaults())), ["gopls"]);
    assert!(pool(&Starts::default()).available(&defaults()).is_empty());
}

#[test]
fn a_server_is_started_once_per_root_and_shared_after() {
    let starts = Starts { installed: vec!["rust-analyzer"], ..Default::default() };
    let pool = pool(&starts);
    let first = open(&pool, "/no-such-w/src/a.rs").expect("first");
    assert_eq!((first.language.as_str(), first.root.as_path()), ("rust", Path::new("/no-such-w")));
    open(&pool, "/no-such-w/src/b.rs").expect("same root");
    assert_eq!(starts.count(), 1);
    open(&pool, "/elsewhere-x/c.rs").expect("another root");
    assert_eq!(starts.count(), 2);
}

#[test]
fn a_missing_or_failed_server_says_why_and_is_not_retried_in_a_loop() {
    let starts = Starts { installed: vec!["gopls"], broken: vec!["gopls"], ..Default::default() };
    let pool = pool(&starts);
    let none = open(&pool, "/no-such-w/notes.txt").expect_err("no server");
    assert!(none.contains(".txt"), "{none}");
    let missing = open(&pool, "/no-such-w/a.rs").expect_err("not installed");
    assert!(missing.contains("rust-analyzer") && missing.contains("not installed"), "{missing}");
    assert!(open(&pool, "/no-such-w/main.go").expect_err("crashed").contains("crashed"));
    assert!(open(&pool, "/no-such-w/main.go").expect_err("still").contains("crashed"));
    assert_eq!(starts.count(), 1);
}

#[test]
fn a_server_that_ended_is_started_again_and_an_idle_one_is_let_go() {
    let starts = Starts { installed: vec!["rust-analyzer"], ..Default::default() };
    let pool = pool(&starts);
    open(&pool, "/no-such-w/a.rs").expect("first");
    starts.servers.lock().expect("servers")[0].alive.store(false, Ordering::Relaxed);
    let again = open(&pool, "/no-such-w/a.rs").expect("restarted");
    assert!(again.server.is_alive());
    assert_eq!(starts.count(), 2);
    pool.sweep(Instant::now() + IDLE / 2);
    open(&pool, "/no-such-w/a.rs").expect("kept");
    assert_eq!(starts.count(), 2);
    pool.sweep(Instant::now() + IDLE * 2);
    open(&pool, "/no-such-w/a.rs").expect("started anew");
    assert_eq!(starts.count(), 3);
    assert!(starts.servers.lock().expect("servers").iter().all(|s| s.root == Path::new("/no-such-w")));
}

#[test]
fn an_edit_says_its_server_is_installing_and_the_tool_says_it_installed_one() {
    let data = tempfile::tempdir().expect("data");
    let installs = Installs::held();
    let pool = LspPool::installing(Starts::default(), IDLE, data.path().into(), installs.clone());
    let lsp = crate::lsp::Lsp { servers: Arc::new(pool), specs: Arc::new(defaults()) };
    let (file, cwd, stop) = (Path::new("/no-such-w/app.ts"), Path::new("/no-such-w"), AtomicBool::new(false));
    assert_eq!(
        lsp.problems(file, "", cwd, &stop).as_deref(),
        Some("<diagnostics>\ntypescript is still installing, so this file was not checked; later edits will be.\n</diagnostics>")
    );
    let waiting = {
        let installs = installs.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            installs.release();
        })
    };
    let (_, installed) = lsp.open(file, cwd, &stop).expect("installed");
    waiting.join().expect("released");
    assert_eq!(installed.as_deref(), Some("typescript"));
    let (_, again) = lsp.open(file, cwd, &stop).expect("ready");
    assert_eq!(again, None);
    assert_eq!(lsp.problems(file, "", cwd, &stop), None);
}
