use agent_engine::tools::builtin::schedules;
use serde_json::{Value, json};

use crate::feature::Context;

pub fn handle(context: &Context<'_>, input: Value) -> Result<Value, String> {
    let client = context.client;
    if let Some(id) = input["delete"].as_str() {
        schedules::remove(client, id);
    }
    if let Some(wanted) = input.get("create") {
        let when = serde_json::from_value(wanted["when"].clone()).map_err(|error| error.to_string())?;
        let prompt = wanted["prompt"].as_str().map(str::trim).filter(|prompt| !prompt.is_empty());
        schedules::add(client, prompt.ok_or("a scheduled prompt needs its text")?, when, schedules::now())?;
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

    #[test]
    fn a_client_schedules_a_prompt_of_its_own() {
        let client = 77_003;
        let ask = |input: Value| crate::feature::testing::local(client, |context| handle(context, input));
        let at = schedules::now() + 600;
        let made = ask(json!({ "create": { "prompt": "check the deploy", "when": { "at": at } } })).expect("made");
        let kept = listed(&made, "scheduled");
        assert_eq!(kept.len(), 1);
        assert_eq!((kept[0].prompt.as_str(), &kept[0].when), ("check the deploy", &When::At(at)));
        assert!(ask(json!({ "create": { "prompt": "  ", "when": { "at": at } } })).is_err(), "a prompt is needed");
        assert!(ask(json!({ "create": { "prompt": "x", "when": "soon" } })).is_err());
        forget(client);
    }
}
