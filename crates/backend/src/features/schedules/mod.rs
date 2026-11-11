use agent_engine::tools::builtin::schedules;
use serde_json::{Value, json};

use crate::feature::Context;

pub fn handle(context: &Context<'_>, input: Value) -> Result<Value, String> {
    let client = context.client;
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
        let (client, other) = (77_001, 77_002);
        let ask = |client: u64, input: Value| crate::feature::testing::local(client, |context| handle(context, input));
        let now = schedules::now();
        schedules::add(client, "check the build", When::At(now - 1), now).expect("due");
        let later = schedules::add(client, "tomorrow", When::At(now + 86_400), now).expect("later");
        let found = ask(client, json!({ "take_due": true })).expect("found");
        assert_eq!(
            listed(&found, "due").iter().map(|due| due.prompt.as_str()).collect::<Vec<_>>(),
            ["check the build"]
        );
        assert_eq!(listed(&found, "scheduled"), std::slice::from_ref(&later));
        assert!(listed(&ask(client, json!({ "take_due": true })).expect("again"), "due").is_empty());
        assert!(listed(&ask(other, json!({})).expect("other"), "scheduled").is_empty());
        let deleted = ask(client, json!({ "delete": later.id })).expect("deleted");
        assert!(listed(&deleted, "scheduled").is_empty());
        forget(client);
    }
}
