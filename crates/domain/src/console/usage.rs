use serde_json::{Value, json};

use super::organization::{budget_nano_of, name_of, nano_to_usd};

pub fn overview(stored: &Value) -> Value {
    json!({
        "name": name_of(stored),
        "role": "owner",
        "people": 1,
        "agents": 0,
        "budgetUsedPercent": 0,
        "approvalsWaitingForMe": 0,
        "proposalsWaitingForMe": 0,
    })
}

pub fn limits() -> Value {
    json!({
        "members": { "used": 1, "limit": null, "max": null, "canAddSeats": false, "payer": null },
        "storage": { "usedBytes": 0, "limitBytes": null, "ownStorage": false },
        "uploads": { "usedBytes24h": 0, "limitBytesPerDay": null },
    })
}

pub fn report(stored: &Value, since: &str) -> Value {
    json!({
        "since": since,
        "defaultAgentBudgetUsd": nano_to_usd(budget_nano_of(stored)),
        "totalUsd": 0,
        "platformBilled": { "calls": 0, "costUsd": 0 },
        "byKind": [],
        "agents": [],
    })
}

pub fn plans(month: &str) -> Value {
    json!({ "month": month, "money": true, "accounts": [], "api": [] })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_overview_counts_the_owner_alone() {
        assert_eq!(
            overview(&json!({ "name": "Casa" })),
            json!({ "name": "Casa", "role": "owner", "people": 1, "agents": 0, "budgetUsedPercent": 0, "approvalsWaitingForMe": 0, "proposalsWaitingForMe": 0 })
        );
    }

    #[test]
    fn limits_are_the_owner_alone_with_no_caps() {
        assert_eq!(
            limits()["members"],
            json!({ "used": 1, "limit": null, "max": null, "canAddSeats": false, "payer": null })
        );
        assert_eq!(limits()["storage"]["limitBytes"], Value::Null);
    }

    #[test]
    fn usage_without_records_is_zero_since_the_month_began() {
        let report = report(&Value::Null, "2026-11-01T00:00:00.000Z");
        assert_eq!(report["since"], "2026-11-01T00:00:00.000Z");
        assert_eq!(report["defaultAgentBudgetUsd"], 50.0);
        assert_eq!(report["byKind"], json!([]));
        assert_eq!(plans("2026-11"), json!({ "month": "2026-11", "money": true, "accounts": [], "api": [] }));
    }
}
