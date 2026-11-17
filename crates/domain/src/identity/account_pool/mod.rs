use serde::{Deserialize, Serialize};

use crate::fallback::Failure;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Balance {
    #[default]
    Off,
    Failover,
    RoundRobin,
}

impl Balance {
    pub const ALL: [Self; 3] = [Self::Off, Self::Failover, Self::RoundRobin];

    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Failover => "failover",
            Self::RoundRobin => "round-robin",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Self::Off => "only the account in use",
            Self::Failover => "the account in use, then the others when it is busy or signed out",
            Self::RoundRobin => "each run starts on the next account, and the others take over when one is busy",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        let wanted = text.trim();
        Self::ALL.into_iter().find(|balance| balance.name().eq_ignore_ascii_case(wanted))
    }
}

pub fn order(balance: Balance, count: usize, active: usize, run: u64) -> Vec<usize> {
    if count == 0 {
        return Vec::new();
    }
    let active = active.min(count - 1);
    let start = match balance {
        Balance::Off | Balance::Failover => active,
        Balance::RoundRobin => usize::try_from(run % count as u64).unwrap_or(0),
    };
    let tried = if balance == Balance::Off { 1 } else { count };
    (0..tried).map(|offset| (start + offset) % count).collect()
}

pub fn hands_over(failure: Failure) -> bool {
    matches!(failure, Failure::Overloaded | Failure::Unauthorized)
}

#[cfg(test)]
mod tests;
