use std::cell::Cell;

thread_local! {
    static RAW: Cell<bool> = const { Cell::new(false) };
}

struct Raw;

impl Drop for Raw {
    fn drop(&mut self) {
        RAW.with(|raw| raw.set(false));
    }
}

pub fn reading<T>(work: impl FnOnce() -> T) -> T {
    RAW.with(|raw| raw.set(true));
    let _raw = Raw;
    work()
}

pub fn active() -> bool {
    RAW.with(Cell::get)
}

#[cfg(test)]
mod tests {
    use super::{active, reading};

    #[test]
    fn raw_text_is_asked_for_only_inside_its_own_call_and_thread() {
        assert!(!active());
        assert!(reading(active));
        assert!(!active(), "it ends with the call");
        let other = reading(|| std::thread::spawn(active).join().expect("thread"));
        assert!(!other, "another thread is never in raw mode");
    }
}
