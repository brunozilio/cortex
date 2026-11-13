use serde_json::{Value, json};

use super::{Feature, Slash};
use crate::tui::app::{App, Msg};

const NAME: &str = "suggested_rules";

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash {
            names: &["rules"],
            about: "Rules your past answers suggest: /rules lists them, /rules keep <n> saves one",
            run,
        }],
        ..Feature::default()
    }
}

fn run(app: &mut App, argument: Option<String>) {
    let cwd = app.cwd.clone();
    let keep = argument.as_deref().and_then(|text| text.strip_prefix("keep")).map(str::trim).map(str::parse::<usize>);
    app.spawn(move |sdk| {
        let found = match sdk.feature(NAME, json!({ "cwd": cwd })) {
            Ok(found) => found,
            Err(error) => return Msg::Error(error.to_string()),
        };
        match keep {
            None => Msg::Notice(listed(&found)),
            Some(Err(_)) => Msg::Notice("Use /rules keep <n>, with n from /rules.".into()),
            Some(Ok(at)) => match found.get(at.wrapping_sub(1)) {
                Some(rule) => match sdk.feature(NAME, json!({ "cwd": cwd, "keep": rule })) {
                    Ok(_) => Msg::Notice(format!(
                        "Kept {} as a {} rule in {}/settings.local.json.",
                        rule["rule"].as_str().unwrap_or_default(),
                        rule["kind"].as_str().unwrap_or_default(),
                        brand::DIR
                    )),
                    Err(error) => Msg::Error(error.to_string()),
                },
                None => Msg::Notice(format!("There is no suggestion {at}; /rules lists them.")),
            },
        }
    });
}

fn listed(found: &Value) -> String {
    let lines: Vec<String> = found
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(at, rule)| {
            let verb = if rule["kind"] == "deny" { "denied" } else { "allowed" };
            format!(
                "  {}. {} {}  (you {verb} it {} times)",
                at + 1,
                rule["kind"].as_str().unwrap_or_default(),
                rule["rule"].as_str().unwrap_or_default(),
                rule["times"]
            )
        })
        .collect();
    if lines.is_empty() {
        return "No rules to suggest yet: they come from answers you give again and again.".into();
    }
    format!("Your answers suggest these rules (/rules keep <n> saves one):\n{}", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_suggestion_is_numbered_with_why() {
        let found = json!([
            { "rule": "Bash(cargo test:*)", "kind": "allow", "times": 7 },
            { "rule": "Bash(git push:*)", "kind": "deny", "times": 3 }
        ]);
        let text = listed(&found);
        assert!(text.contains("  1. allow Bash(cargo test:*)  (you allowed it 7 times)"), "{text}");
        assert!(text.contains("  2. deny Bash(git push:*)  (you denied it 3 times)"), "{text}");
        assert!(listed(&json!([])).starts_with("No rules"));
    }
}
