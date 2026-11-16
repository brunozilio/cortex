use serde::{Deserialize, Serialize};

use crate::{Decision, EngineEvent};

pub const HOLDOUT_EVERY: u32 = 20;
const ENOUGH_HELD_OUT: u32 = 5;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct RunOutcome {
    pub completed: bool,
    pub corrections: u32,
    pub cost_usd: Option<f64>,
}

impl RunOutcome {
    pub fn observe(&mut self, event: &EngineEvent) {
        match event {
            EngineEvent::Steered(_)
            | EngineEvent::Answered { decision: Decision::Deny | Decision::DenyWith(_), .. } => {
                self.corrections += 1;
            }
            EngineEvent::Completed { cost_usd, .. } => {
                self.completed = true;
                self.cost_usd = *cost_usd;
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Tally {
    pub runs: u32,
    pub corrections: u32,
    pub cost_usd: f64,
}

impl Tally {
    fn line(&self, label: &str) -> String {
        let runs = f64::from(self.runs.max(1));
        format!(
            "{label}: {} runs, {:.1} corrections per 100 runs, ${:.2} per run",
            self.runs,
            f64::from(self.corrections) * 100.0 / runs,
            self.cost_usd / runs
        )
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Scorecard {
    pub with: Tally,
    pub without: Tally,
    #[serde(default)]
    pub briefed: u32,
}

impl Scorecard {
    pub fn next_held_out(&mut self) -> bool {
        self.briefed = self.briefed.wrapping_add(1);
        self.briefed.is_multiple_of(HOLDOUT_EVERY)
    }

    pub fn record(&mut self, held_out: bool, outcome: &RunOutcome) {
        if !outcome.completed {
            return;
        }
        let tally = if held_out { &mut self.without } else { &mut self.with };
        tally.runs += 1;
        tally.corrections += outcome.corrections;
        tally.cost_usd += outcome.cost_usd.unwrap_or_default();
    }

    pub fn report(&self) -> String {
        let mut lines = vec![self.with.line("With memory"), self.without.line("Without memory")];
        if self.without.runs < ENOUGH_HELD_OUT {
            lines.push(format!(
                "Not enough runs without memory yet: one run in {HOLDOUT_EVERY} goes without, and {ENOUGH_HELD_OUT} are needed to compare."
            ));
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests;
