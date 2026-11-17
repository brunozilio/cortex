use domain::permission_rules::RuleKind;
use domain::suggested_rules::Suggestion;

use super::{Feature, Slash};
use crate::tui::app::{App, Msg};

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
    app.spawn(move |client| {
        let found = match client.suggested_rules(&cwd) {
            Ok(found) => found,
            Err(error) => return Msg::Error(error.to_string()),
        };
        match keep {
            None => Msg::Notice(listed(&found)),
            Some(Err(_)) => Msg::Notice("Use /rules keep <n>, with n from /rules.".into()),
            Some(Ok(at)) => match found.get(at.wrapping_sub(1)) {
                Some(suggestion) => match client.keep_rule(&cwd, suggestion.kind, &suggestion.rule) {
                    Ok(()) => Msg::Notice(format!(
                        "Kept {} as a {} rule in {}/settings.local.json.",
                        suggestion.rule,
                        suggestion.kind.word(),
                        brand::DIR
                    )),
                    Err(error) => Msg::Error(error.to_string()),
                },
                None => Msg::Notice(format!("There is no suggestion {at}; /rules lists them.")),
            },
        }
    });
}

fn listed(found: &[Suggestion]) -> String {
    let lines: Vec<String> = found
        .iter()
        .enumerate()
        .map(|(at, suggestion)| {
            let verb = if suggestion.kind == RuleKind::Deny { "denied" } else { "allowed" };
            format!(
                "  {}. {} {}  (you {verb} it {} times)",
                at + 1,
                suggestion.kind.word(),
                suggestion.rule,
                suggestion.times
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
        let found = [
            Suggestion { rule: "Bash(cargo test:*)".into(), kind: RuleKind::Allow, times: 7 },
            Suggestion { rule: "Bash(git push:*)".into(), kind: RuleKind::Deny, times: 3 },
        ];
        let text = listed(&found);
        assert!(text.contains("  1. allow Bash(cargo test:*)  (you allowed it 7 times)"), "{text}");
        assert!(text.contains("  2. deny Bash(git push:*)  (you denied it 3 times)"), "{text}");
        assert!(listed(&[]).starts_with("No rules"));
    }
}
