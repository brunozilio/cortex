use std::collections::HashMap;

pub const ALLOWED_REPEATS: u32 = 2;

const WAITING_TOOLS: [&str; 6] = ["Bash", "BashOutput", "TaskOutput", "TaskList", "TaskGet", "Monitor"];

#[derive(Debug, Default)]
pub struct Repeats {
    seen: HashMap<(String, String), u32>,
}

impl Repeats {
    pub fn admit(&mut self, name: &str, input: &str, may_change: bool) -> bool {
        if WAITING_TOOLS.contains(&name) {
            return true;
        }
        if may_change {
            self.seen.retain(|(seen, _), _| seen == name);
        }
        let count = self.seen.entry((name.to_owned(), input.to_owned())).or_default();
        *count += 1;
        *count <= ALLOWED_REPEATS
    }
}

pub fn refused(name: &str) -> String {
    format!(
        "Not run: you already called {name} with this exact input {ALLOWED_REPEATS} times in this run and nothing \
         changed since, so its earlier result still holds. If you need a tool you do not have, load it with \
         ToolSearch (query \"select:<name>\"); otherwise go on with what you already know."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_third_identical_call_with_nothing_changed_in_between_is_refused() {
        let mut repeats = Repeats::default();
        assert!(repeats.admit("mcp__jira__getJiraIssue", r#"{"key":"KAN-1"}"#, true));
        assert!(repeats.admit("ToolSearch", r#"{"query":"x"}"#, false));
        assert!(repeats.admit("mcp__jira__getJiraIssue", r#"{"key":"KAN-1"}"#, true));
        assert!(!repeats.admit("mcp__jira__getJiraIssue", r#"{"key":"KAN-1"}"#, true));
        assert!(repeats.admit("mcp__jira__getJiraIssue", r#"{"key":"KAN-2"}"#, true), "other input");
    }

    #[test]
    fn a_change_by_another_tool_starts_the_count_again() {
        let mut repeats = Repeats::default();
        repeats.admit("Read", r#"{"file_path":"a"}"#, false);
        repeats.admit("Read", r#"{"file_path":"a"}"#, false);
        assert!(repeats.admit("Edit", r#"{"file_path":"a"}"#, true));
        assert!(repeats.admit("Read", r#"{"file_path":"a"}"#, false));
        assert!(repeats.admit("Read", r#"{"file_path":"a"}"#, false));
        assert!(!repeats.admit("Read", r#"{"file_path":"a"}"#, false));
    }

    #[test]
    fn tools_that_wait_on_something_running_may_repeat() {
        let mut repeats = Repeats::default();
        for _ in 0..5 {
            assert!(repeats.admit("BashOutput", r#"{"bash_id":"bash_1"}"#, false));
            assert!(repeats.admit("Bash", r#"{"command":"npm test"}"#, true));
        }
    }

    #[test]
    fn the_refusal_points_at_loading_another_tool() {
        let text = refused("mcp__jira__getJiraIssue");
        assert!(text.contains("mcp__jira__getJiraIssue") && text.contains("select:"), "{text}");
    }
}
