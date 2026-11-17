use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    pub limit_usd: f64,
    pub spent_usd: f64,
}

impl Budget {
    pub fn allows(&self, run_cost: f64) -> bool {
        self.spent_usd + run_cost < self.limit_usd
    }

    pub fn refusal(&self, run_cost: f64) -> String {
        format!(
            "Stopped: this session spent ${:.2} of its ${:.2} budget. Raise it with /budget <dollars>, or turn it off with /budget off.",
            self.spent_usd + run_cost,
            self.limit_usd
        )
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Spending {
    pub limit_usd: Option<f64>,
    pub spent_usd: f64,
}

impl Spending {
    pub fn observe(&mut self, event: &crate::EngineEvent) {
        self.spent_usd += spent(event);
    }

    pub fn budget(&self) -> Option<Budget> {
        self.limit_usd.map(|limit_usd| Budget { limit_usd, spent_usd: self.spent_usd })
    }

    pub fn allows(&self, run_cost: f64) -> bool {
        self.budget().is_none_or(|budget| budget.allows(run_cost))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BudgetCommand {
    Show,
    Off,
    Set(f64),
}

pub fn parse(argument: Option<&str>) -> Result<BudgetCommand, String> {
    let text = argument.unwrap_or_default().trim();
    match text {
        "" => Ok(BudgetCommand::Show),
        "off" | "none" => Ok(BudgetCommand::Off),
        _ => match text.trim_start_matches('$').parse::<f64>() {
            Ok(limit) if limit > 0.0 && limit.is_finite() => Ok(BudgetCommand::Set(limit)),
            _ => Err("Use /budget <dollars> (for example /budget 5), or /budget off.".into()),
        },
    }
}

pub fn spent(event: &crate::EngineEvent) -> f64 {
    match event {
        crate::EngineEvent::Usage(usage) => usage.estimated_cost_usd.unwrap_or(0.0),
        crate::EngineEvent::AgentEvent { event, .. } => spent(event),
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_budget_allows_calls_until_the_session_has_spent_it() {
        let budget = Budget { limit_usd: 5.0, spent_usd: 4.0 };
        assert!(budget.allows(0.5));
        assert!(!budget.allows(1.0));
        assert_eq!(
            budget.refusal(1.25),
            "Stopped: this session spent $5.25 of its $5.00 budget. Raise it with /budget <dollars>, or turn it off with /budget off."
        );
    }

    #[test]
    fn a_session_s_spending_adds_up_what_its_runs_cost_and_is_the_budget_of_the_next_one() {
        let usage = |cost| {
            crate::EngineEvent::Usage(crate::billing::ModelUsage {
                estimated_cost_usd: Some(cost),
                ..Default::default()
            })
        };
        let mut spending = Spending::default();
        spending.observe(&usage(1.5));
        spending.observe(&crate::EngineEvent::AgentEvent { index: 1, event: Box::new(usage(0.5)) });
        spending.observe(&crate::EngineEvent::TextDelta("hi".into()));
        assert!((spending.spent_usd - 2.0).abs() < f64::EPSILON);
        assert_eq!(spending.budget(), None, "no limit, no budget");
        assert!(spending.allows(1_000.0), "and nothing is refused");

        spending.limit_usd = Some(3.0);
        assert_eq!(spending.budget(), Some(Budget { limit_usd: 3.0, spent_usd: 2.0 }));
        assert!(spending.allows(0.5) && !spending.allows(1.0));
    }

    #[test]
    fn the_command_shows_sets_or_turns_off_the_budget() {
        assert_eq!(parse(None), Ok(BudgetCommand::Show));
        assert_eq!(parse(Some("off")), Ok(BudgetCommand::Off));
        assert_eq!(parse(Some("$2.50")), Ok(BudgetCommand::Set(2.5)));
        assert_eq!(parse(Some(" 10 ")), Ok(BudgetCommand::Set(10.0)));
        assert!(parse(Some("-1")).is_err());
        assert!(parse(Some("lots")).is_err());
    }

    #[test]
    fn spending_counts_sub_agents_too() {
        let usage = |cost| {
            crate::EngineEvent::Usage(crate::billing::ModelUsage {
                estimated_cost_usd: Some(cost),
                ..Default::default()
            })
        };
        assert!((spent(&usage(0.25)) - 0.25).abs() < f64::EPSILON);
        let nested = crate::EngineEvent::AgentEvent { index: 1, event: Box::new(usage(0.5)) };
        assert!((spent(&nested) - 0.5).abs() < f64::EPSILON);
        assert!(spent(&crate::EngineEvent::Cancelled).abs() < f64::EPSILON);
    }
}
