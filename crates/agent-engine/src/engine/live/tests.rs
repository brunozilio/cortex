use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use domain::{ModelOption, PromptRequest, SessionId};

use super::cache::{FromDisk, Load, Loaded, SessionCache};
use super::{LiveSession, LiveSessions, sources};
use crate::tools::Workspace;

fn id(name: &str) -> SessionId {
    SessionId(name.into())
}

fn request(cwd: &Path, home: &Path) -> PromptRequest {
    let mut request = PromptRequest::plain(cwd.into(), "hi", ModelOption::new("m", "M"));
    request.host.extensions = Some(domain::Extensions { home: Some(home.into()), ..Default::default() });
    request
}

#[test]
fn the_least_recently_used_idle_session_leaves_when_too_many_are_live() {
    let live = LiveSessions::holding(2);
    live.enter(&id("a"));
    live.enter(&id("b"));
    live.enter(&id("a"));

    live.enter(&id("c"));

    assert_eq!(live.ids(), ["a", "c"], "b was the least recently used");
}

#[test]
fn a_session_in_the_middle_of_a_run_is_not_let_go() {
    let live = LiveSessions::holding(1);
    let running = live.enter(&id("a"));

    live.enter(&id("b"));

    assert_eq!(live.ids(), ["a", "b"], "over the bound until a's run lets go");
    drop(running);
    live.enter(&id("c"));
    assert_eq!(live.ids(), ["b", "c"]);
}

#[test]
fn entering_a_live_session_again_gives_the_same_one() {
    let live = LiveSessions::default();
    let first = live.enter(&id("a"));
    let again = live.enter(&id("a"));
    assert!(Arc::ptr_eq(&first, &again));
}

struct Counting {
    loads: AtomicUsize,
    print: Mutex<Option<u64>>,
}

impl Load for Counting {
    fn fingerprint(&self, _request: &PromptRequest) -> Option<u64> {
        *self.print.lock().expect("print")
    }

    fn load(&self, request: &PromptRequest) -> Result<Loaded, String> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        FromDisk.load(request)
    }
}

#[test]
fn the_cache_loads_again_only_when_the_sources_changed() {
    let (repo, home) = (tempfile::tempdir().expect("repo"), tempfile::tempdir().expect("home"));
    let request = request(repo.path(), home.path());
    let counting = Counting { loads: AtomicUsize::new(0), print: Mutex::new(Some(1)) };
    let cache = SessionCache::default();

    let first = cache.loaded(&request, &counting).expect("first");
    let second = cache.loaded(&request, &counting).expect("second");
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(counting.loads.load(Ordering::SeqCst), 1);

    *counting.print.lock().expect("print") = Some(2);
    cache.loaded(&request, &counting).expect("changed");
    assert_eq!(counting.loads.load(Ordering::SeqCst), 2);

    *counting.print.lock().expect("print") = None;
    cache.loaded(&request, &counting).expect("unknown");
    cache.loaded(&request, &counting).expect("unknown again");
    assert_eq!(counting.loads.load(Ordering::SeqCst), 4, "sources too many to watch are read every run");
}

#[test]
fn the_fingerprint_follows_what_the_brief_reads_and_nothing_else() {
    let (repo, home) = (tempfile::tempdir().expect("repo"), tempfile::tempdir().expect("home"));
    let request = request(repo.path(), home.path());
    let print = || sources::fingerprint(request.folder()).expect("watched");
    std::fs::write(repo.path().join(brand::INSTRUCTIONS), "Use tabs.").expect("instructions");
    std::fs::write(repo.path().join("main.rs"), "fn main() {}").expect("code");
    let before = print();
    assert_eq!(print(), before, "nothing changed");

    std::fs::write(repo.path().join("main.rs"), "fn main() { run() }").expect("code");
    std::fs::create_dir_all(repo.path().join(brand::DIR).join("worktrees/w1")).expect("worktree");
    assert_eq!(print(), before, "code and the session's own folders are not the brief's");

    std::fs::write(repo.path().join(brand::INSTRUCTIONS), "Use spaces.").expect("instructions");
    let edited = print();
    assert_ne!(edited, before, "an edited instruction file");

    std::fs::create_dir_all(repo.path().join("web")).expect("web");
    std::fs::write(repo.path().join("web").join(brand::INSTRUCTIONS), "Use React.").expect("nested");
    let nested = print();
    assert_ne!(nested, edited, "a new nested instruction file");

    let skill = home.path().join(brand::DIR).join("skills/deploy");
    std::fs::create_dir_all(&skill).expect("skill");
    std::fs::write(skill.join("SKILL.md"), "---\ndescription: Ship\n---\nShip it.").expect("skill");
    let skilled = print();
    assert_ne!(skilled, nested, "a new skill of the owner's");

    std::fs::write(repo.path().join(brand::DIR).join("settings.local.json"), "{}").expect("settings");
    assert_ne!(print(), skilled, "a new settings file");
}

#[test]
fn a_session_that_started_ends_once_with_the_hooks_of_its_last_run() {
    let dir = tempfile::tempdir().expect("dir");
    let journal = dir.path().join("journal");
    let mut workspace = Workspace::new(dir.path());
    let hooks = serde_json::json!({"SessionEnd": [{"hooks": [
        {"type": "command", "command": format!("cat >> '{}'; echo >> '{}'", journal.display(), journal.display())}
    ]}]});
    workspace.hooks = crate::hooks::Hooks::parse(&hooks).expect("hooks");
    workspace.session_id = "s1".into();

    let started = LiveSession::new(id("s1"));
    assert!(started.begins_run(&workspace, dir.path().into()), "the first run starts the session");
    assert!(!started.begins_run(&workspace, dir.path().into()), "a later run does not");
    drop(started);
    let never = LiveSession::new(id("s2"));
    drop(never);

    let said = std::fs::read_to_string(&journal).expect("journal");
    assert_eq!(said.lines().count(), 1, "{said}");
    assert!(said.contains(r#""hook_event_name":"SessionEnd""#) && said.contains(r#""session_id":"s1""#), "{said}");
}
