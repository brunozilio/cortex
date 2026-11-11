use std::cell::Cell;

thread_local! {
    static AGENT: Cell<Option<usize>> = const { Cell::new(None) };
}

pub fn agent() -> Option<usize> {
    AGENT.with(Cell::get)
}

pub(super) fn within<T>(index: usize, run: impl FnOnce() -> T) -> T {
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            AGENT.with(|agent| agent.set(self.0));
        }
    }
    let _restore = Restore(AGENT.with(|agent| agent.replace(Some(index))));
    run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_running_agent_is_known_on_its_thread_and_restored_after() {
        assert_eq!(agent(), None);
        within(3, || {
            assert_eq!(agent(), Some(3));
            within(5, || assert_eq!(agent(), Some(5)));
            assert_eq!(agent(), Some(3));
            std::thread::spawn(|| assert_eq!(agent(), None)).join().expect("thread");
        });
        assert_eq!(agent(), None);
    }
}
