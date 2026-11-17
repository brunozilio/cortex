use app::SessionId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    pub session: SessionId,
    pub title: Option<String>,
    pub at: u64,
}

pub fn ago(at: u64, now: u64) -> String {
    let seconds = now.saturating_sub(at);
    match seconds {
        0..60 => "just now".to_owned(),
        60..3_600 => format!("{}m ago", seconds / 60),
        3_600..86_400 => format!("{}h ago", seconds / 3_600),
        _ => format!("{}d ago", seconds / 86_400),
    }
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn how_long_ago_is_said_in_the_largest_whole_unit() {
        assert_eq!(ago(100, 130), "just now");
        assert_eq!(ago(100, 100 + 300), "5m ago");
        assert_eq!(ago(100, 100 + 7_200), "2h ago");
        assert_eq!(ago(100, 100 + 3 * 86_400), "3d ago");
    }
}
