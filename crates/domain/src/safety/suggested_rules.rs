use std::collections::BTreeMap;

use crate::permission_rules::RuleKind;
use crate::{AskKind, Decision, EngineEvent};

const ALLOWS: u32 = 5;
const DENIALS: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Suggestion {
    pub rule: String,
    pub kind: RuleKind,
    pub times: u32,
}

#[derive(Debug, Default)]
struct Tally {
    allowed: u32,
    denied: u32,
}

pub fn suggestions<'a>(
    events: impl IntoIterator<Item = &'a EngineEvent>,
    kept: &[(RuleKind, String)],
) -> Vec<Suggestion> {
    let mut asked: BTreeMap<String, String> = BTreeMap::new();
    let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();
    for event in events {
        match event {
            EngineEvent::Asked(ask) => {
                if let AskKind::Permission { rule: Some(rule), .. } = &ask.kind {
                    asked.insert(ask.id.0.clone(), rule.clone());
                }
            }
            EngineEvent::Answered { ask, decision } => {
                let Some(rule) = asked.remove(&ask.0) else { continue };
                let tally = tallies.entry(rule).or_default();
                match decision {
                    Decision::Allow => tally.allowed += 1,
                    Decision::Deny | Decision::DenyWith(_) => tally.denied += 1,
                    _ => {}
                }
            }
            _ => {}
        }
    }
    let known = |rule: &str| kept.iter().any(|(_, kept)| kept == rule);
    let mut found: Vec<Suggestion> = tallies
        .into_iter()
        .filter(|(rule, _)| !known(rule))
        .filter_map(|(rule, tally)| match (tally.allowed, tally.denied) {
            (allowed, 0) if allowed >= ALLOWS => Some(Suggestion { rule, kind: RuleKind::Allow, times: allowed }),
            (0, denied) if denied >= DENIALS => Some(Suggestion { rule, kind: RuleKind::Deny, times: denied }),
            _ => None,
        })
        .collect();
    found.sort_by(|a, b| b.times.cmp(&a.times).then_with(|| a.rule.cmp(&b.rule)));
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentAsk, AskId};

    fn asked(id: &str, rule: &str, decision: Decision) -> [EngineEvent; 2] {
        let kind = AskKind::Permission {
            tool: "Bash".into(),
            subject: "x".into(),
            rememberable: true,
            rule: Some(rule.into()),
        };
        [
            EngineEvent::Asked(AgentAsk { id: AskId(id.into()), kind }),
            EngineEvent::Answered { ask: AskId(id.into()), decision },
        ]
    }

    fn history(rule: &str, decision: &Decision, times: usize) -> Vec<EngineEvent> {
        (0..times).flat_map(|at| asked(&format!("{rule}-{at}"), rule, decision.clone())).collect()
    }

    #[test]
    fn a_rule_approved_again_and_again_is_offered_as_allow() {
        let events = history("Bash(cargo test:*)", &Decision::Allow, 5);

        let found = suggestions(&events, &[]);

        assert_eq!(found, [Suggestion { rule: "Bash(cargo test:*)".into(), kind: RuleKind::Allow, times: 5 }]);
        assert!(suggestions(&events[..8], &[]).is_empty(), "four approvals are not yet a habit");
    }

    #[test]
    fn a_rule_always_denied_is_offered_as_deny_and_a_mixed_one_is_not_offered() {
        let denied = history("Bash(git push:*)", &Decision::Deny, 3);
        assert_eq!(suggestions(&denied, &[])[0].kind, RuleKind::Deny);

        let mut mixed = history("Bash(npm run:*)", &Decision::Allow, 6);
        mixed.extend(asked("late", "Bash(npm run:*)", Decision::Deny));
        assert!(suggestions(&mixed, &[]).is_empty());
    }

    #[test]
    fn a_rule_already_kept_is_not_offered_again() {
        let events = history("Bash(cargo test:*)", &Decision::Allow, 5);
        let kept = [(RuleKind::Allow, "Bash(cargo test:*)".to_owned())];
        assert!(suggestions(&events, &kept).is_empty());
    }
}
