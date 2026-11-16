use chrono::{DateTime, Datelike, SecondsFormat, TimeZone, Utc};
use domain::console::usage;
use serde_json::Value;

use crate::feature::Context;

const ORGANIZATION: &str = "organization";

pub fn handle(context: &Context<'_>, input: Value) -> Result<Value, String> {
    answer(context, &input, Utc::now())
}

fn answer(context: &Context<'_>, input: &Value, now: DateTime<Utc>) -> Result<Value, String> {
    let op = input["op"].as_str().ok_or("the usage feature needs an op")?;
    match op {
        "report" => {
            let stored = context.services.database.preference(ORGANIZATION)?.unwrap_or(Value::Null);
            let since = match input["since"].as_str() {
                Some(since) => DateTime::parse_from_rfc3339(since)
                    .map_err(|_| "since must be an ISO date and time".to_owned())?
                    .with_timezone(&Utc),
                None => month_start(now)?,
            };
            Ok(usage::report(&stored, &since.to_rfc3339_opts(SecondsFormat::Millis, true)))
        }
        "plans" => Ok(usage::plans(&now.format("%Y-%m").to_string())),
        other => Err(format!("the usage feature has no op {other}")),
    }
}

fn month_start(now: DateTime<Utc>) -> Result<DateTime<Utc>, String> {
    Utc.with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0).single().ok_or_else(|| "no start of the month".into())
}

pub fn forget(_client: u64) {}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::feature::testing;

    fn usage(input: Value) -> Result<Value, String> {
        let mut services = testing::services(None);
        services.database = Arc::new(store::Memory::default());
        let now = Utc.with_ymd_and_hms(2026, 11, 16, 15, 4, 5).single().ok_or("a date")?;
        testing::with(1, &services, |context| answer(context, &input, now))
    }

    #[test]
    fn the_report_starts_at_the_month_unless_asked_and_counts_nothing_yet() -> Result<(), String> {
        let report = usage(json!({ "op": "report" }))?;
        assert_eq!(report["since"], "2026-11-01T00:00:00.000Z");
        assert_eq!(report["totalUsd"], 0);
        assert_eq!(report["defaultAgentBudgetUsd"], 50.0);
        let asked = usage(json!({ "op": "report", "since": "2026-10-03T12:00:00-03:00" }))?;
        assert_eq!(asked["since"], "2026-10-03T15:00:00.000Z");
        assert!(usage(json!({ "op": "report", "since": "yesterday" })).is_err());
        Ok(())
    }

    #[test]
    fn plan_usage_names_this_month_with_no_accounts() {
        assert_eq!(
            usage(json!({ "op": "plans" })),
            Ok(json!({ "month": "2026-11", "money": true, "accounts": [], "api": [] }))
        );
    }
}
