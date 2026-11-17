use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static LAST: AtomicU64 = AtomicU64::new(0);

pub(super) fn next() -> u64 {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_micros() as u64);
    let mut last = LAST.load(Ordering::SeqCst);
    loop {
        let next = now.max(last + 1);
        match LAST.compare_exchange(last, next, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return next,
            Err(newer) => last = newer,
        }
    }
}

pub(super) fn follows(seen: u64) {
    LAST.fetch_max(seen, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_number_is_greater_than_the_one_before_and_than_any_seen_on_file() {
        let first = next();
        let second = next();
        assert!(second > first);
        let far_ahead = second + 10_000_000_000;
        follows(far_ahead);
        assert!(next() > far_ahead, "a clock set back never orders a new entry before a kept one");
    }
}
