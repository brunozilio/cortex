use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use platform::sync::Locked;

const EVERY: Duration = Duration::from_millis(150);
const LONGEST: usize = 200;

type Slot = Arc<Mutex<Option<String>>>;

thread_local! {
    static SLOT: RefCell<Option<Slot>> = const { RefCell::new(None) };
}

pub fn report(output: &str) {
    let Some(line) = last_line(output) else { return };
    SLOT.with(|slot| {
        if let Some(slot) = slot.borrow().as_ref() {
            *slot.locked() = Some(line);
        }
    });
}

pub fn last_line(output: &str) -> Option<String> {
    let line = output.lines().rev().map(str::trim).find(|line| !line.is_empty())?;
    Some(line.chars().take(LONGEST).collect())
}

pub fn watched<T>(show: &(dyn Fn(String) + Sync), run: impl FnOnce() -> T) -> T {
    let slot: Slot = Arc::default();
    let previous = SLOT.with(|current| current.replace(Some(Arc::clone(&slot))));
    let done = AtomicBool::new(false);
    let result = std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut shown: Option<String> = None;
            loop {
                let finished = done.load(Ordering::SeqCst);
                let latest = slot.locked().clone();
                if latest.is_some() && latest != shown {
                    if let Some(line) = latest.clone() {
                        show(line);
                    }
                    shown = latest;
                }
                if finished {
                    break;
                }
                std::thread::sleep(EVERY);
            }
        });
        let result = run();
        done.store(true, Ordering::SeqCst);
        result
    });
    SLOT.with(|current| current.replace(previous));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_line_with_text_is_what_shows() {
        assert_eq!(last_line("compiling a\ncompiling b\n\n").as_deref(), Some("compiling b"));
        assert_eq!(last_line("  \n"), None);
        assert_eq!(last_line(&"x".repeat(500)).map(|line| line.len()), Some(LONGEST));
    }

    #[test]
    fn lines_reported_while_watched_reach_the_viewer_and_nothing_leaks_after() {
        let shown = Mutex::new(Vec::new());
        let answer = watched(&|line| shown.locked().push(line), || {
            report("step 1\n");
            std::thread::sleep(EVERY * 2);
            report("step 1\nstep 2\n");
            std::thread::sleep(EVERY * 2);
            42
        });
        assert_eq!(answer, 42);
        assert_eq!(*shown.locked(), ["step 1", "step 2"]);
        report("after the tool");
        assert_eq!(shown.locked().len(), 2, "outside a watched call a report goes nowhere");
    }
}
