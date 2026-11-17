pub const LASTS_SECS: u64 = 300;
const WARN_SECS: u64 = 120;
const WORTH_TOKENS: u64 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastCall {
    pub at_secs: u64,
    pub cached_tokens: u64,
}

impl LastCall {
    pub fn of(usage: &crate::billing::ModelUsage, at_secs: u64) -> Self {
        Self { at_secs, cached_tokens: usage.cache_read + usage.cache_write }
    }
}

pub fn marker(last: &LastCall, now_secs: u64) -> Option<String> {
    if last.cached_tokens < WORTH_TOKENS {
        return None;
    }
    let idle = now_secs.saturating_sub(last.at_secs);
    let left = LASTS_SECS.checked_sub(idle).filter(|left| *left > 0);
    match left {
        Some(left) if left <= WARN_SECS => Some(format!("cache {}:{:02}", left / 60, left % 60)),
        Some(_) => None,
        None => Some(format!("cache cold: next turn rewrites {}k tokens", last.cached_tokens / 1000)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_marker_counts_down_the_last_two_minutes_then_says_the_cache_is_cold() {
        let last = LastCall { at_secs: 1_000, cached_tokens: 48_000 };
        assert_eq!(marker(&last, 1_100), None, "plenty of time left");
        assert_eq!(marker(&last, 1_195), Some("cache 1:45".into()));
        assert_eq!(marker(&last, 1_299), Some("cache 0:01".into()));
        assert_eq!(marker(&last, 1_300), Some("cache cold: next turn rewrites 48k tokens".into()));
    }

    #[test]
    fn a_small_cache_is_not_worth_a_warning() {
        let last = LastCall { at_secs: 0, cached_tokens: 2_000 };
        assert_eq!(marker(&last, 1_000), None);
    }
}
