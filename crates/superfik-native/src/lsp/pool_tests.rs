use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use superfik_core::lsp::{ServerSpec, defaults};

use super::{LspPool, Starter};
use crate::lsp::{LanguageServer, LanguageServers};

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

/// Starts fakes for the installed commands and records each start.
#[derive(Clone, Default)]
struct Starts {
    installed: Vec<&'static str>,
    broken: Vec<&'static str>,
    started: Arc<Mutex<Vec<(String, PathBuf)>>>,
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
        assert_eq!(program, Path::new("/bin").join(&spec.command));
        self.started.lock().expect("starts").push((spec.name.clone(), root.to_path_buf()));
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
    pool.open(Path::new(file), &defaults(), Path::new("/no-such-w"), &AtomicBool::new(false))
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
    assert!(pool.any_installed(&defaults()));
    assert!(!LspPool::new(Starts::default(), IDLE).any_installed(&defaults()));
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
