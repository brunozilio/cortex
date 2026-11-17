use serde_json::Value;

pub const SCHEDULING_TOOLS: [&str; 2] = ["ScheduleWakeup", "CronCreate"];

#[derive(Debug, Clone, PartialEq)]
pub struct Active {
    pub id: String,
    pub tool: String,
    pub input: Value,
}

pub fn while_watching(name: &str, started_this_run: &[String]) -> Option<String> {
    if started_this_run.is_empty() || !SCHEDULING_TOOLS.contains(&name) {
        return None;
    }
    Some(format!(
        "Not run: {} already checks for you and you will be told when it is met, as a new message even while the \
         chat is idle. End your turn now, and do what the user asked for afterwards only when that event arrives. \
         If the user wants something else on a schedule, they can ask in a new message.",
        started_this_run.join(", ")
    ))
}

pub fn duplicate(tool: &str, input: &Value, active: &[Active]) -> Option<String> {
    let same = active.iter().find(|watch| watch.tool == tool && watch.input == *input)?;
    Some(format!(
        "Not started: {} already watches {tool} with this input and will tell you when it is met. End your turn now.",
        same.id
    ))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn scheduling_is_refused_in_a_run_that_started_a_watch() {
        let refused = while_watching("ScheduleWakeup", &["watch_1".into()]).expect("refused");
        assert!(refused.starts_with("Not run: watch_1 already checks for you"), "{refused}");
        assert!(refused.contains("End your turn now"));
        assert!(while_watching("CronCreate", &["watch_1".into()]).is_some());
        assert_eq!(while_watching("ScheduleWakeup", &[]), None, "no watch, no refusal");
        assert_eq!(while_watching("Bash", &["watch_1".into()]), None, "other tools are left alone");
    }

    #[test]
    fn a_second_watch_of_the_same_call_is_refused() {
        let active = [Active { id: "watch_3".into(), tool: "mcp__jira__get".into(), input: json!({ "key": "KAN-1" }) }];
        let refused = duplicate("mcp__jira__get", &json!({ "key": "KAN-1" }), &active).expect("refused");
        assert!(refused.starts_with("Not started: watch_3 already watches mcp__jira__get"), "{refused}");
        assert_eq!(duplicate("mcp__jira__get", &json!({ "key": "KAN-2" }), &active), None);
        assert_eq!(duplicate("Read", &json!({ "key": "KAN-1" }), &active), None);
    }
}
