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
    fn install(&self, _: &Install, _: &Path) -> Result<(), String> {
        panic!("the unpacker crashed");
    }
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
