use agent_engine::tools::builtin::schedules;
use domain::Principal;
use serde_json::{Value, json};

pub fn handle(client: u64, _principal: &Principal, input: Value) -> Result<Value, String> {
    if let Some(id) = input["delete"].as_str() {
        schedules::remove(client, id);
    }
    let due = if input["take_due"] == true { schedules::take_due(client, schedules::now()) } else { Vec::new() };
    Ok(json!({ "due": due, "scheduled": schedules::list(client) }))
}

pub fn forget(client: u64) {
    schedules::forget_client(client);
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::schedule::{Scheduled, When};

    fn listed(value: &Value, key: &str) -> Vec<Scheduled> {
        serde_json::from_value(value[key].clone()).unwrap_or_default()
    }

    #[test]
    fn a_client_takes_its_due_prompts_once_and_deletes_the_others() {
        let (client, other, owner) = (77_001, 77_002, Principal::LocalOwner);
        let now = schedules::now();
        schedules::add(client, "check the build", When::At(now - 1), now).expect("due");
        let later = schedules::add(client, "tomorrow", When::At(now + 86_400), now).expect("later");
        let found = handle(client, &owner, json!({ "take_due": true })).expect("found");
        assert_eq!(listed(&found, "due").iter().map(|due| due.prompt.as_str()).collect::<Vec<_>>(), ["check the build"]);
        assert_eq!(listed(&found, "scheduled"), std::slice::from_ref(&later));
        assert!(listed(&handle(client, &owner, json!({ "take_due": true })).expect("again"), "due").is_empty());
        assert!(listed(&handle(other, &owner, json!({})).expect("other"), "scheduled").is_empty());
        let deleted = handle(client, &owner, json!({ "delete": later.id })).expect("deleted");
        assert!(listed(&deleted, "scheduled").is_empty());
        forget(client);
    }
}
