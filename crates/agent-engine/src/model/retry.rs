use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::{Delta, ModelError, Reply};

const ATTEMPTS: u32 = 4;
const STEP: Duration = Duration::from_millis(25);

pub fn with_retries(
    provider: &str,
    backoff: Duration,
    stop: &AtomicBool,
    on: &mut dyn FnMut(Delta),
    mut once: impl FnMut(&mut dyn FnMut(Delta)) -> Result<Reply, ModelError>,
) -> Result<Reply, ModelError> {
    let mut wait = backoff;
    let mut wrote = false;
    for attempt in 1..=ATTEMPTS {
        let mut seen = |delta: Delta| {
            wrote = true;
            on(delta);
        };
        match once(&mut seen) {
            Err(error) if error.is_transient() && !wrote && attempt < ATTEMPTS => {
                tracing::warn!(%error, attempt, "{provider} is busy, retrying");
                sleep_stopped(wait, stop)?;
                wait *= 2;
            }
            other => return other,
        }
    }
    Err(ModelError::Overloaded("no answer after retrying".into()))
}

fn sleep_stopped(wait: Duration, stop: &AtomicBool) -> Result<(), ModelError> {
    let until = Instant::now() + wait;
    loop {
        if stop.load(Ordering::Relaxed) {
            return Err(ModelError::Cancelled);
        }
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(());
        }
        std::thread::sleep(left.min(STEP));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::{Message, StopReason, Usage};
    use std::sync::Arc;

    fn reply() -> Reply {
        Reply { message: Message::assistant(Vec::new()), stop: StopReason::EndTurn, usage: Usage::default() }
    }

    #[test]
    fn a_busy_provider_is_asked_again_until_it_answers() {
        let mut asked = 0;
        let result = with_retries("P", Duration::ZERO, &AtomicBool::new(false), &mut |_| {}, |_| {
            asked += 1;
            if asked < 3 { Err(ModelError::Overloaded("busy".into())) } else { Ok(reply()) }
        });
        assert!(result.is_ok());
        assert_eq!(asked, 3);
    }

    #[test]
    fn an_answer_already_shown_is_never_asked_again() {
        let mut asked = 0;
        let result = with_retries("P", Duration::ZERO, &AtomicBool::new(false), &mut |_| {}, |on| {
            asked += 1;
            on(Delta::Text("half".into()));
            Err(ModelError::Network("dropped".into()))
        });
        assert!(matches!(result, Err(ModelError::Network(_))));
        assert_eq!(asked, 1);
    }

    #[test]
    fn a_provider_busy_every_time_gives_up() {
        let result = with_retries("P", Duration::ZERO, &AtomicBool::new(false), &mut |_| {}, |_| {
            Err(ModelError::Overloaded("busy".into()))
        });
        assert!(matches!(result, Err(ModelError::Overloaded(_))));
    }

    #[test]
    fn stopping_during_the_wait_ends_it_at_once() {
        let stop = Arc::new(AtomicBool::new(false));
        let stopper = Arc::clone(&stop);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            stopper.store(true, Ordering::Relaxed);
        });
        let started = Instant::now();
        let result = with_retries("P", Duration::from_secs(30), &stop, &mut |_| {}, |_| {
            Err(ModelError::Overloaded("busy".into()))
        });
        assert!(matches!(result, Err(ModelError::Cancelled)));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
