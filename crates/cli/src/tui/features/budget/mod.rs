use app::Spending;
use domain::budget::{BudgetCommand, parse};

use super::{Feature, Setting, Slash};
use crate::tui::app::App;

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash {
            names: &["budget"],
            about: "Cap what this session spends: /budget <dollars>, or off",
            run,
        }],
        status: Some(|app| app.session.live().map(app::Session::spending).and_then(|spending| marker(&spending))),
        setting: Some(Setting { name: "Budget", value: |_| String::new(), run: |app, _| run(app, None) }),
        ..Feature::default()
    }
}

fn run(app: &mut App, argument: Option<String>) {
    let command = match parse(argument.as_deref()) {
        Ok(command) => command,
        Err(error) => return app.transcript.notice(error),
    };
    let Some(session) = app.session.live() else {
        return app.transcript.notice("Pick a model first: a budget belongs to a conversation.");
    };
    let notice = match command {
        BudgetCommand::Show => shown(&session.spending()),
        BudgetCommand::Off => {
            session.set_budget(None);
            "No budget: this session spends without a cap.".into()
        }
        BudgetCommand::Set(limit) => {
            session.set_budget(Some(limit));
            format!(
                "Budget ${limit:.2}: model calls stop once this session has spent it. {}",
                shown(&session.spending())
            )
        }
    };
    app.transcript.notice(notice);
}

fn shown(spending: &Spending) -> String {
    match spending.limit_usd {
        Some(limit) => format!("Spent ${:.2} of ${limit:.2}.", spending.spent_usd),
        None => format!("Spent ${:.2}; no budget (/budget <dollars> sets one).", spending.spent_usd),
    }
}

fn marker(spending: &Spending) -> Option<String> {
    spending.limit_usd.map(|limit| format!("${:.2}/${limit:.2}", spending.spent_usd))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_shows_spent_against_the_budget_only_when_there_is_one() {
        let capped = Spending { limit_usd: Some(5.0), spent_usd: 1.234 };
        assert_eq!(marker(&capped), Some("$1.23/$5.00".into()));
        assert_eq!(shown(&capped), "Spent $1.23 of $5.00.");
        let open = Spending { limit_usd: None, spent_usd: 0.5 };
        assert_eq!(marker(&open), None);
        assert!(shown(&open).contains("no budget"));
    }
}
