use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureKind {
    UsageLimit { resets_at: Option<i64> },
    Unavailable,
    Other,
}

impl FailureKind {
    pub fn classify(message: &str, status: Option<u16>) -> Self {
        let lower = message.to_ascii_lowercase();
        if lower.contains("usage limit") || lower.contains("rate limit") || status == Some(429) {
            return Self::UsageLimit { resets_at: reset_at(message) };
        }
        let down = matches!(status, Some(500 | 502 | 503 | 504 | 529))
            || ["overloaded", "internal server error", "unable to connect", "api error"]
                .iter()
                .any(|needle| lower.contains(needle));
        if down { Self::Unavailable } else { Self::Other }
    }
}

fn reset_at(message: &str) -> Option<i64> {
    let (_, tail) = message.rsplit_once('|')?;
    tail.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_usage_limit_names_its_reset_when_the_message_carries_one() {
        let kind = FailureKind::classify("Claude AI usage limit reached|1761900000", None);
        assert_eq!(kind, FailureKind::UsageLimit { resets_at: Some(1_761_900_000) });
    }

    #[test]
    fn a_usage_limit_without_a_reset_time_still_counts_as_a_limit() {
        assert_eq!(
            FailureKind::classify("Claude AI usage limit reached", None),
            FailureKind::UsageLimit { resets_at: None }
        );
        assert_eq!(FailureKind::classify("oops", Some(429)), FailureKind::UsageLimit { resets_at: None });
    }

    #[test]
    fn an_overloaded_or_failing_api_is_unavailable() {
        assert_eq!(FailureKind::classify("API Error: 529 overloaded", Some(529)), FailureKind::Unavailable);
        assert_eq!(FailureKind::classify("Unable to connect to API", None), FailureKind::Unavailable);
        assert_eq!(FailureKind::classify("boom", Some(503)), FailureKind::Unavailable);
    }

    #[test]
    fn everything_else_is_other() {
        assert_eq!(FailureKind::classify("Claude Code stopped: error_max_turns", None), FailureKind::Other);
        assert_eq!(FailureKind::classify("bad request", Some(400)), FailureKind::Other);
    }
}
