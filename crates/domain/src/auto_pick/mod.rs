use crate::model::Effort;

const GO_AHEADS: [&str; 12] =
    ["ok", "okay", "yes", "y", "go", "go on", "continue", "proceed", "sim", "continua", "pode", "segue"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Tier {
    Small,
    Session,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Pick {
    pub tier: Tier,
    pub effort: Effort,
}

impl Pick {
    pub const FALLBACK: Self = Self { tier: Tier::Session, effort: Effort::Medium };

    pub fn parse(reply: &str) -> Option<Self> {
        let start = reply.find('{')?;
        let end = reply.rfind('}')?;
        let value: serde_json::Value = serde_json::from_str(reply.get(start..=end)?).ok()?;
        let tier = match value["tier"].as_str()? {
            "small" => Tier::Small,
            "large" | "session" => Tier::Session,
            _ => return None,
        };
        let effort = match value["effort"].as_str()? {
            "low" => Effort::Low,
            "medium" => Effort::Medium,
            "high" => Effort::High,
            _ => return None,
        };
        Some(Self { tier, effort })
    }
}

pub fn settled(prompt: &str) -> Option<Pick> {
    let bare = prompt.trim().trim_end_matches(['.', '!']).to_lowercase();
    GO_AHEADS.contains(&bare.as_str()).then_some(Pick { tier: Tier::Session, effort: Effort::Low })
}

#[cfg(test)]
mod tests;
