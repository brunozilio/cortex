use super::*;

pub(super) fn until(what: &str, met: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !met() {
        assert!(Instant::now() < deadline, "never happened: {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn with_npm() -> Starts {
    Starts { installed: vec!["npm"], ..Default::default() }
}

struct Crashing;

impl Installer for Crashing {
    fn install(&self, _: &Install, _: &Path, _: &AtomicBool) -> Result<(), String> {
        panic!("the unpacker crashed");
    }
}

#[derive(Clone, Default)]
struct Slow {
    started: Arc<Mutex<usize>>,
    stopped: Arc<AtomicBool>,
}

impl Installer for Slow {
    fn install(&self, install: &Install, dir: &Path, stop: &AtomicBool) -> Result<(), String> {
        let attempt = {
            let mut started = self.started.lock().expect("started");
            *started += 1;
            *started
        };
        if attempt == 1 {
            while !stop.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(5));
            }
            self.stopped.store(true, Ordering::Relaxed);
            return Err("it was stopped".into());
        }
        let program = dir.join(&install.program);
        std::fs::create_dir_all(program.parent().expect("folder")).map_err(|e| e.to_string())?;
        std::fs::write(&program, "").map_err(|e| e.to_string())
    }
}

#[test]
fn an_install_ends_when_the_only_caller_waiting_for_it_stops_and_the_next_request_starts_it_again() {
    let data = tempfile::tempdir().expect("data");
    let installs = Slow::default();
    let pool = Arc::new(LspPool::installing(with_npm(), IDLE, data.path().into(), installs.clone()));
    let stop = Arc::new(AtomicBool::new(false));
    let waiting = {
        let (pool, stop) = (Arc::clone(&pool), Arc::clone(&stop));
        std::thread::spawn(move || {
            pool.open(Path::new("/no-such-w/app.ts"), &defaults(), Path::new("/no-such-w"), &stop, Wait::ForInstall)
                .map(|_| ())
        })
    };
    until("the install started", || *installs.started.lock().expect("started") == 1);
    stop.store(true, Ordering::Relaxed);
    let stopped = waiting.join().expect("joined").expect_err("stopped");
    assert!(stopped.contains("Stopped while typescript was installing"), "{stopped}");
    until("the installer was told to stop", || installs.stopped.load(Ordering::Relaxed));
    until("the stopped install is forgotten", || names(&pool.available(&defaults())).contains(&"typescript"));
    open(&pool, "/no-such-w/app.ts").expect("installed by the next request");
    assert_eq!(*installs.started.lock().expect("started"), 2);
}

#[test]
fn a_failed_install_is_tried_again_after_a_short_while() {
    let data = tempfile::tempdir().expect("data");
    let installs = Installs::failing("the registry is unreachable");
    let retry = Duration::from_millis(300);
    let pool = LspPool::installing(with_npm(), IDLE, data.path().into(), installs.clone()).retrying_after(retry);
    assert!(open(&pool, "/no-such-w/app.ts").expect_err("failed").contains("unreachable"));
    assert!(open(&pool, "/no-such-w/app.ts").expect_err("too soon").contains("unreachable"));
    assert_eq!(installs.count(), 1);
    std::thread::sleep(retry + Duration::from_millis(50));
    assert!(names(&pool.available(&defaults())).contains(&"typescript"), "it is offered again");
    assert!(open(&pool, "/no-such-w/app.ts").is_err());
    assert_eq!(installs.count(), 2, "the install ran again");
}

#[test]
fn a_sweep_during_an_install_keeps_the_server_started_after_it() {
    let data = tempfile::tempdir().expect("data");
    let starts = with_npm();
    let installs = Installs::held();
    let pool = Arc::new(LspPool::installing(starts.clone(), IDLE, data.path().into(), installs.clone()));
    let waiting = {
        let pool = Arc::clone(&pool);
        std::thread::spawn(move || open(&pool, "/no-such-w/app.ts").map(|_| ()))
    };
    until("the install started", || installs.count() == 1);
    pool.sweep(Instant::now());
    installs.release();
    waiting.join().expect("joined").expect("installed");
    open(&pool, "/no-such-w/b.ts").expect("shared");
    assert_eq!(starts.count(), 1);
}

#[test]
fn an_installer_that_crashes_fails_the_install_instead_of_hanging_it() {
    let data = tempfile::tempdir().expect("data");
    let pool = LspPool::installing(with_npm(), IDLE, data.path().into(), Crashing);
    let stop = Arc::new(AtomicBool::new(false));
    let giving_up = Arc::clone(&stop);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3));
        giving_up.store(true, Ordering::Relaxed);
    });
    let failed = pool
        .open(Path::new("/no-such-w/app.ts"), &defaults(), Path::new("/no-such-w"), &stop, Wait::ForInstall)
        .expect_err("crashed");
    assert!(failed.contains("Could not install typescript") && failed.contains("crashed"), "{failed}");
}

#[test]
fn an_install_a_call_would_start_is_named_before_it_starts_and_nothing_once_installed() {
    let data = tempfile::tempdir().expect("data");
    let installs = Installs::succeeding();
    let pool = LspPool::installing(with_npm(), IDLE, data.path().into(), installs.clone());
    let file = Path::new("/no-such-w/app.ts");
    let pending = pool.pending_install(file, &defaults()).expect("would install");
    assert!(pending.contains("typescript") && pending.contains("--ignore-scripts"), "{pending}");
    assert_eq!(installs.count(), 0, "naming it installs nothing");
    open(&pool, "/no-such-w/app.ts").expect("installed");
    assert_eq!(pool.pending_install(file, &defaults()), None);
    let on_path = LspPool::installing(
        Starts { installed: vec!["typescript-language-server"], ..Default::default() },
        IDLE,
        data.path().join("other"),
        Installs::succeeding(),
    );
    assert_eq!(on_path.pending_install(file, &defaults()), None, "a server on PATH needs no install");
    assert_eq!(pool.pending_install(Path::new("/no-such-w/README"), &defaults()), None);
}

#[test]
fn a_refused_install_never_starts_and_says_why() {
    let data = tempfile::tempdir().expect("data");
    let installs = Installs::succeeding();
    let lsp = crate::Lsp {
        servers: Arc::new(LspPool::installing(with_npm(), IDLE, data.path().into(), installs.clone())),
        specs: Arc::new(defaults()),
    };
    let stop = AtomicBool::new(false);
    let refused = lsp
        .open(Path::new("/no-such-w/app.ts"), Path::new("/no-such-w"), &stop, crate::Installing::Refused("no"))
        .expect_err("refused");
    assert!(refused.contains("install the typescript language server") && refused.contains("but no"), "{refused}");
    assert_eq!(installs.count(), 0);
    let (_, installed) = lsp
        .open(Path::new("/no-such-w/app.ts"), Path::new("/no-such-w"), &stop, crate::Installing::Allowed)
        .expect("allowed");
    assert_eq!(installed.as_deref(), Some("typescript"));
    assert_eq!(installs.count(), 1);
}

#[test]
fn a_server_is_offered_only_when_the_tool_that_installs_it_is_there() {
    let data = tempfile::tempdir().expect("data");
    let bare = LspPool::installing(Starts::default(), IDLE, data.path().into(), Installs::succeeding());
    let offered = bare.available(&defaults());
    let offered = names(&offered);
    assert!(offered.contains(&"rust-analyzer"), "{offered:?}");
    assert!(
        !offered.contains(&"typescript") && !offered.contains(&"pyright") && !offered.contains(&"gopls"),
        "{offered:?}"
    );
    let node = LspPool::installing(with_npm(), IDLE, data.path().into(), Installs::succeeding());
    let offered = node.available(&defaults());
    let offered = names(&offered);
    assert!(
        offered.contains(&"typescript") && offered.contains(&"pyright") && !offered.contains(&"gopls"),
        "{offered:?}"
    );
}
