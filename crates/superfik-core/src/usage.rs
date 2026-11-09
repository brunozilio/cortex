//! How full an engine session's context window is, and what fills it.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContextSlice {
    SystemPrompt,
    Tools,
    Mcp,
    Agents,
    Memory,
    Skills,
    Messages,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextShare {
    pub slice: ContextSlice,
    pub tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ContextUsage {
    pub used: u64,
    pub window: u64,
    pub reserved: u64,
    pub shares: Vec<ContextShare>,
}

impl ContextUsage {
    pub fn fraction(&self) -> f32 {
        if self.window == 0 { 0.0 } else { (self.used as f64 / self.window as f64).clamp(0.0, 1.0) as f32 }
    }

    pub fn percent(&self) -> u32 {
        (self.fraction() * 100.0).round() as u32
    }

    pub fn free(&self) -> u64 {
        self.window.saturating_sub(self.used).saturating_sub(self.reserved)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Compaction {
    pub before: u64,
    pub after: u64,
}

impl Compaction {
    pub fn freed(&self) -> u64 {
        self.before.saturating_sub(self.after)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionCommand {
    MeasureContext,
    Compact,
    Title,
    Suggest,
    Recap,
    Side,
    HeadsUp,
    Rename,
}

impl SessionCommand {
    pub fn is_chore(self) -> bool {
        matches!(self, Self::Title | Self::Suggest | Self::Recap | Self::Side | Self::HeadsUp | Self::Rename)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeadsUp {
    pub urgent: bool,
    pub learn: String,
    pub title: String,
    pub explain: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChoreAnswer {
    Title(String),
    Suggestion(String),
    Recap(String),
    Side(String),
    HeadsUp(HeadsUp),
}

pub fn compact_tokens(tokens: u64) -> String {
    let scaled = |value: f64, unit: &str| {
        let text = if value >= 100.0 { format!("{value:.0}") } else { format!("{value:.1}") };
        format!("{}{unit}", text.trim_end_matches(".0"))
    };
    match tokens {
        0..1_000 => tokens.to_string(),
        1_000..1_000_000 => scaled(tokens as f64 / 1_000.0, "k"),
        _ => scaled(tokens as f64 / 1_000_000.0, "m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_free_and_percent() {
        let usage = ContextUsage { used: 164_800, window: 1_000_000, reserved: 33_000, shares: vec![] };
        assert_eq!(usage.percent(), 16);
        assert_eq!(usage.free(), 802_200);
        assert_eq!(ContextUsage::default().fraction(), 0.0);
        let over = ContextUsage { used: 300_000, window: 200_000, reserved: 33_000, shares: vec![] };
        assert_eq!((over.fraction(), over.free()), (1.0, 0));
    }

    #[test]
    fn tokens_read_like_the_engine_prints_them() {
        assert_eq!(compact_tokens(435), "435");
        assert_eq!(compact_tokens(4_200), "4.2k");
        assert_eq!(compact_tokens(6_000), "6k");
        assert_eq!(compact_tokens(164_800), "165k");
        assert_eq!(compact_tokens(1_000_000), "1m");
    }

    #[test]
    fn compaction_reports_what_it_freed() {
        assert_eq!(Compaction { before: 19_875, after: 4_917 }.freed(), 14_958);
        assert_eq!(Compaction { before: 10, after: 20 }.freed(), 0);
    }
}
