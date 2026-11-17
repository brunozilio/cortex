use crate::permission_rules::{PermissionRules, RuleKind};

const FENCE: &str = "```";

pub fn commands(criteria: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut fenced = false;
    for line in criteria.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(FENCE) {
            fenced = !fenced;
            continue;
        }
        let spans: Vec<&str> =
            if fenced { vec![trimmed.trim_start_matches("$ ")] } else { line.split('`').skip(1).step_by(2).collect() };
        for span in spans.into_iter().map(str::trim).filter(|span| !span.is_empty()) {
            if !found.iter().any(|known| known == span) {
                found.push(span.to_owned());
            }
        }
    }
    found
}

pub fn judge_rules(criteria: &str, owner: &PermissionRules) -> PermissionRules {
    commands(criteria)
        .iter()
        .fold(owner.clone(), |rules, command| rules.with(RuleKind::Allow, &format!("Bash({command})")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission_rules::{PermissionRules, RuleKind, Target};

    #[test]
    fn the_commands_are_the_criteria_s_code_spans_and_code_block_lines() {
        let text = "Run `cargo test --workspace` and `npm run build`.\n```sh\n$ make lint\n\npytest -q\n```\nDone.";
        assert_eq!(commands(text), ["cargo test --workspace", "npm run build", "make lint", "pytest -q"]);
        assert!(commands("no commands here").is_empty());
    }

    #[test]
    fn the_judge_may_run_what_the_criteria_name_and_what_the_owner_allowed_and_nothing_else() {
        let owner = PermissionRules::default().with(RuleKind::Allow, "Bash(make check:*)");
        let rules = judge_rules("`cargo test` passes", &owner);
        let decide = |command: &str| rules.decide("Bash", &Target::Command(command), "/p", None);
        assert_eq!(decide("cargo test"), Some(RuleKind::Allow));
        assert_eq!(decide("make check --all"), Some(RuleKind::Allow));
        assert_eq!(decide("rm -rf target"), None);
        assert_eq!(decide("cargo test && rm -rf target"), None, "a compound command needs every part allowed");
    }
}
