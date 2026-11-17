use std::hash::{BuildHasher, Hasher};
use std::time::{Duration, SystemTime};

use tokio_util::sync::CancellationToken;

use super::{BoxFuture, Delta, ModelError, OnDelta, Reply, TurnRequest};

const ATTEMPTS: u32 = 4;
pub const RETRY_AFTER_AT_MOST: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Failed {
    pub error: ModelError,
    pub retry_after: Option<Duration>,
}

impl From<ModelError> for Failed {
    fn from(error: ModelError) -> Self {
        Self { error, retry_after: None }
    }
}

pub(crate) trait Attempt: Sync {
    fn once<'a>(
        &'a self,
        request: &'a TurnRequest,
        on: OnDelta<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, Failed>>;
}

pub(crate) fn retry_after(headers: &[(String, String)], now: SystemTime) -> Option<Duration> {
    let header = |name: &str| {
        headers.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.trim().to_owned())
    };
    if let Some(millis) = header("retry-after-ms").and_then(|value| value.parse::<f64>().ok())
        && millis.is_finite()
        && millis >= 0.0
    {
        return Some(Duration::try_from_secs_f64(millis / 1000.0).unwrap_or(Duration::MAX));
    }
    let value = header("retry-after")?;
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let at = jiff::fmt::rfc2822::parse(&value).ok()?.timestamp();
    let now = jiff::Timestamp::try_from(now).ok()?;
    Some(Duration::try_from(at.duration_since(now)).unwrap_or(Duration::ZERO))
}

pub(crate) async fn refusal(
    response: super::http::Response,
    cancel: &CancellationToken,
    judge: fn(u16, &str) -> ModelError,
) -> Failed {
    let retry_after = retry_after(&response.headers, SystemTime::now());
    let status = response.status;
    match response.body.capped(cancel, ERROR_BODY_AT_MOST).await {
        Ok(bytes) => Failed { error: judge(status, &String::from_utf8_lossy(&bytes)), retry_after },
        Err(error) => error.into(),
    }
}

pub(crate) const ERROR_BODY_AT_MOST: usize = 64 * 1024;

pub(crate) fn jittered(wait: Duration, random: u64) -> Duration {
    let half = wait / 2;
    let spread = u64::try_from(half.as_nanos()).unwrap_or(u64::MAX);
    if spread == 0 {
        return wait;
    }
    half + Duration::from_nanos(random % (spread + 1))
}

fn random() -> u64 {
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos());
    hasher.finish()
}

pub(crate) fn pause(wait: Duration, retry_after: Option<Duration>) -> Option<Duration> {
    match retry_after {
        Some(asked) if asked > RETRY_AFTER_AT_MOST => None,
        Some(asked) => Some(asked),
        None => Some(jittered(wait, random())),
    }
}

pub(crate) async fn with_retries(
    provider: &str,
    backoff: Duration,
    attempt: &dyn Attempt,
    request: &TurnRequest,
    on: OnDelta<'_>,
    cancel: &CancellationToken,
) -> Result<Reply, ModelError> {
    let mut wait = backoff;
    for tried in 1..=ATTEMPTS {
        let mut wrote = false;
        let outcome = {
            let mut seen = |delta: Delta| {
                wrote = true;
                on(delta);
            };
            attempt.once(request, &mut seen, cancel).await
        };
        let failed = match outcome {
            Ok(reply) => return Ok(reply),
            Err(failed) if failed.error.is_transient() && !wrote && tried < ATTEMPTS => failed,
            Err(failed) => return Err(failed.error),
        };
        let Some(delay) = pause(wait, failed.retry_after) else { return Err(failed.error) };
        tracing::warn!(error = %failed.error, attempt = tried, ?delay, "{provider} is busy, retrying");
        tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(ModelError::Cancelled),
            () = tokio::time::sleep(delay) => {}
        }
        wait *= 2;
    }
    Err(ModelError::Overloaded("no answer after retrying".into()))
}

#[cfg(test)]
#[path = "retry_tests.rs"]
mod tests;
